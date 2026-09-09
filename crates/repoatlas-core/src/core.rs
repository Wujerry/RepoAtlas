use crate::db;
use crate::detect;
use crate::environment;
use crate::error::{Error, Result};
use crate::git;
use crate::models::{
    AppSettings, AtlasReport, AttentionItem, AuditEvent, CheckoutLineage, CollectionUpsert,
    DashboardCollectionSummary, DashboardProjectItem, DashboardSnapshot, DashboardTaskRunItem,
    DashboardTaskRunStats, DashboardTaskRunSummary, GitCommandResult, GitDiff, GitOp, GitSnapshot,
    GitStatus, LineageCheckout, PendingApproval, ProjectBrief, ProjectCollection, ProjectDetail,
    ProjectEvent, ProjectPatch, ProjectQuery, ProjectRemoval, ProjectSummary, ReadmeDocument,
    ScanProgress, ScanResult, ScanRoot, ScanRootRemoval, SearchHit, StartHereTask, TaskDefinition,
};
use crate::paths;
use crate::scan::{path_exists, ScanEngine};
use chrono::{SecondsFormat, Utc};
use fs2::FileExt;
use rusqlite::{params, Connection, OptionalExtension};
use serde::de::DeserializeOwned;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::AtomicBool;
use uuid::Uuid;

mod approvals;
mod modules;

pub struct Core {
    conn: Connection,
    log_dir: Option<PathBuf>,
    // The desktop process owns an OS-level lock for the database lifetime.
    // MCP can still open a second SQLite connection, but it must not perform
    // desktop-runtime recovery while that lock is held by the app.
    runtime_lock: Option<File>,
}

type IconOverride = (String, Vec<u8>, Option<String>);

/// Immutable operation context; filesystem and process work requires no Core
/// connection. Adapters serialize writes for the same checkout outside Core.
pub struct PreparedGitOperation {
    project_id: String,
    path: PathBuf,
    op: GitOp,
}

impl PreparedGitOperation {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn execute(&self) -> Result<(GitCommandResult, Option<GitSnapshot>)> {
        let result = git::execute(&self.path, self.op.clone())?;
        let snapshot = git::snapshot(&self.path);
        Ok((result, snapshot))
    }
}

impl Core {
    /// Changes committed by other connections, not filesystem changes. The
    /// token is meaningful only on the long-lived desktop Core connection.
    pub fn data_version(&self) -> Result<i64> {
        self.conn
            .query_row("PRAGMA data_version", [], |row| row.get(0))
            .map_err(Into::into)
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let runtime_lock = acquire_runtime_lock(path, true)?;
        let core = Self {
            conn: db::open(path)?,
            log_dir: path.parent().map(|parent| parent.join("task-logs")),
            runtime_lock,
        };
        // A desktop restart cannot keep the in-memory Broker handles which
        // own running child processes. Resolve any persisted transition that
        // was interrupted by that restart before exposing the database to the
        // UI. The standalone MCP process uses `open_without_recovery` so it
        // can coexist with an already-running desktop Broker.
        core.recover_interrupted_runtime()?;
        core.retry_managed_file_cleanup()?;
        Ok(core)
    }

    /// Open a database without claiming ownership of task-run recovery.
    ///
    /// MCP can be launched while the desktop app is still running. In that
    /// case the desktop Broker remains the source of truth for live child
    /// processes, so an MCP connection must not mark those rows failed merely
    /// because it opened the shared SQLite file.
    pub fn open_without_recovery(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        // If no desktop owner is present, MCP may claim the lock and perform
        // normal restart recovery. When the desktop owns it, keep the MCP
        // connection unlocked and leave live task rows untouched.
        let runtime_lock = acquire_runtime_lock(path, false)?;
        let core = Self {
            conn: db::open(path)?,
            log_dir: path.parent().map(|parent| parent.join("task-logs")),
            runtime_lock,
        };
        if core.runtime_lock.is_some() {
            core.recover_interrupted_runtime()?;
        }
        core.retry_managed_file_cleanup()?;
        Ok(core)
    }

    pub fn open_in_memory() -> Result<Self> {
        let core = Self {
            conn: db::open_in_memory()?,
            // Keep the same broker-log boundary as the standalone MCP
            // process. Tests and ephemeral callers still get a real safety
            // boundary instead of an opt-out from log path validation.
            log_dir: Some(std::env::temp_dir().join("repoatlas-task-logs")),
            runtime_lock: None,
        };
        core.recover_interrupted_runtime()?;
        core.retry_managed_file_cleanup()?;
        Ok(core)
    }

    pub fn settings(&self) -> Result<AppSettings> {
        let theme = self.setting("theme")?.unwrap_or_else(|| "system".into());
        let locale = self.setting("locale")?.unwrap_or_else(|| "system".into());
        let ui_font = self.setting("uiFont")?.unwrap_or_default();
        let console_font = self.setting("consoleFont")?.unwrap_or_default();
        Ok(AppSettings {
            theme,
            locale,
            ui_font,
            console_font,
        })
    }

    pub fn update_settings(&self, settings: AppSettings) -> Result<AppSettings> {
        self.put_setting("theme", &settings.theme)?;
        self.put_setting("locale", &settings.locale)?;
        self.put_setting("uiFont", &settings.ui_font)?;
        self.put_setting("consoleFont", &settings.console_font)?;
        Ok(settings)
    }

    pub fn add_scan_root(&self, path: impl AsRef<Path>) -> Result<ScanRoot> {
        self.add_scan_root_with_optional_origin(path, None)
    }

    /// Add a Scan Root and persist its audit record in the same transaction.
    ///
    /// The path is canonicalized before opening the transaction. Only the
    /// small metadata mutation and audit insert are performed while SQLite's
    /// write lock is held.
    pub fn add_scan_root_with_origin(
        &self,
        path: impl AsRef<Path>,
        origin: &str,
    ) -> Result<ScanRoot> {
        self.add_scan_root_with_optional_origin(path, Some(origin))
    }

    fn add_scan_root_with_optional_origin(
        &self,
        path: impl AsRef<Path>,
        origin: Option<&str>,
    ) -> Result<ScanRoot> {
        let canonical = paths::canonicalize(path.as_ref())?;
        if !canonical.is_dir() {
            return Err(Error::msg(format!(
                "scan root is not a directory: {}",
                paths::path_to_string(&canonical)
            )));
        }
        let path = paths::path_to_string(&canonical);
        let origin = origin.map(str::to_owned);
        self.with_immediate_transaction(|core| {
            let root = core.add_scan_root_inner(&path)?;
            if let Some(origin) = origin.as_deref() {
                core.record_audit_event(
                    origin,
                    "add_scan_root",
                    "scan_root",
                    Some(&root.id),
                    Some("path supplied"),
                    "success",
                )?;
            }
            Ok(root)
        })
    }

    fn add_scan_root_inner(&self, path: &str) -> Result<ScanRoot> {
        let now = now();
        let id = Uuid::new_v4().to_string();
        match self.conn.execute(
            "INSERT INTO scan_roots (id, path, created_at) VALUES (?1, ?2, ?3)",
            params![id, path, now],
        ) {
            Ok(_) => self.get_scan_root(&id),
            Err(rusqlite::Error::SqliteFailure(err, _))
                if err.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                Ok(self.conn.query_row(
                    "SELECT id, path, created_at, last_scanned_at FROM scan_roots WHERE path = ?1",
                    params![path],
                    scan_root_from_row,
                )?)
            }
            Err(err) => Err(err.into()),
        }
    }

    pub fn list_scan_roots(&self) -> Result<Vec<ScanRoot>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, path, created_at, last_scanned_at FROM scan_roots ORDER BY created_at",
        )?;
        let rows = stmt
            .query_map([], scan_root_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Remove a Scan Root record and optionally the projects discovered from it.
    ///
    /// This compatibility entry point deliberately does not create an audit
    /// event. Call [`Self::remove_scan_root_with_origin`] when the caller needs
    /// the mutation and its audit record to commit (or roll back) together.
    pub fn remove_scan_root(&self, id: &str, also_remove_records: bool) -> Result<ScanRootRemoval> {
        self.remove_scan_root_with_optional_origin(id, also_remove_records, None)
    }

    /// Remove a Scan Root and persist the caller origin in the same SQLite
    /// transaction as the removal.
    ///
    /// The audit insert intentionally happens after the root/project rows have
    /// been removed. Project deletion clears audit rows that belong to the
    /// deleted project, while this root-level event must remain as the durable
    /// record of the operation. If the audit insert fails, the complete
    /// metadata mutation is rolled back.
    pub fn remove_scan_root_with_origin(
        &self,
        id: &str,
        also_remove_records: bool,
        origin: &str,
    ) -> Result<ScanRootRemoval> {
        self.remove_scan_root_with_optional_origin(id, also_remove_records, Some(origin))
    }

    fn remove_scan_root_with_optional_origin(
        &self,
        id: &str,
        also_remove_records: bool,
        origin: Option<&str>,
    ) -> Result<ScanRootRemoval> {
        let origin = origin.map(str::to_owned);
        let (root, project_ids, removed_projects, pending_log_cleanup) =
            self.with_immediate_transaction(|core| {
            let root = core.get_scan_root(id)?;
            let project_ids: Vec<String> = {
                let mut stmt = core
                    .conn
                    .prepare("SELECT id FROM projects WHERE scan_root_id = ?1")?;
                let rows = stmt
                    .query_map(params![id], |row| row.get(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                rows
            };
            if also_remove_records {
                core.ensure_no_running_projects(&project_ids)?;
            }
            let mut removed_projects = 0;
            let mut pending_log_cleanup = 0;
            if also_remove_records {
                for project_id in &project_ids {
                    let removal = core.remove_project_inner(project_id)?;
                    removed_projects += 1;
                    pending_log_cleanup += removal.pending_log_cleanup;
                }
            } else {
                core.conn.execute(
                    "UPDATE projects SET scan_root_id = NULL, origin = CASE WHEN origin = 'scan' THEN 'orphaned' ELSE origin END WHERE scan_root_id = ?1",
                    params![id],
                )?;
            }
            core.conn
                .execute("DELETE FROM scan_roots WHERE id = ?1", params![id])?;
            if let Some(origin) = origin.as_deref() {
                core.record_audit_event(
                    origin,
                    "remove_scan_root",
                    "scan_root",
                    Some(id),
                    Some(if also_remove_records {
                        "record policy removeRecords; RepoAtlas records removed; filesystem preserved"
                    } else {
                        "record policy orphan; RepoAtlas authorization removed; filesystem preserved"
                    }),
                    "success",
                )?;
            }
            Ok((root, project_ids, removed_projects, pending_log_cleanup))
        })?;
        // Filesystem cleanup is deliberately outside the database transaction:
        // a locked SQLite transaction must never be held while waiting on an
        // arbitrary filesystem operation. The queue keeps retries durable.
        let _ = self.retry_managed_file_cleanup();
        let pending_log_cleanup = if also_remove_records {
            project_ids
                .iter()
                .map(|project_id| self.pending_log_cleanup_count(project_id).unwrap_or(0))
                .sum()
        } else {
            pending_log_cleanup
        };
        Ok(ScanRootRemoval {
            root,
            removed_projects,
            orphaned_projects: if also_remove_records {
                0
            } else {
                project_ids.len() as u64
            },
            pending_log_cleanup,
            filesystem_deleted: false,
        })
    }

    pub fn list_projects(&self, query: ProjectQuery) -> Result<Vec<ProjectSummary>> {
        let collection_ids = query
            .collection_id
            .as_deref()
            .map(|id| self.collection_project_ids(id))
            .transpose()?;
        if let Some(search) = query
            .search
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            return Ok(self
                .search_projects(search, 5_000)?
                .into_iter()
                .map(|hit| hit.project)
                .filter(|project| {
                    collection_ids
                        .as_ref()
                        .is_none_or(|ids| ids.contains(&project.id))
                })
                .filter(|project| self.matches_section(project, &query))
                .take(query.limit.unwrap_or(200).clamp(1, 5_000))
                .collect());
        }
        self.load_projects_for_query(&query)
    }

    pub fn list_collections(&self) -> Result<Vec<ProjectCollection>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.id, c.name, c.description, COUNT(m.project_id), c.created_at, c.updated_at
             FROM project_collections c
             LEFT JOIN project_collection_members m ON m.collection_id = c.id
             GROUP BY c.id
             ORDER BY lower(c.name), c.id",
        )?;
        let collections = stmt
            .query_map([], |row| {
                Ok(ProjectCollection {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    project_count: row.get::<_, i64>(3)? as usize,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(collections)
    }

    pub fn create_collection_with_origin(
        &self,
        upsert: CollectionUpsert,
        origin: &str,
    ) -> Result<ProjectCollection> {
        let (name, description) = validate_collection(upsert)?;
        let id = Uuid::new_v4().to_string();
        let timestamp = now();
        self.with_immediate_transaction(|core| {
            core.conn.execute(
                "INSERT INTO project_collections (id, name, description, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
                params![id, name, description, timestamp],
            ).map_err(map_collection_constraint)?;
            core.record_audit_event(origin, "create_collection", "collection", Some(&id), None, "success")?;
            core.get_collection(&id)
        })
    }

    pub fn update_collection_with_origin(
        &self,
        id: &str,
        upsert: CollectionUpsert,
        origin: &str,
    ) -> Result<ProjectCollection> {
        let (name, description) = validate_collection(upsert)?;
        self.with_immediate_transaction(|core| {
            let changed = core.conn.execute(
                "UPDATE project_collections SET name = ?1, description = ?2, updated_at = ?3 WHERE id = ?4",
                params![name, description, now(), id],
            ).map_err(map_collection_constraint)?;
            if changed == 0 { return Err(Error::NotFound(id.into())); }
            core.record_audit_event(origin, "update_collection", "collection", Some(id), None, "success")?;
            core.get_collection(id)
        })
    }

    pub fn delete_collection_with_origin(&self, id: &str, origin: &str) -> Result<()> {
        self.with_immediate_transaction(|core| {
            let changed = core
                .conn
                .execute("DELETE FROM project_collections WHERE id = ?1", [id])?;
            if changed == 0 {
                return Err(Error::NotFound(id.into()));
            }
            core.record_audit_event(
                origin,
                "delete_collection",
                "collection",
                Some(id),
                Some("collection record only; project directories unchanged"),
                "success",
            )?;
            Ok(())
        })
    }

    pub fn set_collection_members_with_origin(
        &self,
        id: &str,
        project_ids: &[String],
        origin: &str,
    ) -> Result<ProjectCollection> {
        let unique = project_ids.iter().cloned().collect::<HashSet<_>>();
        self.with_immediate_transaction(|core| {
            core.get_collection(id)?;
            core.ensure_projects_exist(&unique)?;
            let timestamp = now();
            core.replace_collection_members(id, &unique, &timestamp)?;
            core.conn.execute(
                "UPDATE project_collections SET updated_at = ?1 WHERE id = ?2",
                params![now(), id],
            )?;
            core.record_audit_event(
                origin,
                "set_collection_members",
                "collection",
                Some(id),
                Some(&format!("{} projects", project_ids.len())),
                "success",
            )?;
            core.get_collection(id)
        })
    }

    pub fn collection_member_ids(&self, id: &str) -> Result<Vec<String>> {
        self.get_collection(id)?;
        let mut stmt = self.conn.prepare(
            "SELECT project_id FROM project_collection_members WHERE collection_id = ?1 ORDER BY created_at, project_id",
        )?;
        let project_ids = stmt
            .query_map([id], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(project_ids)
    }

    pub fn save_collection_with_members_with_origin(
        &self,
        id: Option<&str>,
        upsert: CollectionUpsert,
        project_ids: &[String],
        origin: &str,
    ) -> Result<ProjectCollection> {
        let (name, description) = validate_collection(upsert)?;
        let collection_id = id
            .map(str::to_string)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let unique = project_ids.iter().cloned().collect::<HashSet<_>>();
        self.with_immediate_transaction(|core| {
            core.ensure_projects_exist(&unique)?;
            let timestamp = now();
            if id.is_some() {
                let changed = core.conn.execute(
                    "UPDATE project_collections SET name = ?1, description = ?2, updated_at = ?3 WHERE id = ?4",
                    params![name, description, timestamp, collection_id],
                ).map_err(map_collection_constraint)?;
                if changed == 0 { return Err(Error::NotFound(collection_id.clone())); }
            } else {
                core.conn.execute(
                    "INSERT INTO project_collections (id, name, description, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
                    params![collection_id, name, description, timestamp],
                ).map_err(map_collection_constraint)?;
            }
            core.replace_collection_members(&collection_id, &unique, &timestamp)?;
            core.record_audit_event(origin, "save_collection", "collection", Some(&collection_id), Some(&format!("{} projects", unique.len())), "success")?;
            core.get_collection(&collection_id)
        })
    }

    fn get_collection(&self, id: &str) -> Result<ProjectCollection> {
        self.conn.query_row(
            "SELECT c.id, c.name, c.description, COUNT(m.project_id), c.created_at, c.updated_at
             FROM project_collections c LEFT JOIN project_collection_members m ON m.collection_id = c.id
             WHERE c.id = ?1 GROUP BY c.id",
            [id],
            |row| Ok(ProjectCollection { id: row.get(0)?, name: row.get(1)?, description: row.get(2)?, project_count: row.get::<_, i64>(3)? as usize, created_at: row.get(4)?, updated_at: row.get(5)? }),
        ).optional()?.ok_or_else(|| Error::NotFound(id.into()))
    }

    fn collection_project_ids(&self, id: &str) -> Result<HashSet<String>> {
        Ok(self.collection_member_ids(id)?.into_iter().collect())
    }

    fn ensure_projects_exist(&self, project_ids: &HashSet<String>) -> Result<()> {
        let mut found = HashSet::with_capacity(project_ids.len());
        let ids = project_ids.iter().collect::<Vec<_>>();
        for chunk in ids.chunks(500) {
            let placeholders = (1..=chunk.len())
                .map(|index| format!("?{index}"))
                .collect::<Vec<_>>()
                .join(", ");
            let mut stmt = self.conn.prepare(&format!(
                "SELECT id FROM projects WHERE id IN ({placeholders})"
            ))?;
            let rows = stmt
                .query_map(rusqlite::params_from_iter(chunk.iter().copied()), |row| {
                    row.get::<_, String>(0)
                })?;
            found.extend(rows.collect::<rusqlite::Result<Vec<_>>>()?);
        }
        if let Some(missing) = project_ids.iter().find(|id| !found.contains(*id)) {
            return Err(Error::NotFound(missing.clone()));
        }
        Ok(())
    }

    fn replace_collection_members(
        &self,
        collection_id: &str,
        desired: &HashSet<String>,
        timestamp: &str,
    ) -> Result<()> {
        let existing = {
            let mut stmt = self.conn.prepare(
                "SELECT project_id FROM project_collection_members WHERE collection_id = ?1",
            )?;
            let rows = stmt
                .query_map([collection_id], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<HashSet<_>>>()?;
            rows
        };
        let mut remove = self.conn.prepare(
            "DELETE FROM project_collection_members WHERE collection_id = ?1 AND project_id = ?2",
        )?;
        for project_id in existing.difference(desired) {
            remove.execute(params![collection_id, project_id])?;
        }
        let mut insert = self.conn.prepare(
            "INSERT OR IGNORE INTO project_collection_members (collection_id, project_id, created_at) VALUES (?1, ?2, ?3)",
        )?;
        for project_id in desired.difference(&existing) {
            insert.execute(params![collection_id, project_id, timestamp])?;
        }
        Ok(())
    }

    pub fn search_projects(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
        let mut scores: HashMap<String, i64> = HashMap::new();
        if let Some(unicode_query) = unicode_match_query(query) {
            self.accumulate_fts("project_fts_unicode", &unicode_query, 4, &mut scores)?;
        }
        if let Some(trigram_query) = trigram_match_query(query) {
            self.accumulate_fts("project_fts_trigram", &trigram_query, 5, &mut scores)?;
        }
        let scored: Vec<(String, i64)> = scores.into_iter().collect();
        let ids: Vec<String> = scored.iter().map(|(id, _)| id.clone()).collect();
        let projects = self.load_projects_by_ids(&ids)?;
        let mut hits = scored
            .into_iter()
            .filter_map(|(id, score)| {
                projects
                    .get(&id)
                    .cloned()
                    .map(|project| SearchHit { project, score })
            })
            .collect::<Vec<_>>();
        hits.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.project.display_name.cmp(&b.project.display_name))
        });
        hits.truncate(limit);
        if hits.is_empty() {
            let needle = query.to_lowercase();
            hits = self
                .load_projects()?
                .into_iter()
                .filter_map(|project| {
                    let blob = format!(
                        "{} {} {} {} {} {}",
                        project.display_name,
                        project.canonical_path,
                        project.description.clone().unwrap_or_default(),
                        project.languages.join(" "),
                        project.frameworks.join(" "),
                        project.tags.join(" ")
                    )
                    .to_lowercase();
                    if blob.contains(&needle) {
                        Some(SearchHit { score: 1, project })
                    } else {
                        None
                    }
                })
                .take(limit)
                .collect();
        }
        Ok(hits)
    }

    pub fn get_project(&self, id: &str) -> Result<ProjectDetail> {
        let row = self
            .conn
            .query_row(
                r#"
                SELECT id, canonical_path, display_name, detected_name, notes, description, vcs_kind, availability,
                       archived, favorite, origin, scan_root_id, languages_json, frameworks_json,
                       package_managers_json, facts_json, tasks_json, dependencies_json, git_json,
                       readme_path, readme_excerpt, source_mtime, last_commit_at, last_opened_at, updated_at
                FROM projects WHERE id = ?1
                "#,
                params![id],
                |row| Ok(row_to_detail(row)),
            )
            .optional()?
            .ok_or_else(|| Error::NotFound(id.into()))?;
        let mut detail = row?;
        for fact in &mut detail.facts {
            if fact.kind == "lineage" {
                fact.value = git::sanitize_remote_url(&fact.value);
            }
        }
        detail.modules = self.list_modules(id)?;
        detail.project.directory_group = self.is_directory_group(id)?;
        detail.project.tags = self.tags_for(&detail.project.id)?;
        detail.start_here = start_here_tasks(&detail.tasks, &detail.facts);
        detail.lineage = self.lineage_for(&detail)?;
        detail.recent_events = self.list_project_events(&detail.project.id, 5)?;
        Ok(detail)
    }

    pub fn task_for_start(
        &self,
        project_id: &str,
        task_id: &str,
    ) -> Result<(String, TaskDefinition)> {
        let (canonical_path, tasks_json) = self
            .conn
            .query_row(
                "SELECT canonical_path, tasks_json FROM projects WHERE id = ?1",
                [project_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
            .ok_or_else(|| Error::NotFound(project_id.into()))?;
        let task = from_json::<Vec<TaskDefinition>>(tasks_json)?
            .into_iter()
            .find(|task| task.id == task_id)
            .ok_or_else(|| Error::NotFound(task_id.into()))?;
        Ok((canonical_path, task))
    }

    pub fn read_project_readme(&self, project_id: &str) -> Result<ReadmeDocument> {
        let relative = self
            .conn
            .query_row(
                "SELECT readme_path FROM projects WHERE id = ?1",
                [project_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .ok_or_else(|| Error::NotFound(project_id.into()))?
            .ok_or_else(|| Error::msg("README not found"))?;
        self.read_project_document(project_id, &relative)
    }

    pub fn read_project_document(
        &self,
        project_id: &str,
        relative_path: &str,
    ) -> Result<ReadmeDocument> {
        const MAX_DOCUMENT_BYTES: usize = 1024 * 1024;
        let project_root = self.project_path(project_id)?;
        let relative = Path::new(relative_path);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| matches!(component, Component::ParentDir))
            || relative.as_os_str().is_empty()
        {
            return Err(Error::msg("document path is outside the project"));
        }
        if relative.components().count() != 1 {
            return Err(Error::msg("only project-root documents can be read"));
        }
        let allowed = [
            "README.md",
            "Readme.md",
            "readme.md",
            "README.MD",
            "README",
            "AGENTS.md",
            "Agents.md",
            "agents.md",
        ];
        let file_name = relative
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if !allowed
            .iter()
            .any(|name| name.eq_ignore_ascii_case(file_name))
        {
            return Err(Error::msg("unsupported project document"));
        }
        let file = environment::open_regular_project_file(&project_root, relative)?;
        let mut bytes = Vec::with_capacity(MAX_DOCUMENT_BYTES + 1);
        file.take((MAX_DOCUMENT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        let truncated = bytes.len() > MAX_DOCUMENT_BYTES;
        bytes.truncate(MAX_DOCUMENT_BYTES);
        Ok(ReadmeDocument {
            path: file_name.to_string(),
            content: String::from_utf8_lossy(&bytes).into_owned(),
            truncated,
        })
    }

    pub fn inspect_project_environment(
        &self,
        project_id: &str,
    ) -> Result<crate::models::EnvironmentInspection> {
        let detail = self.get_project(project_id)?;
        let inspection = environment::inspect_environment(&detail);
        self.persist_environment_observation(&inspection)?;
        Ok(inspection)
    }

    pub fn record_environment_inspection(
        &self,
        inspection: &crate::models::EnvironmentInspection,
    ) -> Result<()> {
        self.persist_environment_observation(inspection)
    }

    pub fn get_project_brief(&self, project_id: &str) -> Result<ProjectBrief> {
        let detail = self.get_project(project_id)?;
        let environment = environment::inspect_environment(&detail);
        let mut recent_runs = self.list_task_runs(project_id)?;
        recent_runs.truncate(10);
        let recent_activity = self.list_project_events(project_id, 12)?;
        Ok(ProjectBrief {
            modules: detail.modules,
            project: detail.project,
            facts: detail.facts,
            environment,
            git: detail.git,
            readme_path: detail.readme_path,
            project_files: detail.project_files,
            start_here: detail.start_here,
            tasks: detail.tasks,
            recent_runs,
            recent_activity,
        })
    }

    fn persist_environment_observation(
        &self,
        inspection: &crate::models::EnvironmentInspection,
    ) -> Result<()> {
        let inspection_json = serde_json::to_string(inspection).map_err(|error| {
            Error::msg(format!(
                "could not serialize environment inspection: {error}"
            ))
        })?;
        let mut conditions = inspection
            .runtimes
            .iter()
            .filter(|runtime| matches!(runtime.match_state.as_str(), "mismatch" | "missing"))
            .map(|runtime| {
                format!(
                    "{}:{}:{}:{}:{}:{}",
                    runtime.ecosystem,
                    runtime.label,
                    runtime.constraint.as_deref().unwrap_or_default(),
                    runtime.source.as_deref().unwrap_or_default(),
                    runtime.local_version.as_deref().unwrap_or_default(),
                    runtime.match_state
                )
            })
            .collect::<Vec<_>>();
        conditions.sort();
        let condition_version = stable_fingerprint(&conditions.join("\n"));
        self.conn.execute(
            "INSERT INTO environment_observations (project_id, inspection_json, observed_at, condition_version) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(project_id) DO UPDATE SET inspection_json = excluded.inspection_json, observed_at = excluded.observed_at, condition_version = excluded.condition_version",
            params![inspection.project_id, inspection_json, now(), condition_version],
        )?;
        Ok(())
    }

    pub fn read_project_icons(
        &self,
        project_ids: &[String],
    ) -> Result<Vec<crate::models::ProjectIcon>> {
        // Virtualized lists request only the currently visible rows. Keep this
        // endpoint genuinely batched: resolving every icon through
        // `get_project` would also run lineage/event queries for every row and
        // turn a 48-item viewport into an avoidable N+1 database workload.
        let requested: Vec<String> = project_ids.iter().take(48).cloned().collect();
        if requested.is_empty() {
            return Ok(Vec::new());
        }
        let mut unique_ids = Vec::new();
        let mut seen = HashSet::new();
        for id in &requested {
            if seen.insert(id.clone()) {
                unique_ids.push(id.clone());
            }
        }
        let placeholders = (1..=unique_ids.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT id, canonical_path, languages_json, facts_json FROM projects WHERE id IN ({placeholders})"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(unique_ids.iter()), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut projects = HashMap::with_capacity(rows.len());
        for (id, path, languages_json, facts_json) in rows {
            let languages: Vec<String> = from_json(languages_json)?;
            let facts: Vec<crate::models::DetectedFact> = from_json(facts_json)?;
            projects.insert(id, (path, languages, facts));
        }
        if projects.len() != unique_ids.len() {
            let missing = unique_ids
                .iter()
                .find(|id| !projects.contains_key(*id))
                .cloned()
                .unwrap_or_default();
            return Err(Error::NotFound(missing));
        }

        let mut overrides = HashMap::<String, IconOverride>::new();
        let override_sql = format!(
            "SELECT project_id, mime_type, bytes, source_name FROM project_icon_overrides WHERE project_id IN ({placeholders})"
        );
        let mut override_stmt = self.conn.prepare(&override_sql)?;
        let override_rows =
            override_stmt.query_map(rusqlite::params_from_iter(unique_ids.iter()), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    (
                        row.get::<_, String>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                        row.get(3)?,
                    ),
                ))
            })?;
        for row in override_rows {
            let (id, icon) = row?;
            overrides.insert(id, icon);
        }

        requested
            .into_iter()
            .map(|id| {
                let (path, languages, facts) = projects
                    .get(&id)
                    .ok_or_else(|| Error::NotFound(id.clone()))?;
                if let Some((mime, bytes, source)) = overrides.get(&id) {
                    return Ok(environment::project_icon_from_bytes(
                        &id,
                        mime,
                        bytes,
                        source.clone(),
                    ));
                }
                Ok(environment::read_project_icon(
                    &id,
                    Path::new(path),
                    facts,
                    languages,
                ))
            })
            .collect()
    }

    pub fn set_project_icon(
        &self,
        project_id: &str,
        source_path: impl AsRef<Path>,
    ) -> Result<crate::models::ProjectIcon> {
        let source_path = source_path.as_ref();
        let (mime, bytes) = environment::load_icon_override(source_path)?;
        let source_name = source_path
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::to_string);
        self.set_project_icon_inner(project_id, &mime, &bytes, source_name)
    }

    pub fn set_project_icon_with_origin(
        &self,
        project_id: &str,
        source_path: impl AsRef<Path>,
        origin: &str,
    ) -> Result<crate::models::ProjectIcon> {
        let source_path = source_path.as_ref();
        let (mime, bytes) = environment::load_icon_override(source_path)?;
        let source_name = source_path
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::to_string);
        let origin = origin.to_owned();
        self.with_immediate_transaction(|core| {
            let icon = core.set_project_icon_inner(project_id, &mime, &bytes, source_name)?;
            core.record_audit_event(
                &origin,
                "set_project_icon",
                "project",
                Some(project_id),
                Some("icon path supplied"),
                "success",
            )?;
            Ok(icon)
        })
    }

    fn set_project_icon_inner(
        &self,
        project_id: &str,
        mime: &str,
        bytes: &[u8],
        source_name: Option<String>,
    ) -> Result<crate::models::ProjectIcon> {
        self.get_project(project_id)?;
        self.conn.execute(
            "INSERT INTO project_icon_overrides (project_id, mime_type, bytes, source_name, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(project_id) DO UPDATE SET mime_type = excluded.mime_type, bytes = excluded.bytes,
             source_name = excluded.source_name, updated_at = excluded.updated_at",
            params![project_id, mime, bytes, source_name, now()],
        )?;
        self.conn.execute(
            "UPDATE projects SET updated_at = ?1 WHERE id = ?2",
            params![now(), project_id],
        )?;
        Ok(environment::project_icon_from_bytes(
            project_id,
            mime,
            bytes,
            source_name,
        ))
    }

    pub fn clear_project_icon(&self, project_id: &str) -> Result<crate::models::ProjectIcon> {
        self.clear_project_icon_inner(project_id)
    }

    pub fn clear_project_icon_with_origin(
        &self,
        project_id: &str,
        origin: &str,
    ) -> Result<crate::models::ProjectIcon> {
        let origin = origin.to_owned();
        self.with_immediate_transaction(|core| {
            let icon = core.clear_project_icon_inner(project_id)?;
            core.record_audit_event(
                &origin,
                "clear_project_icon",
                "project",
                Some(project_id),
                None,
                "success",
            )?;
            Ok(icon)
        })
    }

    fn clear_project_icon_inner(&self, project_id: &str) -> Result<crate::models::ProjectIcon> {
        let detail = self.get_project(project_id)?;
        self.conn.execute(
            "DELETE FROM project_icon_overrides WHERE project_id = ?1",
            params![project_id],
        )?;
        self.conn.execute(
            "UPDATE projects SET updated_at = ?1 WHERE id = ?2",
            params![now(), project_id],
        )?;
        Ok(environment::read_project_icon(
            project_id,
            Path::new(&detail.project.canonical_path),
            &detail.facts,
            &detail.project.languages,
        ))
    }

    pub fn read_project_file(
        &self,
        project_id: &str,
        relative_path: &str,
    ) -> Result<ReadmeDocument> {
        let detail = self.get_project(project_id)?;
        environment::read_detected_file(
            Path::new(&detail.project.canonical_path),
            relative_path,
            &detail.project_files,
        )
    }

    pub fn resolve_project_file(&self, project_id: &str, relative_path: &str) -> Result<PathBuf> {
        let detail = self.get_project(project_id)?;
        environment::resolve_project_file(
            Path::new(&detail.project.canonical_path),
            relative_path,
            &detail.project_files,
        )
    }

    pub fn register_project(&self, path: impl AsRef<Path>) -> Result<ProjectSummary> {
        self.register_project_inner(path)
    }

    /// Register a Project and its audit event in one metadata transaction.
    ///
    /// Canonicalization and discovery happen before/inside the same operation,
    /// but no project files are modified. If the audit row cannot be written,
    /// the project record is rolled back with the mutation.
    pub fn register_project_with_origin(
        &self,
        path: impl AsRef<Path>,
        origin: &str,
    ) -> Result<ProjectSummary> {
        let canonical = paths::canonicalize(path.as_ref())?;
        if !canonical.is_dir() {
            return Err(Error::msg("path is not a directory"));
        }
        let origin = origin.to_owned();
        self.with_immediate_transaction(|core| {
            let scan_root_id = core.matching_scan_root(&canonical)?;
            if let Some((owner, module_id)) = core.module_at_path(&canonical)? {
                core.promote_module_inner(
                    &owner,
                    &module_id,
                    &origin,
                    &paths::path_to_string(&canonical),
                )?;
            } else {
                core.upsert_project(&canonical, scan_root_id.as_deref(), "manual")?;
            }
            let project = core.project_by_path(&canonical)?;
            core.discover_modules(&project.id)?;
            core.record_audit_event(
                &origin,
                "register_project",
                "project",
                Some(&project.id),
                Some("path supplied"),
                "success",
            )?;
            core.project_by_path(&canonical)
        })
    }

    fn register_project_inner(&self, path: impl AsRef<Path>) -> Result<ProjectSummary> {
        let canonical = paths::canonicalize(path.as_ref())?;
        if !canonical.is_dir() {
            return Err(Error::msg("path is not a directory"));
        }
        if let Some((owner, module_id)) = self.module_at_path(&canonical)? {
            return self.promote_module_with_origin(&owner, &module_id, "local");
        }
        let scan_root_id = self.matching_scan_root(&canonical)?;
        self.upsert_project(&canonical, scan_root_id.as_deref(), "manual")?;
        let project = self.project_by_path(&canonical)?;
        self.discover_modules(&project.id)?;
        self.project_by_path(&canonical)
    }

    pub fn update_project(&self, id: &str, patch: ProjectPatch) -> Result<ProjectSummary> {
        self.update_project_inner(id, patch)
    }

    /// Update user-owned Project metadata and persist its audit event
    /// atomically. `ProjectPatch` is resolved from the current record inside
    /// the transaction so a failed audit insert cannot leak a partial patch.
    pub fn update_project_with_origin(
        &self,
        id: &str,
        patch: ProjectPatch,
        origin: &str,
    ) -> Result<ProjectSummary> {
        let origin = origin.to_owned();
        self.with_immediate_transaction(|core| {
            let project = core.update_project_inner(id, patch)?;
            core.record_audit_event(
                &origin,
                "update_project",
                "project",
                Some(id),
                None,
                "success",
            )?;
            Ok(project)
        })
    }

    fn update_project_inner(&self, id: &str, patch: ProjectPatch) -> Result<ProjectSummary> {
        let existing = self.get_project(id)?;
        let display_name = patch.display_name.unwrap_or(existing.project.display_name);
        if display_name.trim().is_empty() {
            return Err(Error::msg("project display name is required"));
        }
        let notes = match patch.notes {
            Some(value) if value.trim().is_empty() => None,
            Some(value) => Some(value),
            None => existing.project.notes,
        };
        let description = match patch.description {
            Some(description) => description,
            None => existing.project.description,
        };
        let favorite = patch.favorite.unwrap_or(existing.project.favorite);
        let archived = patch.archived.unwrap_or(existing.project.archived);
        let tasks = patch
            .tasks
            .map(|mut tasks| -> Result<Vec<TaskDefinition>> {
                let mut ids = HashSet::with_capacity(tasks.len());
                for task in &mut tasks {
                    if task.id.trim().is_empty()
                        || task.name.trim().is_empty()
                        || task.executable.trim().is_empty()
                    {
                        return Err(Error::msg(
                            "each task requires a unique id, name, and executable",
                        ));
                    }
                    if task.shell_mode {
                        return Err(Error::msg("shell-mode tasks are not enabled"));
                    }
                    if !ids.insert(task.id.clone()) {
                        return Err(Error::msg("task ids must be unique"));
                    }
                    normalize_task_runtime_metadata(task)?;
                }
                Ok(tasks)
            })
            .transpose()?;
        self.conn.execute(
            "UPDATE projects SET display_name = ?1, notes = ?2, description = ?3, favorite = ?4, archived = ?5, updated_at = ?6 WHERE id = ?7",
            params![display_name, notes, description, favorite as i64, archived as i64, now(), id],
        )?;
        if let Some(tags) = patch.tags {
            self.replace_tags(id, &tags)?;
        }
        if let Some(tasks) = tasks {
            self.conn.execute(
                "UPDATE projects SET tasks_json = ?1, updated_at = ?2 WHERE id = ?3",
                params![to_json(&tasks)?, now(), id],
            )?;
        }
        self.reindex_project(id)?;
        self.record_project_event(id, "metadata", "Updated project metadata", None)?;
        self.get_project_summary(id)?
            .ok_or_else(|| Error::NotFound(id.into()))
    }

    pub fn add_task(&self, project_id: &str, task: TaskDefinition) -> Result<ProjectDetail> {
        self.add_task_inner(project_id, task)
    }

    pub fn add_task_with_origin(
        &self,
        project_id: &str,
        task: TaskDefinition,
        origin: &str,
    ) -> Result<ProjectDetail> {
        let target = if task.id.trim().is_empty() {
            project_id.to_owned()
        } else {
            task.id.clone()
        };
        let origin = origin.to_owned();
        self.with_immediate_transaction(|core| {
            let detail = core.add_task_inner(project_id, task)?;
            core.record_audit_event(
                &origin,
                "add_task",
                "task",
                Some(&target),
                Some(project_id),
                "success",
            )?;
            Ok(detail)
        })
    }

    fn add_task_inner(&self, project_id: &str, mut task: TaskDefinition) -> Result<ProjectDetail> {
        let mut detail = self.get_project(project_id)?;
        if task.shell_mode {
            return Err(Error::msg("shell-mode tasks are not enabled"));
        }
        if task.name.trim().is_empty() {
            return Err(Error::msg("task name is required"));
        }
        if task.executable.trim().is_empty() {
            return Err(Error::msg("task executable is required"));
        }
        normalize_task_runtime_metadata(&mut task)?;
        let mut next = task;
        if next.id.trim().is_empty() {
            next.id = uuid::Uuid::new_v4().to_string();
        }
        if next.kind.trim().is_empty() {
            next.kind = "run".into();
        }
        next.inferred = false;
        if next
            .description
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            next.description = None;
        }
        detail.tasks.retain(|existing| existing.id != next.id);
        detail.tasks.push(next);
        self.update_project_inner(
            project_id,
            ProjectPatch {
                tasks: Some(detail.tasks),
                ..ProjectPatch::default()
            },
        )?;
        self.get_project(project_id)
    }

    pub fn update_task(
        &self,
        project_id: &str,
        task_id: &str,
        task: TaskDefinition,
    ) -> Result<ProjectDetail> {
        self.update_task_inner(project_id, task_id, task)
    }

    pub fn update_task_with_origin(
        &self,
        project_id: &str,
        task_id: &str,
        task: TaskDefinition,
        origin: &str,
    ) -> Result<ProjectDetail> {
        let origin = origin.to_owned();
        self.with_immediate_transaction(|core| {
            let detail = core.update_task_inner(project_id, task_id, task)?;
            core.record_audit_event(
                &origin,
                "update_task",
                "task",
                Some(task_id),
                Some(project_id),
                "success",
            )?;
            Ok(detail)
        })
    }

    fn update_task_inner(
        &self,
        project_id: &str,
        task_id: &str,
        mut task: TaskDefinition,
    ) -> Result<ProjectDetail> {
        let mut detail = self.get_project(project_id)?;
        if task.shell_mode {
            return Err(Error::msg("shell-mode tasks are not enabled"));
        }
        if task.name.trim().is_empty() || task.executable.trim().is_empty() {
            return Err(Error::msg("task name and executable are required"));
        }
        normalize_task_runtime_metadata(&mut task)?;
        if !detail.tasks.iter().any(|existing| existing.id == task_id) {
            return Err(Error::NotFound(task_id.into()));
        }
        task.id = task_id.into();
        task.inferred = false;
        if task
            .description
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            task.description = None;
        }
        detail.tasks = detail
            .tasks
            .into_iter()
            .map(|existing| {
                if existing.id == task_id {
                    task.clone()
                } else {
                    existing
                }
            })
            .collect();
        self.update_project_inner(
            project_id,
            ProjectPatch {
                tasks: Some(detail.tasks),
                ..ProjectPatch::default()
            },
        )?;
        self.get_project(project_id)
    }

    pub fn remove_task(&self, project_id: &str, task_id: &str) -> Result<ProjectDetail> {
        self.remove_task_inner(project_id, task_id)
    }

    pub fn remove_task_with_origin(
        &self,
        project_id: &str,
        task_id: &str,
        origin: &str,
    ) -> Result<ProjectDetail> {
        let origin = origin.to_owned();
        self.with_immediate_transaction(|core| {
            let detail = core.remove_task_inner(project_id, task_id)?;
            core.record_audit_event(
                &origin,
                "remove_task",
                "task",
                Some(task_id),
                Some(project_id),
                "success",
            )?;
            Ok(detail)
        })
    }

    fn remove_task_inner(&self, project_id: &str, task_id: &str) -> Result<ProjectDetail> {
        let mut detail = self.get_project(project_id)?;
        let before = detail.tasks.len();
        detail.tasks.retain(|task| task.id != task_id);
        if before == detail.tasks.len() {
            return Err(Error::NotFound(task_id.into()));
        }
        self.update_project_inner(
            project_id,
            ProjectPatch {
                tasks: Some(detail.tasks),
                ..ProjectPatch::default()
            },
        )?;
        self.get_project(project_id)
    }

    pub fn mark_opened(&self, id: &str) -> Result<ProjectSummary> {
        self.mark_opened_inner(id)
    }

    pub fn mark_opened_with_origin(&self, id: &str, origin: &str) -> Result<ProjectSummary> {
        let origin = origin.to_owned();
        self.with_immediate_transaction(|core| {
            let project = core.mark_opened_inner(id)?;
            core.record_audit_event(
                &origin,
                "mark_project_opened",
                "project",
                Some(id),
                None,
                "success",
            )?;
            Ok(project)
        })
    }

    fn mark_opened_inner(&self, id: &str) -> Result<ProjectSummary> {
        self.conn.execute(
            "UPDATE projects SET last_opened_at = ?1, updated_at = ?1 WHERE id = ?2",
            params![now(), id],
        )?;
        self.record_open_activity(id)?;
        self.get_project_summary(id)?
            .ok_or_else(|| Error::NotFound(id.into()))
    }

    /// Remove a project record while preserving the real project directory.
    ///
    /// This compatibility entry point does not add an audit event. Use
    /// [`Self::remove_project_with_origin`] for externally-originated
    /// mutations that need an atomic audit record.
    pub fn remove_project(&self, id: &str) -> Result<ProjectRemoval> {
        self.remove_project_with_optional_origin(id, None)
    }

    /// Remove a project record and persist the caller origin in the same
    /// SQLite transaction as the removal. A failure to insert the audit event
    /// rolls back the project deletion, so callers never observe a successful
    /// mutation without its corresponding audit trail.
    pub fn remove_project_with_origin(&self, id: &str, origin: &str) -> Result<ProjectRemoval> {
        self.remove_project_with_optional_origin(id, Some(origin))
    }

    fn remove_project_with_optional_origin(
        &self,
        id: &str,
        origin: Option<&str>,
    ) -> Result<ProjectRemoval> {
        let origin = origin.map(str::to_owned);
        let mut removal = self.with_immediate_transaction(|core| {
            let removal = core.remove_project_inner(id)?;
            if let Some(origin) = origin.as_deref() {
                core.record_audit_event(
                    origin,
                    "remove_project",
                    "project",
                    Some(id),
                    Some("RepoAtlas records removed; filesystem preserved"),
                    "success",
                )?;
            }
            Ok(removal)
        })?;
        // Project rows are gone after COMMIT, but queued broker logs are
        // intentionally retained until they can be safely removed. Report
        // the post-retry queue size without failing the already-committed
        // metadata deletion.
        let _ = self.retry_managed_file_cleanup();
        removal.pending_log_cleanup = self.pending_log_cleanup_count(id).unwrap_or(0);
        Ok(removal)
    }

    /// Remove a set of project records and optionally their nested Scan Root
    /// records as one metadata transaction. This is used by the desktop
    /// folder context action so a failure on one project cannot leave the
    /// folder half-removed. No filesystem operation is performed.
    pub fn remove_folder_records(
        &self,
        project_ids: &[String],
        scan_root_ids: &[String],
    ) -> Result<Vec<ProjectRemoval>> {
        self.remove_folder_records_with_optional_origin(project_ids, scan_root_ids, None)
    }

    /// Remove a folder's Project and Scan Root records and persist one audit
    /// event in the same SQLite transaction. This only changes RepoAtlas
    /// metadata; it never deletes or edits the corresponding directories.
    pub fn remove_folder_records_with_origin(
        &self,
        project_ids: &[String],
        scan_root_ids: &[String],
        origin: &str,
    ) -> Result<Vec<ProjectRemoval>> {
        self.remove_folder_records_with_optional_origin(project_ids, scan_root_ids, Some(origin))
    }

    fn remove_folder_records_with_optional_origin(
        &self,
        project_ids: &[String],
        scan_root_ids: &[String],
        origin: Option<&str>,
    ) -> Result<Vec<ProjectRemoval>> {
        let mut unique_projects = Vec::new();
        let mut seen_projects = HashSet::new();
        for id in project_ids {
            if seen_projects.insert(id.clone()) {
                unique_projects.push(id.clone());
            }
        }
        let mut unique_roots = Vec::new();
        let mut seen_roots = HashSet::new();
        for id in scan_root_ids {
            if seen_roots.insert(id.clone()) {
                unique_roots.push(id.clone());
            }
        }
        let mut removals = self.with_immediate_transaction(|core| {
            core.ensure_no_running_projects(&unique_projects)?;
            for root_id in &unique_roots {
                core.get_scan_root(root_id)?;
            }
            let mut removals = Vec::with_capacity(unique_projects.len());
            for project_id in &unique_projects {
                removals.push(core.remove_project_inner(project_id)?);
            }
            // Removing a folder record revokes nested Scan Root authorizations
            // but preserves any project record not included by the caller as
            // an orphan, matching the standalone `orphan` policy.
            for root_id in &unique_roots {
                core.conn.execute(
                    "UPDATE projects SET scan_root_id = NULL, origin = CASE WHEN origin = 'scan' THEN 'orphaned' ELSE origin END WHERE scan_root_id = ?1",
                    params![root_id],
                )?;
                core.conn
                    .execute("DELETE FROM scan_roots WHERE id = ?1", params![root_id])?;
            }
            if let Some(origin) = origin {
                let detail = format!(
                    "removed {} Project records and {} Scan Root records; filesystem preserved",
                    removals.len(),
                    unique_roots.len()
                );
                core.record_audit_event(
                    origin,
                    "remove_folder_records",
                    "folder",
                    None,
                    Some(&detail),
                    "success",
                )?;
            }
            Ok(removals)
        })?;
        let _ = self.retry_managed_file_cleanup();
        for removal in &mut removals {
            removal.pending_log_cleanup = self
                .pending_log_cleanup_count(&removal.project.id)
                .unwrap_or(0);
        }
        Ok(removals)
    }

    fn remove_project_inner(&self, id: &str) -> Result<ProjectRemoval> {
        let detail = self.get_project(id)?;
        let project = detail.project.clone();
        let running = self.count_project_runs(id, Some("active"))?;
        if running > 0 {
            return Err(Error::msg(
                "project has running or starting tasks; stop them before removal",
            ));
        }
        let starting_approvals: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM pending_approvals WHERE project_id = ?1 AND status = 'starting'",
            params![id],
            |row| row.get(0),
        )?;
        if starting_approvals > 0 {
            return Err(Error::msg(
                "project has a starting task approval; wait for it to finish before removal",
            ));
        }
        let removed_tags = self.count_rows("project_tags", id)?;
        let removed_icons = self.count_rows("project_icon_overrides", id)?;
        let removed_task_runs = self.count_rows("task_runs", id)?;
        let removed_memory_items = self.count_rows("ai_memory", id)?;
        let removed_summaries = self.count_rows("ai_summaries", id)?;
        let removed_events = self.count_rows("project_events", id)?;
        let removed_pending_approvals = self.count_rows("pending_approvals", id)?;
        let removed_audit_events = self.count_project_audit_events(id)?;
        let removed_conversation = self.conn.query_row(
            "SELECT COUNT(*) FROM project_conversations WHERE project_id = ?1",
            params![id],
            |row| row.get::<_, i64>(0),
        )? > 0;
        self.queue_task_log_cleanup(id)?;
        self.delete_project_record(id)?;
        let pending_log_cleanup = self.pending_log_cleanup_count(id)?;
        Ok(ProjectRemoval {
            project,
            removed_tags,
            removed_icons,
            removed_task_runs,
            removed_memory_items,
            removed_summaries,
            removed_conversation,
            removed_events,
            removed_pending_approvals,
            removed_audit_events,
            filesystem_deleted: false,
            pending_log_cleanup,
        })
    }

    pub fn relocate_project(&self, id: &str, new_path: impl AsRef<Path>) -> Result<ProjectSummary> {
        self.relocate_project_inner(id, new_path)
    }

    /// Relocate the managed Project record and persist its audit event as one
    /// metadata transaction. The target is only adopted; RepoAtlas never
    /// moves or deletes either directory.
    pub fn relocate_project_with_origin(
        &self,
        id: &str,
        new_path: impl AsRef<Path>,
        origin: &str,
    ) -> Result<ProjectSummary> {
        let canonical = paths::canonicalize(new_path.as_ref())?;
        if !canonical.is_dir() {
            return Err(Error::msg(
                "relocation target must be an existing directory",
            ));
        }
        let origin = origin.to_owned();
        self.with_immediate_transaction(|core| {
            let relocated = core.relocate_project_from_canonical(id, &canonical)?;
            core.record_audit_event(
                &origin,
                "relocate_project",
                "project",
                Some(id),
                Some("path supplied"),
                "success",
            )?;
            Ok(relocated)
        })
    }

    fn relocate_project_inner(
        &self,
        id: &str,
        new_path: impl AsRef<Path>,
    ) -> Result<ProjectSummary> {
        let canonical = paths::canonicalize(new_path.as_ref())?;
        if !canonical.is_dir() {
            return Err(Error::msg(
                "relocation target must be an existing directory",
            ));
        }
        self.with_immediate_transaction(|core| core.relocate_project_from_canonical(id, &canonical))
    }

    fn relocate_project_from_canonical(
        &self,
        id: &str,
        canonical: &Path,
    ) -> Result<ProjectSummary> {
        self.get_project(id)?;
        let canonical_path = paths::path_to_string(canonical);
        let conflicting_id: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM projects WHERE canonical_path = ?1 AND id <> ?2 LIMIT 1",
                params![canonical_path, id],
                |row| row.get(0),
            )
            .optional()?;
        if conflicting_id.is_some() {
            return Err(Error::msg("relocation target is already registered"));
        }
        let scan_root_id = self.matching_scan_root(canonical)?;
        self.conn.execute(
            "UPDATE projects SET canonical_path = ?1, availability = 'ready', scan_root_id = ?2, origin = 'manual', updated_at = ?3 WHERE id = ?4",
            params![canonical_path, scan_root_id, now(), id],
        )?;
        let relocated = self.refresh_project_inner(id)?;
        self.record_project_event(
            id,
            "relocate",
            "Relocated project",
            Some(&relocated.canonical_path),
        )?;
        Ok(relocated)
    }

    pub fn refresh_project(&self, id: &str) -> Result<ProjectSummary> {
        self.refresh_project_inner(id)
    }

    /// Refresh a Project from its current directory and commit its audit row
    /// with the refreshed metadata. The directory itself remains untouched.
    pub fn refresh_project_with_origin(&self, id: &str, origin: &str) -> Result<ProjectSummary> {
        let origin = origin.to_owned();
        self.with_immediate_transaction(|core| {
            let project = core.refresh_project_inner(id)?;
            core.record_audit_event(
                &origin,
                "refresh_project",
                "project",
                Some(id),
                None,
                "success",
            )?;
            Ok(project)
        })
    }

    fn refresh_project_inner(&self, id: &str) -> Result<ProjectSummary> {
        let detail = self.get_project(id)?;
        let path = PathBuf::from(&detail.project.canonical_path);
        if !path_exists(&path) {
            self.conn.execute(
                "UPDATE projects SET availability = 'unavailable', updated_at = ?1 WHERE id = ?2",
                params![now(), id],
            )?;
        } else {
            self.upsert_project(
                &path,
                detail.project.scan_root_id.as_deref(),
                &detail.project.origin,
            )?;
        }
        if path.is_dir() {
            self.discover_modules(id)?;
        }
        self.record_project_event(id, "refresh", "Refreshed project", None)?;
        self.get_project_summary(id)?
            .ok_or_else(|| Error::NotFound(id.into()))
    }

    pub fn scan_root_path(&self, root_id: &str) -> Result<String> {
        Ok(self.get_scan_root(root_id)?.path)
    }

    /// Resolve a folder refresh to existing authorizations, without adding a
    /// Scan Root or walking siblings outside the selected folder.
    pub fn folder_scan_targets(&self, folder: &Path) -> Result<Vec<(String, String)>> {
        for ancestor in folder.ancestors() {
            if crate::scan::is_link(&std::fs::symlink_metadata(ancestor)?) {
                return Err(Error::msg("folder scan does not follow symbolic links"));
            }
        }
        let folder = paths::canonicalize(folder)?;
        if !folder.is_dir() {
            return Err(Error::msg("folder scan requires a directory"));
        }
        let mut roots = self.list_scan_roots()?;
        roots.sort_by_key(|root| std::cmp::Reverse(root.path.len()));
        if let Some(root) = roots
            .iter()
            .find(|root| paths::is_within(&folder, Path::new(&root.path)))
        {
            return Ok(vec![(root.id.clone(), paths::path_to_string(&folder))]);
        }
        roots.retain(|root| paths::is_within(Path::new(&root.path), &folder));
        roots.sort_by_key(|root| root.path.len());
        let mut targets: Vec<(String, String)> = Vec::new();
        for root in roots {
            if !targets
                .iter()
                .any(|(_, parent)| paths::is_within(Path::new(&root.path), Path::new(parent)))
            {
                targets.push((root.id, root.path));
            }
        }
        if targets.is_empty() {
            return Err(Error::msg(
                "folder is not covered by an authorized Scan Root",
            ));
        }
        Ok(targets)
    }

    /// Scan an authorized Scan Root while keeping the audit lifecycle short
    /// and durable. A `started` event is committed before walking the
    /// directory; the walk itself never holds a SQLite write transaction. A
    /// second event records the terminal result. If that terminal event cannot
    /// be persisted, return an explicit error because the scan's metadata may
    /// already have been committed and cannot be rolled back safely.
    pub fn scan_root_with_origin(
        &self,
        root_id: &str,
        cancel: &AtomicBool,
        on_progress: &dyn Fn(ScanProgress),
        origin: &str,
    ) -> Result<ScanResult> {
        // Validate before creating an audit entry so an unknown root cannot
        // create a misleading lifecycle record.
        self.get_scan_root(root_id)?;
        let origin = origin.to_owned();
        self.record_audit_event(
            &origin,
            "scan_root",
            "scan_root",
            Some(root_id),
            Some("scan started"),
            "started",
        )?;

        match self.scan_root(root_id, cancel, on_progress) {
            Ok(result) => {
                let detail = format!(
                    "scanId={} visited={} discovered={} unavailable={} cancelled={}",
                    result.scan_id,
                    result.visited,
                    result.discovered,
                    result.unavailable,
                    result.cancelled
                );
                if self
                    .record_audit_event(
                        &origin,
                        "scan_root",
                        "scan_root",
                        Some(root_id),
                        Some(&detail),
                        if result.cancelled {
                            "cancelled"
                        } else {
                            "success"
                        },
                    )
                    .is_err()
                {
                    return Err(Error::msg(
                        "scan completed but its result audit could not be persisted",
                    ));
                }
                Ok(result)
            }
            Err(error) => {
                let _ = self.record_audit_event(
                    &origin,
                    "scan_root",
                    "scan_root",
                    Some(root_id),
                    Some("scan failed"),
                    "failed",
                );
                Err(error)
            }
        }
    }

    pub fn ingest_scan(
        &self,
        root_id: &str,
        discovered: &[crate::scan::DiscoveredProject],
        cancelled: bool,
    ) -> Result<u64> {
        let mut found_ids = HashSet::new();
        for item in discovered {
            found_ids.insert(self.ingest_scan_project(root_id, item)?);
        }
        self.finalize_scan_ingest(root_id, &found_ids, cancelled)
    }

    pub fn ingest_scan_project(
        &self,
        root_id: &str,
        discovered: &crate::scan::DiscoveredProject,
    ) -> Result<String> {
        let boundary = PathBuf::from(self.get_scan_root(root_id)?.path);
        self.with_immediate_transaction(|core| {
            core.ingest_directory(&discovered.path, Some(root_id), &boundary)
        })
    }

    pub fn finalize_scan_ingest(
        &self,
        root_id: &str,
        found_ids: &HashSet<String>,
        cancelled: bool,
    ) -> Result<u64> {
        let root_path = self.scan_root_path(root_id)?;
        self.finalize_folder_scan_ingest(root_id, Path::new(&root_path), found_ids, cancelled)
    }

    pub fn finalize_folder_scan_ingest(
        &self,
        root_id: &str,
        folder: &Path,
        found_ids: &HashSet<String>,
        cancelled: bool,
    ) -> Result<u64> {
        let root_path = self.scan_root_path(root_id)?;
        if !paths::is_within(folder, Path::new(&root_path)) {
            return Err(Error::msg("folder is outside the authorized Scan Root"));
        }
        if !cancelled {
            let existing: Vec<(String, String)> = {
                let mut stmt = self
                    .conn
                    .prepare("SELECT id, canonical_path FROM projects WHERE scan_root_id = ?1")?;
                let rows = stmt
                    .query_map(params![root_id], |row| Ok((row.get(0)?, row.get(1)?)))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                rows
            };
            for (id, path) in existing {
                if paths::is_within(Path::new(&path), folder)
                    && !found_ids.contains(&id)
                    && !PathBuf::from(&path).exists()
                {
                    self.conn.execute(
                        "UPDATE projects SET availability = 'unavailable', updated_at = ?1 WHERE id = ?2",
                        params![now(), id],
                    )?;
                }
            }
            if paths::is_within(Path::new(&root_path), folder) {
                self.conn.execute(
                    "UPDATE scan_roots SET last_scanned_at = ?1 WHERE id = ?2",
                    params![now(), root_id],
                )?;
            }
            for id in found_ids {
                self.update_missing_modules(id)?;
                self.record_project_event(id, "scan", "Scan refreshed project", Some(root_id))?;
            }
        }
        let mut statement = self.conn.prepare(
            "SELECT canonical_path FROM projects WHERE scan_root_id = ?1 AND availability = 'unavailable'",
        )?;
        let unavailable = statement
            .query_map(params![root_id], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(unavailable
            .iter()
            .filter(|path| paths::is_within(Path::new(path), folder))
            .count() as u64)
    }

    pub fn scan_root(
        &self,
        root_id: &str,
        cancel: &AtomicBool,
        on_progress: &dyn Fn(ScanProgress),
    ) -> Result<ScanResult> {
        let root = self.get_scan_root(root_id)?;
        let scan_id = Uuid::new_v4().to_string();
        let engine = ScanEngine {
            scan_id: scan_id.clone(),
            root: PathBuf::from(&root.path),
            cancel,
            on_progress,
        };
        on_progress(ScanProgress {
            scan_id: scan_id.clone(),
            root_path: root.path.clone(),
            phase: "started".into(),
            visited: 0,
            discovered: 0,
            current_path: Some(root.path.clone()),
            message: None,
        });
        let (discovered, visited, errors, walk_cancelled) = engine.walk()?;
        let mut found_ids = HashSet::new();
        for item in &discovered {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            found_ids.insert(self.ingest_scan_project(root_id, item)?);
        }
        let cancelled = walk_cancelled || cancel.load(std::sync::atomic::Ordering::Relaxed);
        let unavailable = self.finalize_scan_ingest(root_id, &found_ids, cancelled)?;
        on_progress(ScanProgress {
            scan_id: scan_id.clone(),
            root_path: root.path.clone(),
            phase: if cancelled { "cancelled" } else { "completed" }.into(),
            visited,
            discovered: discovered.len() as u64,
            current_path: None,
            message: None,
        });
        Ok(ScanResult {
            scan_id,
            visited,
            discovered: discovered.len() as u64,
            unavailable,
            cancelled,
            errors,
        })
    }

    pub fn scan_all(
        &self,
        cancel: &AtomicBool,
        on_progress: &dyn Fn(ScanProgress),
    ) -> Result<Vec<ScanResult>> {
        let mut results = Vec::new();
        for root in self.list_scan_roots()? {
            results.push(self.scan_root(&root.id, cancel, on_progress)?);
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
        }
        Ok(results)
    }

    pub fn connection(&self) -> &rusqlite::Connection {
        &self.conn
    }

    pub fn list_task_runs(&self, project_id: &str) -> Result<Vec<crate::models::TaskRun>> {
        crate::broker::list_runs(&self.conn, project_id)
    }

    pub fn list_active_task_runs(&self) -> Result<Vec<crate::models::TaskRun>> {
        crate::broker::list_active_runs(&self.conn)
    }

    pub fn get_task_run(&self, run_id: &str) -> Result<crate::models::TaskRun> {
        crate::broker::get_run(&self.conn, run_id)
    }

    pub fn finish_task_run(
        &self,
        run_id: &str,
        status: &str,
        exit_code: Option<i32>,
    ) -> Result<crate::models::TaskRun> {
        crate::broker::finish_run(&self.conn, run_id, status, exit_code)
    }

    pub fn read_task_log(&self, run_id: &str) -> Result<String> {
        let run = self.get_task_run(run_id)?;
        if let Some(log_dir) = &self.log_dir {
            let root = paths::canonicalize(log_dir).unwrap_or_else(|_| paths::normalize(log_dir));
            let log_path = paths::canonicalize(Path::new(&run.log_path))
                .unwrap_or_else(|_| paths::normalize(Path::new(&run.log_path)));
            if !paths::is_within(&log_path, &root) || log_path == root {
                return Err(Error::Unauthorized(
                    "task log is outside the broker log directory".into(),
                ));
            }
        }
        crate::broker::read_log(&run, 200_000)
    }

    pub fn record_project_event(
        &self,
        project_id: &str,
        kind: &str,
        title: &str,
        detail: Option<&str>,
    ) -> Result<ProjectEvent> {
        let event = ProjectEvent {
            id: Uuid::new_v4().to_string(),
            project_id: project_id.into(),
            kind: kind.into(),
            title: title.into(),
            detail: detail
                .map(str::to_string)
                .filter(|value| !value.trim().is_empty()),
            created_at: now(),
        };
        self.conn.execute(
            "INSERT INTO project_events (id, project_id, kind, title, detail, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![event.id, event.project_id, event.kind, event.title, event.detail, event.created_at],
        )?;
        Ok(event)
    }

    fn record_open_activity(&self, id: &str) -> Result<Option<ProjectEvent>> {
        const OPEN_EVENT_KIND: &str = "open";
        const OPEN_EVENT_TITLE: &str = "Opened project";
        let recent_open: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM project_events WHERE project_id = ?1 AND kind = ?2 AND title = ?3 AND datetime(created_at) >= datetime('now', '-30 minutes') ORDER BY datetime(created_at) DESC, id DESC LIMIT 1",
                params![id, OPEN_EVENT_KIND, OPEN_EVENT_TITLE],
                |row| row.get(0),
            )
            .optional()?;
        if recent_open.is_some() {
            return Ok(None);
        }
        self.record_project_event(id, OPEN_EVENT_KIND, OPEN_EVENT_TITLE, None)
            .map(Some)
    }

    pub fn record_audit_event(
        &self,
        origin: &str,
        action: &str,
        target_type: &str,
        target_id: Option<&str>,
        detail: Option<&str>,
        outcome: &str,
    ) -> Result<AuditEvent> {
        let event = AuditEvent {
            id: Uuid::new_v4().to_string(),
            origin: origin.into(),
            action: action.into(),
            target_type: target_type.into(),
            target_id: target_id.map(str::to_string),
            detail: detail
                .map(str::to_string)
                .filter(|value| !value.trim().is_empty()),
            outcome: outcome.into(),
            created_at: now(),
        };
        self.conn.execute(
            "INSERT INTO audit_events (id, origin, action, target_type, target_id, detail, outcome, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![event.id, event.origin, event.action, event.target_type, event.target_id, event.detail, event.outcome, event.created_at],
        )?;
        Ok(event)
    }

    pub fn list_audit_events(&self, limit: usize) -> Result<Vec<AuditEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, origin, action, target_type, target_id, detail, outcome, created_at FROM audit_events ORDER BY datetime(created_at) DESC, id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit.min(500) as i64], |row| {
            Ok(AuditEvent {
                id: row.get(0)?,
                origin: row.get(1)?,
                action: row.get(2)?,
                target_type: row.get(3)?,
                target_id: row.get(4)?,
                detail: row.get(5)?,
                outcome: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn list_attention_items(&self) -> Result<Vec<AttentionItem>> {
        let approvals = self.list_pending_approvals()?;
        self.list_attention_items_with_approvals(&approvals)
    }

    pub fn attention_center(&self) -> Result<crate::models::AttentionCenterState> {
        let approvals = self.list_pending_approvals()?;
        let items = self.list_attention_items_with_approvals(&approvals)?;
        Ok(crate::models::AttentionCenterState { approvals, items })
    }

    pub fn dashboard_snapshot(&self) -> Result<DashboardSnapshot> {
        const RECENT_PROJECT_LIMIT: usize = 6;
        const RECENT_RUN_LIMIT: usize = 8;
        const ATTENTION_PREVIEW_LIMIT: usize = 3;

        let (project_count, available_project_count, unavailable_project_count) =
            self.conn.query_row(
                "SELECT COUNT(*),
                    COALESCE(SUM(CASE WHEN availability = 'ready' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN availability = 'unavailable' THEN 1 ELSE 0 END), 0)
             FROM projects WHERE archived = 0",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )?;

        let attention_items = self.list_attention_items()?;
        let active_runs = self.list_active_task_runs()?;
        let recent_runs = crate::broker::list_dashboard_runs(&self.conn, RECENT_RUN_LIMIT)?;
        let seven_day_runs = self.dashboard_task_run_stats()?;
        let collections = self.dashboard_collection_summaries(&active_runs, &attention_items)?;
        let recent_projects = self.dashboard_recent_projects(RECENT_PROJECT_LIMIT)?;

        let run_project_ids = recent_runs
            .iter()
            .map(|run| run.project_id.clone())
            .collect::<Vec<_>>();
        let project_names = self.load_projects_by_ids(&run_project_ids)?;
        let recent_runs = recent_runs
            .into_iter()
            .map(|run| DashboardTaskRunItem {
                project_name: project_names
                    .get(&run.project_id)
                    .map(|project| project.display_name.clone())
                    .unwrap_or_else(|| run.project_id.clone()),
                run,
            })
            .collect();

        Ok(DashboardSnapshot {
            generated_at: now(),
            project_count: project_count as usize,
            available_project_count: available_project_count as usize,
            unavailable_project_count: unavailable_project_count as usize,
            collection_count: collections.len(),
            active_run_count: active_runs.len(),
            attention_count: attention_items.len(),
            seven_day_runs,
            collections,
            recent_projects,
            recent_runs,
            attention_preview: attention_items
                .into_iter()
                .take(ATTENTION_PREVIEW_LIMIT)
                .collect(),
        })
    }

    fn dashboard_task_run_stats(&self) -> Result<DashboardTaskRunStats> {
        let mut stats = DashboardTaskRunStats::default();
        let mut stmt = self.conn.prepare(
            "SELECT status, COUNT(*) FROM task_runs
             WHERE status IN ('succeeded', 'failed', 'cancelled')
               AND datetime(COALESCE(finished_at, started_at)) >= datetime('now', '-7 days')
             GROUP BY status",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
        })?;
        for row in rows {
            let (status, count) = row?;
            match status.as_str() {
                "succeeded" => stats.succeeded = count,
                "failed" => stats.failed = count,
                "cancelled" => stats.cancelled = count,
                _ => {}
            }
        }
        stats.total = stats.succeeded + stats.failed + stats.cancelled;
        Ok(stats)
    }

    fn dashboard_collection_summaries(
        &self,
        active_runs: &[crate::models::TaskRun],
        attention_items: &[AttentionItem],
    ) -> Result<Vec<DashboardCollectionSummary>> {
        let collections = self.list_collections()?;
        if collections.is_empty() {
            return Ok(Vec::new());
        }

        let mut memberships = HashMap::<String, Vec<String>>::new();
        let mut counts = HashMap::<String, (usize, usize)>::new();
        let mut stmt = self.conn.prepare(
            "SELECT m.collection_id, m.project_id, p.archived
             FROM project_collection_members m
             JOIN projects p ON p.id = m.project_id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? != 0,
            ))
        })?;
        for row in rows {
            let (collection_id, project_id, archived) = row?;
            memberships
                .entry(project_id)
                .or_default()
                .push(collection_id.clone());
            let entry = counts.entry(collection_id).or_default();
            if archived {
                entry.1 += 1;
            } else {
                entry.0 += 1;
            }
        }

        let mut active_counts = HashMap::<String, usize>::new();
        for run in active_runs {
            if let Some(collection_ids) = memberships.get(&run.project_id) {
                for collection_id in collection_ids {
                    *active_counts.entry(collection_id.clone()).or_default() += 1;
                }
            }
        }
        let mut attention_counts = HashMap::<String, usize>::new();
        for item in attention_items {
            let Some(project_id) = item.project_id.as_ref() else {
                continue;
            };
            if let Some(collection_ids) = memberships.get(project_id) {
                for collection_id in collection_ids {
                    *attention_counts.entry(collection_id.clone()).or_default() += 1;
                }
            }
        }

        let mut last_activity = HashMap::<String, String>::new();
        let mut activity_stmt = self.conn.prepare(
            "WITH activity(project_id, occurred_at) AS (
                SELECT id, last_opened_at FROM projects WHERE last_opened_at IS NOT NULL
                UNION ALL
                SELECT project_id, created_at FROM project_events
                UNION ALL
                SELECT project_id, started_at FROM task_runs
             )
             SELECT m.collection_id, MAX(activity.occurred_at)
             FROM project_collection_members m
             JOIN activity ON activity.project_id = m.project_id
             GROUP BY m.collection_id",
        )?;
        let activity_rows = activity_stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in activity_rows {
            let (collection_id, occurred_at) = row?;
            last_activity.insert(collection_id, occurred_at);
        }

        Ok(collections
            .into_iter()
            .map(|collection| {
                let (project_count, archived_project_count) =
                    counts.get(&collection.id).copied().unwrap_or_default();
                DashboardCollectionSummary {
                    active_run_count: active_counts.get(&collection.id).copied().unwrap_or(0),
                    attention_count: attention_counts.get(&collection.id).copied().unwrap_or(0),
                    last_activity_at: last_activity.remove(&collection.id),
                    project_count,
                    archived_project_count,
                    id: collection.id,
                    name: collection.name,
                    description: collection.description,
                }
            })
            .collect())
    }

    fn dashboard_recent_projects(&self, limit: usize) -> Result<Vec<DashboardProjectItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id FROM projects
             WHERE archived = 0 AND last_opened_at IS NOT NULL
             ORDER BY datetime(last_opened_at) DESC, lower(display_name), id
             LIMIT ?1",
        )?;
        let project_ids = stmt
            .query_map([limit.clamp(1, 50) as i64], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if project_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut projects = self.load_projects_by_ids(&project_ids)?;
        let placeholders = (1..=project_ids.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");

        let mut git = HashMap::<String, GitSnapshot>::new();
        let mut git_stmt = self.conn.prepare(&format!(
            "SELECT id, git_json FROM projects WHERE id IN ({placeholders}) AND git_json IS NOT NULL"
        ))?;
        let git_rows = git_stmt
            .query_map(rusqlite::params_from_iter(project_ids.iter()), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
        for row in git_rows {
            let (project_id, value) = row?;
            if let Ok(snapshot) = serde_json::from_str::<GitSnapshot>(&value) {
                git.insert(project_id, snapshot);
            }
        }

        let mut latest_runs = HashMap::<String, DashboardTaskRunSummary>::new();
        let mut run_stmt = self.conn.prepare(&format!(
            "SELECT r.project_id, r.kind, r.status, r.started_at, r.finished_at
             FROM projects p JOIN task_runs r ON r.id = (
                  SELECT latest.id FROM task_runs latest
                  WHERE latest.project_id = p.id
                  ORDER BY datetime(latest.started_at) DESC, latest.id DESC LIMIT 1
               )
             WHERE p.id IN ({placeholders})"
        ))?;
        let run_rows =
            run_stmt.query_map(rusqlite::params_from_iter(project_ids.iter()), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    DashboardTaskRunSummary {
                        kind: row.get(1)?,
                        status: row.get(2)?,
                        started_at: row.get(3)?,
                        finished_at: row.get(4)?,
                    },
                ))
            })?;
        for row in run_rows {
            let (project_id, run) = row?;
            latest_runs.insert(project_id, run);
        }

        Ok(project_ids
            .into_iter()
            .filter_map(|project_id| {
                projects
                    .remove(&project_id)
                    .map(|project| DashboardProjectItem {
                        git: git.remove(&project_id),
                        latest_run: latest_runs.remove(&project_id),
                        project,
                    })
            })
            .collect())
    }

    fn list_attention_items_with_approvals(
        &self,
        approvals: &[PendingApproval],
    ) -> Result<Vec<AttentionItem>> {
        let mut items = Vec::new();
        for approval in approvals {
            items.push(AttentionItem {
                id: format!("approval:{}", approval.id),
                kind: "approval".into(),
                severity: "action".into(),
                project_id: Some(approval.project_id.clone()),
                run_id: None,
                approval_id: Some(approval.id.clone()),
                title: approval.title.clone(),
                detail: approval.detail.clone(),
                occurred_at: approval.created_at.clone(),
                source_version: approval.created_at.clone(),
                acknowledgeable: false,
            });
        }
        let unavailable_projects = {
            let mut stmt = self.conn.prepare(
                "SELECT id, display_name, canonical_path, updated_at FROM projects WHERE availability = 'unavailable' ORDER BY datetime(updated_at) DESC LIMIT 500",
            )?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };
        for (project_id, display_name, canonical_path, updated_at) in unavailable_projects {
            items.push(AttentionItem {
                id: format!("project-unavailable:{project_id}"),
                kind: "project_unavailable".into(),
                severity: "warning".into(),
                project_id: Some(project_id),
                run_id: None,
                approval_id: None,
                title: format!("{display_name} is unavailable"),
                detail: canonical_path,
                occurred_at: updated_at.clone(),
                source_version: updated_at,
                acknowledgeable: true,
            });
        }
        let mut failed_stmt = self.conn.prepare(
            "SELECT id, project_id, kind, exit_code, started_at, finished_at FROM task_runs WHERE status = 'failed' ORDER BY datetime(started_at) DESC LIMIT 50",
        )?;
        let failed = failed_stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<i32>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (id, project_id, kind, exit_code, started_at, finished_at) in failed {
            let occurred_at = finished_at.unwrap_or(started_at);
            items.push(AttentionItem {
                id: format!("run-failed:{id}"),
                kind: "task_failed".into(),
                severity: "error".into(),
                project_id: Some(project_id),
                run_id: Some(id),
                approval_id: None,
                title: format!("{kind} task failed"),
                detail: exit_code.map_or_else(
                    || "No exit code was reported".into(),
                    |code| format!("Exit code {code}"),
                ),
                occurred_at: occurred_at.clone(),
                source_version: occurred_at,
                acknowledgeable: true,
            });
        }
        let mut environment_stmt = self.conn.prepare(
            "SELECT project_id, inspection_json, observed_at, condition_version FROM environment_observations ORDER BY datetime(observed_at) DESC",
        )?;
        let observations = environment_stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (project_id, inspection_json, observed_at, condition_version) in observations {
            let Ok(inspection) =
                serde_json::from_str::<crate::models::EnvironmentInspection>(&inspection_json)
            else {
                continue;
            };
            let mismatches = inspection
                .runtimes
                .into_iter()
                .filter(|runtime| matches!(runtime.match_state.as_str(), "mismatch" | "missing"))
                .map(|runtime| runtime.label)
                .collect::<Vec<_>>();
            if mismatches.is_empty() {
                continue;
            }
            items.push(AttentionItem {
                id: format!("environment:{project_id}"),
                kind: "environment_mismatch".into(),
                severity: "warning".into(),
                project_id: Some(project_id),
                run_id: None,
                approval_id: None,
                title: "Project environment needs attention".into(),
                detail: mismatches.join(", "),
                occurred_at: observed_at.clone(),
                source_version: condition_version,
                acknowledgeable: true,
            });
        }
        for event in self.list_audit_events(50)?.into_iter().filter(|event| {
            (event.outcome == "failure" || event.outcome == "failed")
                && event.action != "finish_task_run"
        }) {
            items.push(AttentionItem {
                id: format!("audit:{}", event.id),
                kind: "system_failure".into(),
                severity: "error".into(),
                project_id: (event.target_type == "project")
                    .then_some(event.target_id.clone())
                    .flatten(),
                run_id: None,
                approval_id: None,
                title: event.action.replace('_', " "),
                detail: event
                    .detail
                    .unwrap_or_else(|| "RepoAtlas recorded an operation failure".into()),
                occurred_at: event.created_at.clone(),
                source_version: event.created_at,
                acknowledgeable: true,
            });
        }
        let acknowledgeable_ids = items
            .iter()
            .filter(|item| item.acknowledgeable)
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>();
        let mut acknowledged = HashSet::new();
        for chunk in acknowledgeable_ids.chunks(500) {
            let placeholders = (1..=chunk.len())
                .map(|index| format!("?{index}"))
                .collect::<Vec<_>>()
                .join(", ");
            let mut stmt = self.conn.prepare(&format!(
                "SELECT item_id, source_version FROM attention_acknowledgements WHERE item_id IN ({placeholders})"
            ))?;
            let rows = stmt
                .query_map(rusqlite::params_from_iter(chunk.iter().copied()), |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?;
            acknowledged.extend(rows.collect::<rusqlite::Result<Vec<_>>>()?);
        }
        items.retain(|item| {
            !item.acknowledgeable
                || !acknowledged.contains(&(item.id.clone(), item.source_version.clone()))
        });
        items.sort_by(|left, right| {
            attention_severity_rank(&right.severity)
                .cmp(&attention_severity_rank(&left.severity))
                .then_with(|| right.occurred_at.cmp(&left.occurred_at))
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(items)
    }

    pub fn acknowledge_attention_item(&self, item_id: &str, source_version: &str) -> Result<()> {
        if item_id.trim().is_empty() || source_version.trim().is_empty() {
            return Err(Error::msg(
                "attention item id and source version are required",
            ));
        }
        let current = self
            .list_attention_items()?
            .into_iter()
            .find(|item| item.id == item_id && item.source_version == source_version)
            .ok_or_else(|| Error::msg("attention item is no longer current"))?;
        if !current.acknowledgeable {
            return Err(Error::msg(
                "this live attention item cannot be acknowledged",
            ));
        }
        self.with_immediate_transaction(|core| {
            core.conn.execute(
                "INSERT OR REPLACE INTO attention_acknowledgements (item_id, source_version, acknowledged_at) VALUES (?1, ?2, ?3)",
                params![item_id, source_version, now()],
            )?;
            core.record_audit_event(
                "desktop",
                "acknowledge_attention_item",
                "attention_item",
                Some(item_id),
                Some(source_version),
                "success",
            )?;
            Ok(())
        })
    }

    pub fn list_project_events(&self, project_id: &str, limit: usize) -> Result<Vec<ProjectEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project_id, kind, title, detail, created_at FROM project_events WHERE project_id = ?1 ORDER BY datetime(created_at) DESC, id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![project_id, limit as i64], |row| {
            Ok(ProjectEvent {
                id: row.get(0)?,
                project_id: row.get(1)?,
                kind: row.get(2)?,
                title: row.get(3)?,
                detail: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn atlas_report(&self, project_id: &str) -> Result<AtlasReport> {
        let detail = self.get_project(project_id)?;
        let environment = self.inspect_project_environment(project_id).ok();
        Ok(AtlasReport {
            project_id: project_id.into(),
            markdown: render_atlas_report(&detail, environment.as_ref()),
            generated_at: now(),
        })
    }

    fn lineage_for(&self, detail: &ProjectDetail) -> Result<Option<CheckoutLineage>> {
        let remote = detail
            .facts
            .iter()
            .find(|fact| fact.kind == "lineage")
            .map(|fact| fact.value.clone())
            .or_else(|| crate::git::origin_url(Path::new(&detail.project.canonical_path)));
        let Some(remote_url) = remote else {
            return Ok(None);
        };
        let Some(normalized) = crate::git::normalize_remote_url(&remote_url) else {
            return Ok(Some(CheckoutLineage {
                remote_url: Some(remote_url),
                normalized_url: None,
                checkouts: Vec::new(),
            }));
        };
        let mut stmt = self.conn.prepare(
            "SELECT id, display_name, canonical_path, git_json FROM projects WHERE id != ?1 AND lineage_key = ?2",
        )?;
        let rows = stmt.query_map(params![detail.project.id, normalized], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?;
        let mut checkouts = Vec::new();
        for row in rows {
            let (id, display_name, canonical_path, git_json) = row?;
            let git: Option<crate::models::GitSnapshot> = opt_from_json(git_json)?;
            checkouts.push(LineageCheckout {
                project_id: id,
                display_name,
                canonical_path,
                branch: git.and_then(|snap| snap.branch),
            });
        }
        checkouts.sort_by(|left, right| {
            left.display_name
                .cmp(&right.display_name)
                .then(left.canonical_path.cmp(&right.canonical_path))
        });
        Ok(Some(CheckoutLineage {
            remote_url: Some(remote_url),
            normalized_url: Some(normalized),
            checkouts,
        }))
    }

    pub fn export_json(&self) -> Result<String> {
        crate::import_export::export(&self.conn)
    }

    pub fn import_json(&self, text: &str) -> Result<usize> {
        crate::import_export::import_into(&self.conn, text, &|path| {
            self.matching_scan_root_for_import(path).ok().flatten()
        })
    }

    pub fn backup_db(&self, dest: impl AsRef<Path>) -> Result<()> {
        crate::import_export::backup_to(&self.conn, dest.as_ref())
    }

    fn matching_scan_root_for_import(&self, path: &Path) -> Result<Option<String>> {
        self.matching_scan_root(path)
    }

    pub fn git_status(&self, project_id: &str) -> Result<GitStatus> {
        git::status(&self.project_path(project_id)?)
    }

    pub fn git_diff(&self, project_id: &str, path: Option<&str>, staged: bool) -> Result<GitDiff> {
        git::diff(&self.project_path(project_id)?, path, staged)
    }

    pub fn git_execute(&self, project_id: &str, op: GitOp) -> Result<GitCommandResult> {
        let operation = self.prepare_git_operation(project_id, op)?;
        self.finish_git_operation(&operation, operation.execute())
    }

    pub fn prepare_git_operation(
        &self,
        project_id: &str,
        op: GitOp,
    ) -> Result<PreparedGitOperation> {
        Ok(PreparedGitOperation {
            project_id: project_id.into(),
            path: self.project_path(project_id)?,
            op,
        })
    }

    pub fn validate_git_operation(&self, operation: &PreparedGitOperation) -> Result<()> {
        if self.project_path(&operation.project_id)? != operation.path {
            return Err(Error::msg(
                "project location changed; retry the Git operation",
            ));
        }
        Ok(())
    }

    pub fn finish_git_operation(
        &self,
        operation: &PreparedGitOperation,
        outcome: Result<(GitCommandResult, Option<GitSnapshot>)>,
    ) -> Result<GitCommandResult> {
        let project_id = operation.project_id.as_str();
        let op = &operation.op;
        let action = git_action_name(op);
        let (mut result, snapshot) = match outcome {
            Ok(result) => result,
            Err(error) => {
                self.record_audit_event(
                    "desktop",
                    action,
                    "project",
                    Some(project_id),
                    None,
                    "failed",
                )?;
                return Err(error);
            }
        };
        let (title, detail) = match op {
            GitOp::Commit { .. } => ("Git commit", None),
            GitOp::Pull => ("Git pull", None),
            GitOp::Push => ("Git push", None),
            GitOp::StashPush { .. } => ("Git stash", None),
            GitOp::StashPop => ("Git stash pop", None),
            GitOp::Checkout { branch, .. } => ("Git checkout", Some(branch.as_str())),
            _ => ("Git operation", None),
        };
        self.record_audit_event(
            "desktop",
            action,
            "project",
            Some(project_id),
            detail,
            if result.ok { "success" } else { "failed" },
        )?;
        // The operation already ran: preserve its outcome and audit even when
        // another client moved or removed the record while Git was running.
        if self.validate_git_operation(operation).is_err() {
            let warning =
                "Project record changed during Git execution; cached state was not updated.";
            result.stdout.push_str(&format!("\n{warning}"));
            result.stderr.push_str(&format!("\n{warning}"));
            return Ok(result);
        }
        if let Some(snapshot) = snapshot {
            self.conn.execute("UPDATE projects SET git_json = ?1, last_commit_at = ?2, updated_at = ?3 WHERE id = ?4",
                params![to_json(&snapshot)?, snapshot.last_commit_at, now(), project_id])?;
        }
        let _ = self.record_project_event(project_id, "git", title, detail);
        Ok(result)
    }

    pub fn project_path(&self, project_id: &str) -> Result<PathBuf> {
        self.conn
            .query_row(
                "SELECT canonical_path FROM projects WHERE id = ?1",
                [project_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(PathBuf::from)
            .ok_or_else(|| Error::NotFound(project_id.into()))
    }

    fn upsert_project(
        &self,
        path: &Path,
        scan_root_id: Option<&str>,
        origin: &str,
    ) -> Result<ProjectSummary> {
        let canonical = paths::canonicalize(path)?;
        let detection = detect::detect(&canonical);
        let git = git::snapshot(&canonical);
        let lineage_remote = detection
            .facts
            .iter()
            .find(|fact| fact.kind == "lineage")
            .map(|fact| fact.value.clone())
            .or_else(|| git::origin_url(&canonical));
        let lineage_key = lineage_remote
            .as_deref()
            .and_then(git::normalize_remote_url);
        let last_commit_at = git.as_ref().and_then(|snap| snap.last_commit_at.clone());
        let now = now();
        let path_string = paths::path_to_string(&canonical);
        let existing_id: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM projects WHERE canonical_path = ?1",
                params![path_string],
                |row| row.get(0),
            )
            .optional()?;
        let id = existing_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let existing_metadata: Option<(String, Option<String>, Option<String>)> = self
            .conn
            .query_row(
                "SELECT display_name, detected_name, description FROM projects WHERE id = ?1",
                params![id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()?;
        let display_name = existing_metadata
            .as_ref()
            .map(|(display, detected, _)| {
                if detected.as_deref() == Some(display.as_str()) || detected.is_none() {
                    detection.name.clone()
                } else {
                    display.clone()
                }
            })
            .unwrap_or_else(|| detection.name.clone());
        let description = existing_metadata.and_then(|(_, _, description)| description);
        let existing_tasks: Vec<TaskDefinition> = self
            .conn
            .query_row(
                "SELECT tasks_json FROM projects WHERE id = ?1",
                params![id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(from_json)
            .transpose()?
            .unwrap_or_default();
        let module_ids: HashSet<_> = if existing_tasks.is_empty() {
            Vec::new()
        } else {
            self.list_modules(&id)?
        }
        .into_iter()
        .filter(|m| m.project_id.is_none())
        .flat_map(|m| m.task_ids)
        .collect();
        let module_tasks: Vec<_> = existing_tasks
            .iter()
            .filter(|t| t.inferred && module_ids.contains(&t.id))
            .cloned()
            .collect();
        let mut merged_tasks = merge_tasks(existing_tasks, detection.tasks.clone());
        merged_tasks.extend(module_tasks);
        self.conn.execute(
            r#"
            INSERT INTO projects (
                id, canonical_path, display_name, detected_name, notes, description, vcs_kind, availability,
                archived, favorite, origin, scan_root_id, languages_json, frameworks_json,
                package_managers_json, facts_json, lineage_key, tasks_json, dependencies_json, git_json,
                readme_path, readme_excerpt, search_blob, source_mtime, last_commit_at,
                created_at, updated_at
            ) VALUES (
                ?1, ?2, ?3, ?4, NULL, ?5, ?6, 'ready', 0, 0, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?22
            )
            ON CONFLICT(canonical_path) DO UPDATE SET
                display_name = excluded.display_name,
                detected_name = excluded.detected_name,
                vcs_kind = excluded.vcs_kind,
                availability = 'ready',
                origin = CASE WHEN projects.origin = 'manual' THEN projects.origin ELSE excluded.origin END,
                scan_root_id = excluded.scan_root_id,
                languages_json = excluded.languages_json,
                frameworks_json = excluded.frameworks_json,
                package_managers_json = excluded.package_managers_json,
                facts_json = excluded.facts_json,
                lineage_key = excluded.lineage_key,
                tasks_json = excluded.tasks_json,
                dependencies_json = excluded.dependencies_json,
                git_json = excluded.git_json,
                readme_path = excluded.readme_path,
                readme_excerpt = excluded.readme_excerpt,
                search_blob = excluded.search_blob,
                source_mtime = excluded.source_mtime,
                last_commit_at = excluded.last_commit_at,
                updated_at = excluded.updated_at
            "#,
            params![
                id,
                path_string,
                display_name,
                detection.name,
                description,
                detection.vcs_kind,
                origin,
                scan_root_id,
                to_json(&detection.languages)?,
                to_json(&detection.frameworks)?,
                to_json(&detection.package_managers)?,
                to_json(&detection.facts)?,
                lineage_key,
                to_json(&merged_tasks)?,
                opt_json(&detection.dependencies)?,
                opt_json(&git)?,
                detection.readme_path,
                detection.readme_excerpt,
                detection.search_blob,
                detection.source_mtime,
                last_commit_at,
                now,
            ],
        )?;
        self.reindex_project(&id)?;
        self.get_project_summary(&id)?
            .ok_or_else(|| Error::NotFound(id))
    }

    fn load_projects(&self) -> Result<Vec<ProjectSummary>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, canonical_path, display_name, detected_name, notes, description, vcs_kind, availability,
                   archived, favorite, origin, scan_root_id, languages_json, frameworks_json,
                   package_managers_json, source_mtime, last_commit_at, last_opened_at, updated_at
            FROM projects
            ORDER BY favorite DESC, datetime(COALESCE(last_opened_at, updated_at)) DESC, display_name
            "#,
        )?;
        let mut projects = stmt
            .query_map([], |row| Ok(row_to_summary(row)))?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .collect::<std::result::Result<Vec<_>, _>>()?;
        self.attach_tags(&mut projects)?;
        Ok(projects)
    }

    fn load_projects_for_query(&self, query: &ProjectQuery) -> Result<Vec<ProjectSummary>> {
        let mut conditions = Vec::new();
        match query.section.as_deref() {
            Some("favorites") => conditions.push("p.favorite = 1 AND p.archived = 0"),
            Some("recent") => conditions.push("p.last_opened_at IS NOT NULL AND p.archived = 0"),
            Some("archived") => conditions.push("p.archived = 1"),
            _ if query.include_archived != Some(true) => conditions.push("p.archived = 0"),
            _ => {}
        }
        let mut parameters = Vec::new();
        if let Some(collection_id) = query.collection_id.as_ref() {
            parameters.push(collection_id.clone());
            conditions.push(
                "EXISTS (SELECT 1 FROM project_collection_members m WHERE m.collection_id = ?1 AND m.project_id = p.id)",
            );
        }
        let limit_clause = query.limit.map_or_else(String::new, |limit| {
            parameters.push(limit.clamp(1, 10_000).to_string());
            format!("LIMIT ?{}", parameters.len())
        });
        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };
        let sql = format!(
            r#"
            SELECT p.id, p.canonical_path, p.display_name, p.detected_name, p.notes, p.description, p.vcs_kind, p.availability,
                   p.archived, p.favorite, p.origin, p.scan_root_id, p.languages_json, p.frameworks_json,
                   p.package_managers_json, p.source_mtime, p.last_commit_at, p.last_opened_at, p.updated_at
            FROM projects p
            {where_clause}
            ORDER BY p.favorite DESC, datetime(COALESCE(p.last_opened_at, p.updated_at)) DESC, p.display_name
            {limit_clause}
            "#,
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let mut projects = stmt
            .query_map(rusqlite::params_from_iter(parameters.iter()), |row| {
                Ok(row_to_summary(row))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .collect::<std::result::Result<Vec<_>, _>>()?;
        self.attach_tags(&mut projects)?;
        Ok(projects)
    }

    fn load_projects_by_ids(&self, ids: &[String]) -> Result<HashMap<String, ProjectSummary>> {
        Ok(self
            .load_project_summaries_by_ids(ids)?
            .into_iter()
            .map(|project| (project.id.clone(), project))
            .collect())
    }

    fn load_project_summaries_by_ids(&self, ids: &[String]) -> Result<Vec<ProjectSummary>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = (1..=ids.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT id, canonical_path, display_name, detected_name, notes, description, vcs_kind, availability, archived, favorite, origin, scan_root_id, languages_json, frameworks_json, package_managers_json, source_mtime, last_commit_at, last_opened_at, updated_at FROM projects WHERE id IN ({placeholders}) ORDER BY favorite DESC, datetime(COALESCE(last_opened_at, updated_at)) DESC, display_name"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(ids.iter()), |row| {
                Ok(row_to_summary(row))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut projects = rows
            .into_iter()
            .collect::<std::result::Result<Vec<_>, _>>()?;
        self.attach_tags(&mut projects)?;
        Ok(projects)
    }

    fn attach_tags(&self, projects: &mut [ProjectSummary]) -> Result<()> {
        if projects.is_empty() {
            return Ok(());
        }
        let mut tags = HashMap::<String, Vec<String>>::new();
        let ids = projects
            .iter()
            .map(|project| &project.id)
            .collect::<Vec<_>>();
        for chunk in ids.chunks(500) {
            let placeholders = (1..=chunk.len())
                .map(|index| format!("?{index}"))
                .collect::<Vec<_>>()
                .join(", ");
            let mut stmt = self.conn.prepare(&format!(
                "SELECT project_id, tag FROM project_tags WHERE project_id IN ({placeholders}) ORDER BY project_id, tag"
            ))?;
            let rows = stmt
                .query_map(rusqlite::params_from_iter(chunk.iter().copied()), |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?;
            for row in rows {
                let (project_id, tag) = row?;
                tags.entry(project_id).or_default().push(tag);
            }
        }
        let mut stmt = self
            .conn
            .prepare("SELECT project_id FROM project_directory_groups")?;
        let groups = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<HashSet<_>>>()?;
        for project in projects {
            project.directory_group = groups.contains(&project.id);
            project.tags = tags.remove(&project.id).unwrap_or_default();
        }
        Ok(())
    }

    fn matches_section(&self, project: &ProjectSummary, query: &ProjectQuery) -> bool {
        if query.include_archived != Some(true)
            && project.archived
            && query.section.as_deref() != Some("archived")
        {
            return false;
        }
        match query.section.as_deref() {
            Some("favorites") => project.favorite && !project.archived,
            Some("recent") => project.last_opened_at.is_some() && !project.archived,
            Some("archived") => project.archived,
            _ => true,
        }
    }

    fn accumulate_fts(
        &self,
        table: &str,
        match_query: &str,
        weight: i64,
        scores: &mut HashMap<String, i64>,
    ) -> Result<()> {
        let sql = format!(
            "SELECT project_id, bm25({table}) FROM {table} WHERE {table} MATCH ?1 LIMIT 200"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![match_query], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        });
        let rows = match rows {
            Ok(rows) => rows,
            Err(_) => return Ok(()),
        };
        for row in rows {
            let (id, rank) = row?;
            let score = ((-rank) * 10.0) as i64 + weight;
            scores
                .entry(id)
                .and_modify(|current| *current += score)
                .or_insert(score);
        }
        Ok(())
    }

    fn reindex_project(&self, id: &str) -> Result<()> {
        let (display_name, path, description, languages, frameworks, package_managers, search_blob, notes): (
            String,
            String,
            Option<String>,
            String,
            String,
            String,
            String,
            Option<String>,
        ) = self.conn.query_row(
            "SELECT display_name, canonical_path, description, languages_json, frameworks_json, package_managers_json, search_blob, notes FROM projects WHERE id = ?1",
            params![id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                ))
            },
        )?;
        let tags = self.tags_for(id)?.join(" ");
        let content = [
            display_name,
            path,
            description.unwrap_or_default(),
            languages,
            frameworks,
            package_managers,
            tags,
            notes.unwrap_or_default(),
            search_blob,
        ]
        .join("\n");
        self.conn.execute(
            "DELETE FROM project_fts_unicode WHERE project_id = ?1",
            params![id],
        )?;
        self.conn.execute(
            "DELETE FROM project_fts_trigram WHERE project_id = ?1",
            params![id],
        )?;
        self.conn.execute(
            "INSERT INTO project_fts_unicode (project_id, content) VALUES (?1, ?2)",
            params![id, content],
        )?;
        self.conn.execute(
            "INSERT INTO project_fts_trigram (project_id, content) VALUES (?1, ?2)",
            params![id, content],
        )?;
        Ok(())
    }

    fn count_rows(&self, table: &str, project_id: &str) -> Result<u64> {
        // `table` is always selected from the fixed call sites above; values
        // supplied by users never reach this SQL identifier.
        let sql = format!("SELECT COUNT(*) FROM {table} WHERE project_id = ?1");
        Ok(self
            .conn
            .query_row(&sql, params![project_id], |row| row.get::<_, i64>(0))? as u64)
    }

    fn count_project_runs(&self, project_id: &str, status: Option<&str>) -> Result<u64> {
        let count: i64 = match status {
            Some("active") => self.conn.query_row(
                "SELECT COUNT(*) FROM task_runs WHERE project_id = ?1 AND status IN ('running', 'starting')",
                params![project_id],
                |row| row.get(0),
            )?,
            Some(status) => self.conn.query_row(
                "SELECT COUNT(*) FROM task_runs WHERE project_id = ?1 AND status = ?2",
                params![project_id, status],
                |row| row.get(0),
            )?,
            None => self.conn.query_row(
                "SELECT COUNT(*) FROM task_runs WHERE project_id = ?1",
                params![project_id],
                |row| row.get(0),
            )?,
        };
        Ok(count as u64)
    }

    fn ensure_no_running_projects(&self, project_ids: &[String]) -> Result<()> {
        for project_id in project_ids {
            if self.count_project_runs(project_id, Some("active"))? > 0 {
                return Err(Error::msg(
                    "one or more projects have running or starting tasks; stop them before removal",
                ));
            }
            let starting_approvals: i64 = self.conn.query_row(
                "SELECT COUNT(*) FROM pending_approvals WHERE project_id = ?1 AND status = 'starting'",
                params![project_id],
                |row| row.get(0),
            )?;
            if starting_approvals > 0 {
                return Err(Error::msg(
                    "one or more projects have a task approval being started; wait for it to finish before removal",
                ));
            }
        }
        Ok(())
    }

    fn queue_task_log_cleanup(&self, project_id: &str) -> Result<()> {
        let mut stmt = self
            .conn
            .prepare("SELECT log_path FROM task_runs WHERE project_id = ?1")?;
        let paths = stmt
            .query_map(params![project_id], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let Some(log_dir) = &self.log_dir else {
            return Ok(());
        };
        let root = paths::canonicalize(log_dir).unwrap_or_else(|_| paths::normalize(log_dir));
        for log_path in paths {
            let candidate = PathBuf::from(&log_path);
            let normalized =
                paths::canonicalize(&candidate).unwrap_or_else(|_| paths::normalize(&candidate));
            if !paths::is_within(&normalized, &root)
                || normalized == root
                || normalized.file_name().is_none()
            {
                continue;
            }
            self.conn.execute(
                "INSERT OR IGNORE INTO managed_file_cleanup (id, project_id, path, kind, created_at) VALUES (?1, ?2, ?3, 'task-log', ?4)",
                params![Uuid::new_v4().to_string(), project_id, paths::path_to_string(&normalized), now()],
            )?;
        }
        Ok(())
    }

    pub fn retry_managed_file_cleanup(&self) -> Result<()> {
        let Some(log_dir) = &self.log_dir else {
            return Ok(());
        };
        let root = paths::canonicalize(log_dir).unwrap_or_else(|_| paths::normalize(log_dir));
        let mut stmt = self
            .conn
            .prepare("SELECT id, path FROM managed_file_cleanup")?;
        let entries = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (id, path) in entries {
            let candidate = PathBuf::from(&path);
            let normalized =
                paths::canonicalize(&candidate).unwrap_or_else(|_| paths::normalize(&candidate));
            if !paths::is_within(&normalized, &root) || normalized == root {
                self.conn.execute(
                    "UPDATE managed_file_cleanup SET last_error = ?1 WHERE id = ?2",
                    params!["cleanup path is outside the task log directory", id],
                )?;
                continue;
            }
            match fs::remove_file(&normalized) {
                Ok(()) => {
                    self.conn.execute(
                        "DELETE FROM managed_file_cleanup WHERE id = ?1",
                        params![id],
                    )?;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    self.conn.execute(
                        "DELETE FROM managed_file_cleanup WHERE id = ?1",
                        params![id],
                    )?;
                }
                Err(error) => {
                    self.conn.execute(
                        "UPDATE managed_file_cleanup SET last_error = ?1 WHERE id = ?2",
                        params![error.to_string(), id],
                    )?;
                }
            }
        }
        Ok(())
    }

    /// Mark execution records which cannot survive a process restart as
    /// interrupted. The Broker keeps child handles in memory only; once the
    /// desktop process has gone away there is no safe way for a new Broker to
    /// resume those handles. This method is intentionally called only by
    /// `open`/`open_in_memory` (the MCP adapter opts out via
    /// `open_without_recovery` to avoid disturbing a live desktop Broker).
    fn recover_interrupted_runtime(&self) -> Result<()> {
        let recovered_at = now();
        let stale_approvals: Vec<(String, String)> = {
            let mut stmt = self.conn.prepare(
                "SELECT id, project_id FROM pending_approvals WHERE status = 'starting'",
            )?;
            let rows = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };
        for (id, project_id) in stale_approvals {
            let changed = self.conn.execute(
                "UPDATE pending_approvals SET status = 'failed', error = ?1, resolved_at = ?2 WHERE id = ?3 AND status = 'starting'",
                params![
                    "interrupted when RepoAtlas restarted",
                    recovered_at,
                    id
                ],
            )?;
            if changed == 1 {
                // Recovery should never prevent the application from opening
                // a readable database. Keep the audit best-effort here; all
                // user-initiated transitions still surface audit failures.
                let _ = self.record_audit_event(
                    "system",
                    "recover_task_approval",
                    "task_approval",
                    Some(&id),
                    Some("starting approval interrupted by restart"),
                    "failed",
                );
                let _ = self.record_project_event(
                    &project_id,
                    "task",
                    "Task approval interrupted by restart",
                    None,
                );
            }
        }

        let stale_runs: Vec<(String, String)> = {
            let mut stmt = self.conn.prepare(
                "SELECT id, project_id FROM task_runs WHERE status IN ('running', 'starting')",
            )?;
            let rows = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };
        for (run_id, project_id) in stale_runs {
            let changed = self.conn.execute(
                "UPDATE task_runs SET status = 'failed', finished_at = ?1 WHERE id = ?2 AND status IN ('running', 'starting')",
                params![recovered_at, run_id],
            )?;
            if changed == 1 {
                let _ = self.record_audit_event(
                    "system",
                    "recover_task_run",
                    "task_run",
                    Some(&run_id),
                    Some("run interrupted by RepoAtlas restart"),
                    "failed",
                );
                let _ = self.record_project_event(
                    &project_id,
                    "task",
                    "Task interrupted by restart",
                    Some(&run_id),
                );
            }
        }
        Ok(())
    }

    fn pending_log_cleanup_count(&self, project_id: &str) -> Result<u64> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM managed_file_cleanup WHERE project_id = ?1",
            params![project_id],
            |row| row.get::<_, i64>(0),
        )? as u64)
    }

    fn count_project_audit_events(&self, project_id: &str) -> Result<u64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM audit_events WHERE (target_type = 'project' AND target_id = ?1) OR (target_type = 'conversation' AND target_id = ?1) OR (target_type = 'task' AND target_id IN (SELECT json_extract(value, '$.id') FROM projects, json_each(projects.tasks_json) WHERE projects.id = ?1)) OR (target_type = 'memory' AND target_id IN (SELECT id FROM ai_memory WHERE project_id = ?1)) OR (target_type = 'summary' AND target_id IN (SELECT id FROM ai_summaries WHERE project_id = ?1)) OR (target_type = 'task_approval' AND target_id IN (SELECT id FROM pending_approvals WHERE project_id = ?1)) OR (target_type = 'task_run' AND target_id IN (SELECT id FROM task_runs WHERE project_id = ?1))",
            params![project_id],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    fn delete_project_audit_events(&self, project_id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM audit_events WHERE (target_type = 'project' AND target_id = ?1) OR (target_type = 'conversation' AND target_id = ?1) OR (target_type = 'task' AND target_id IN (SELECT json_extract(value, '$.id') FROM projects, json_each(projects.tasks_json) WHERE projects.id = ?1)) OR (target_type = 'memory' AND target_id IN (SELECT id FROM ai_memory WHERE project_id = ?1)) OR (target_type = 'summary' AND target_id IN (SELECT id FROM ai_summaries WHERE project_id = ?1)) OR (target_type = 'task_approval' AND target_id IN (SELECT id FROM pending_approvals WHERE project_id = ?1)) OR (target_type = 'task_run' AND target_id IN (SELECT id FROM task_runs WHERE project_id = ?1))",
            params![project_id],
        )?;
        Ok(())
    }

    fn delete_project_record(&self, id: &str) -> Result<()> {
        self.delete_project_audit_events(id)?;
        self.conn.execute(
            "DELETE FROM project_fts_unicode WHERE project_id = ?1",
            params![id],
        )?;
        self.conn.execute(
            "DELETE FROM project_fts_trigram WHERE project_id = ?1",
            params![id],
        )?;
        self.conn.execute(
            "DELETE FROM project_tags WHERE project_id = ?1",
            params![id],
        )?;
        self.conn
            .execute("DELETE FROM projects WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn replace_tags(&self, id: &str, tags: &[String]) -> Result<()> {
        self.conn.execute(
            "DELETE FROM project_tags WHERE project_id = ?1",
            params![id],
        )?;
        for tag in tags {
            let cleaned = tag.trim();
            if cleaned.is_empty() {
                continue;
            }
            self.conn.execute(
                "INSERT OR IGNORE INTO project_tags (project_id, tag) VALUES (?1, ?2)",
                params![id, cleaned],
            )?;
        }
        Ok(())
    }

    fn tags_for(&self, id: &str) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT tag FROM project_tags WHERE project_id = ?1 ORDER BY tag")?;
        let tags = stmt
            .query_map(params![id], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(tags)
    }

    fn get_project_summary(&self, id: &str) -> Result<Option<ProjectSummary>> {
        let mut project = self
            .conn
            .query_row(
                r#"
                SELECT id, canonical_path, display_name, detected_name, notes, description, vcs_kind, availability,
                       archived, favorite, origin, scan_root_id, languages_json, frameworks_json,
                       package_managers_json, source_mtime, last_commit_at, last_opened_at, updated_at
                FROM projects WHERE id = ?1
                "#,
                params![id],
                |row| Ok(row_to_summary(row)),
            )
            .optional()?
            .transpose()?;
        if let Some(project) = project.as_mut() {
            project.tags = self.tags_for(&project.id)?;
            project.directory_group = self.is_directory_group(&project.id)?;
        }
        Ok(project)
    }

    fn project_by_path(&self, path: &Path) -> Result<ProjectSummary> {
        let path = paths::path_to_string(path);
        let id: String = self.conn.query_row(
            "SELECT id FROM projects WHERE canonical_path = ?1",
            params![path],
            |row| row.get(0),
        )?;
        self.get_project_summary(&id)?
            .ok_or_else(|| Error::NotFound(id))
    }

    fn matching_scan_root(&self, path: &Path) -> Result<Option<String>> {
        // A project can sit below multiple authorized roots. Prefer the most
        // specific (longest) root so relocation/import keeps the tightest
        // authorization boundary instead of whichever root happened to be
        // inserted first.
        let mut matching: Option<ScanRoot> = None;
        for root in self.list_scan_roots()? {
            if paths::is_within(path, Path::new(&root.path)) {
                let should_replace = matching
                    .as_ref()
                    .map(|current| root.path.len() > current.path.len())
                    .unwrap_or(true);
                if should_replace {
                    matching = Some(root);
                }
            }
        }
        Ok(matching.map(|root| root.id))
    }

    fn get_scan_root(&self, id: &str) -> Result<ScanRoot> {
        self.conn
            .query_row(
                "SELECT id, path, created_at, last_scanned_at FROM scan_roots WHERE id = ?1",
                params![id],
                scan_root_from_row,
            )
            .map_err(|_| Error::NotFound(id.into()))
    }

    fn setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?)
    }

    fn put_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Run a metadata mutation while holding SQLite's write lock from the
    /// state check through the final delete/update. This closes the race in
    /// which a second Core connection could approve/start a task between an
    /// "is it running?" query and project deletion.
    fn with_immediate_transaction<T>(
        &self,
        operation: impl FnOnce(&Self) -> Result<T>,
    ) -> Result<T> {
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        match operation(self) {
            Ok(value) => match self.conn.execute_batch("COMMIT") {
                Ok(()) => Ok(value),
                Err(error) => {
                    let _ = self.conn.execute_batch("ROLLBACK");
                    Err(error.into())
                }
            },
            Err(error) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }
}

fn scan_root_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScanRoot> {
    Ok(ScanRoot {
        id: row.get(0)?,
        path: row.get(1)?,
        created_at: row.get(2)?,
        last_scanned_at: row.get(3)?,
    })
}

fn row_to_summary(row: &rusqlite::Row<'_>) -> Result<ProjectSummary> {
    Ok(ProjectSummary {
        directory_group: false,
        id: row.get(0)?,
        canonical_path: row.get(1)?,
        display_name: row.get(2)?,
        detected_name: row.get(3)?,
        notes: row.get(4)?,
        description: row.get(5)?,
        vcs_kind: row.get(6)?,
        availability: row.get(7)?,
        archived: row.get::<_, i64>(8)? != 0,
        favorite: row.get::<_, i64>(9)? != 0,
        origin: row.get(10)?,
        scan_root_id: row.get(11)?,
        languages: from_json(row.get(12)?)?,
        frameworks: from_json(row.get(13)?)?,
        package_managers: from_json(row.get(14)?)?,
        tags: Vec::new(),
        source_mtime: row.get(15)?,
        last_commit_at: row.get(16)?,
        last_opened_at: row.get(17)?,
        updated_at: row.get(18)?,
    })
}

fn row_to_detail(row: &rusqlite::Row<'_>) -> Result<ProjectDetail> {
    let summary = ProjectSummary {
        directory_group: false,
        id: row.get(0)?,
        canonical_path: row.get(1)?,
        display_name: row.get(2)?,
        detected_name: row.get(3)?,
        notes: row.get(4)?,
        description: row.get(5)?,
        vcs_kind: row.get(6)?,
        availability: row.get(7)?,
        archived: row.get::<_, i64>(8)? != 0,
        favorite: row.get::<_, i64>(9)? != 0,
        origin: row.get(10)?,
        scan_root_id: row.get(11)?,
        languages: from_json(row.get(12)?)?,
        frameworks: from_json(row.get(13)?)?,
        package_managers: from_json(row.get(14)?)?,
        tags: Vec::new(),
        source_mtime: row.get(21)?,
        last_commit_at: row.get(22)?,
        last_opened_at: row.get(23)?,
        updated_at: row.get(24)?,
    };
    let facts: Vec<crate::models::DetectedFact> = from_json(row.get(15)?)?;
    Ok(ProjectDetail {
        modules: Vec::new(),
        project: summary,
        facts: facts.clone(),
        readme_path: row.get(19)?,
        readme_excerpt: row.get(20)?,
        tasks: from_json(row.get(16)?)?,
        git: opt_from_json(row.get(18)?)?,
        dependencies: opt_from_json(row.get(17)?)?,
        runtime_requirements: crate::environment::runtime_requirements_from_facts(&facts),
        project_files: crate::environment::project_files_from_facts(&facts),
        start_here: Vec::new(),
        lineage: None,
        recent_events: Vec::new(),
    })
}

fn render_atlas_report(
    detail: &ProjectDetail,
    environment: Option<&crate::models::EnvironmentInspection>,
) -> String {
    let project = &detail.project;
    let mut lines = vec![
        format!("# {}", project.display_name),
        String::new(),
        format!("- Path: {}", project.canonical_path),
        format!("- VCS: {}", project.vcs_kind),
        format!("- Availability: {}", project.availability),
    ];
    if let Some(description) = &project.description {
        lines.push(format!("- Description: {description}"));
    }
    if !project.languages.is_empty() {
        lines.push(format!("- Languages: {}", project.languages.join(", ")));
    }
    if !project.frameworks.is_empty() {
        lines.push(format!("- Frameworks: {}", project.frameworks.join(", ")));
    }
    lines.push(String::new());
    lines.push("## Start here".into());
    if detail.start_here.is_empty() {
        lines.push("No inferred or saved start tasks.".into());
    } else {
        for task in &detail.start_here {
            lines.push(format!("- {} {} -- {}", task.kind, task.name, task.source));
        }
    }
    lines.push(String::new());
    lines.push("## Environment".into());
    if let Some(environment) = environment {
        if environment.runtimes.is_empty() {
            lines.push("No runtime constraints detected.".into());
        } else {
            for runtime in &environment.runtimes {
                lines.push(format!(
                    "- {}: required {}, local {}, {}",
                    runtime.label,
                    runtime.constraint.as_deref().unwrap_or("undeclared"),
                    runtime.local_version.as_deref().unwrap_or("missing"),
                    runtime.match_state
                ));
            }
        }
        if !environment.files.is_empty() {
            lines.push(String::new());
            lines.push("## Evidence files".into());
            for file in &environment.files {
                lines.push(format!("- {} ({})", file.path, file.kind));
            }
        }
    } else {
        lines.push("Environment inspection unavailable.".into());
    }
    lines.push(String::new());
    lines.push("## Checkout lineage".into());
    match &detail.lineage {
        Some(lineage) => {
            if let Some(remote) = &lineage.remote_url {
                lines.push(format!("- Origin: {remote}"));
            }
            if lineage.checkouts.is_empty() {
                lines.push("- No other local checkouts found.".into());
            } else {
                for checkout in &lineage.checkouts {
                    lines.push(format!(
                        "- {} ({})",
                        checkout.display_name, checkout.canonical_path
                    ));
                }
            }
        }
        None => lines.push("No shared repository lineage detected.".into()),
    }
    lines.push(String::new());
    lines.push("## Recent activity".into());
    if detail.recent_events.is_empty() {
        lines.push("No recorded RepoAtlas activity yet.".into());
    } else {
        for event in &detail.recent_events {
            lines.push(format!("- {} -- {}", event.created_at, event.title));
        }
    }
    if let Some(notes) = &project.notes {
        lines.push(String::new());
        lines.push("## Notes".into());
        lines.push(notes.clone());
    }
    lines.join(char::from(10).to_string().as_str())
}

fn start_here_tasks(
    tasks: &[TaskDefinition],
    facts: &[crate::models::DetectedFact],
) -> Vec<StartHereTask> {
    let mut selected = Vec::new();
    for kind in ["dev", "test", "build"] {
        let custom = tasks
            .iter()
            .find(|task| !task.inferred && task.kind.eq_ignore_ascii_case(kind));
        let inferred = tasks
            .iter()
            .find(|task| task.inferred && task.kind.eq_ignore_ascii_case(kind));
        let Some(task) = custom.or(inferred) else {
            continue;
        };
        selected.push(StartHereTask {
            task_id: task.id.clone(),
            kind: task.kind.clone(),
            name: task.name.clone(),
            source: start_here_source(task, facts),
            inferred: task.inferred,
        });
        if selected.len() == 3 {
            break;
        }
    }
    if selected.is_empty() {
        for task in tasks.iter().take(3) {
            selected.push(StartHereTask {
                task_id: task.id.clone(),
                kind: task.kind.clone(),
                name: task.name.clone(),
                source: start_here_source(task, facts),
                inferred: task.inferred,
            });
        }
    }
    selected
}

fn start_here_source(task: &TaskDefinition, facts: &[crate::models::DetectedFact]) -> String {
    if !task.inferred {
        return "user-defined task".into();
    }

    // JavaScript tasks are generated from package.json scripts. Keep the
    // evidence addressable so users can understand exactly why a task was
    // suggested, rather than seeing only a reconstructed command string.
    if matches!(
        task.executable.to_ascii_lowercase().as_str(),
        "npm" | "pnpm" | "yarn" | "bun"
    ) && task.argv.len() >= 2
        && matches!(task.argv[0].as_str(), "run" | "run-script")
        && !task.argv[1].trim().is_empty()
    {
        return format!("package.json#scripts.{}", task.argv[1]);
    }

    // For other ecosystems prefer the detected manifest as the source of the
    // inferred task. The description remains as a useful fallback when no
    // manifest fact was recorded.
    let manifest = match task.executable.to_ascii_lowercase().as_str() {
        "cargo" => Some("Cargo.toml"),
        "go" => Some("go.mod"),
        "mvn" | "mvnw" => Some("pom.xml"),
        "gradle" | "gradlew" => Some("build.gradle"),
        "poetry" => Some("pyproject.toml"),
        "pipenv" => Some("Pipfile"),
        "dotnet" => facts
            .iter()
            .find(|fact| fact.kind == "manifest" && fact.value.ends_with("proj"))
            .map(|fact| fact.value.as_str()),
        _ => None,
    };
    if let Some(manifest) = manifest {
        if facts
            .iter()
            .any(|fact| fact.kind == "manifest" && fact.value.eq_ignore_ascii_case(manifest))
        {
            return manifest.into();
        }
    }

    task.description
        .clone()
        .unwrap_or_else(|| format!("detected {} task", task.kind))
}

fn git_action_name(op: &GitOp) -> &'static str {
    match op {
        GitOp::Status => "git_status",
        GitOp::Diff { .. } => "git_diff",
        GitOp::Log { .. } => "git_log",
        GitOp::Branch => "git_branch",
        GitOp::Checkout { .. } => "git_checkout",
        GitOp::Stage { .. } => "git_stage",
        GitOp::Unstage { .. } => "git_unstage",
        GitOp::Commit { .. } => "git_commit",
        GitOp::Pull => "git_pull",
        GitOp::Push => "git_push",
        GitOp::StashPush { .. } => "git_stash_push",
        GitOp::StashPop => "git_stash_pop",
    }
}

fn to_json<T: serde::Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(|err| Error::msg(err.to_string()))
}

fn opt_json<T: serde::Serialize>(value: &Option<T>) -> Result<Option<String>> {
    match value {
        Some(value) => Ok(Some(to_json(value)?)),
        None => Ok(None),
    }
}

fn from_json<T: DeserializeOwned>(raw: String) -> Result<T> {
    serde_json::from_str(&raw).map_err(|err| Error::msg(err.to_string()))
}

fn opt_from_json<T: DeserializeOwned>(raw: Option<String>) -> Result<Option<T>> {
    match raw {
        Some(raw) => Ok(Some(from_json(raw)?)),
        None => Ok(None),
    }
}

fn merge_tasks(
    existing: Vec<TaskDefinition>,
    detected: Vec<TaskDefinition>,
) -> Vec<TaskDefinition> {
    let key = |task: &TaskDefinition| {
        (
            task.kind.clone(),
            task.executable.clone(),
            task.argv.clone(),
            task.cwd.clone(),
        )
    };
    let mut merged: Vec<_> = existing
        .iter()
        .filter(|task| !task.inferred)
        .cloned()
        .collect();
    let mut seen = merged
        .iter()
        .map(&key)
        .collect::<std::collections::BTreeSet<_>>();
    for mut task in detected {
        let signature = key(&task);
        if seen.insert(signature.clone()) {
            if let Some(old) = existing.iter().find(|old| key(old) == signature) {
                task.id = old.id.clone();
            }
            merged.push(task);
        }
    }
    merged
}

fn unicode_match_query(query: &str) -> Option<String> {
    let terms: Vec<String> = query
        .split_whitespace()
        .map(|term| term.replace(['"', '*', '(', ')', ':'], " "))
        .flat_map(|term| {
            term.split_whitespace()
                .map(|part| format!("{part}*"))
                .collect::<Vec<_>>()
        })
        .collect();
    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" OR "))
    }
}

fn trigram_match_query(query: &str) -> Option<String> {
    let cleaned = query.replace('"', " ").trim().to_string();
    if cleaned.chars().count() < 2 {
        None
    } else {
        Some(format!("\"{}\"", cleaned))
    }
}

fn normalize_task_runtime_metadata(task: &mut TaskDefinition) -> Result<()> {
    task.expected_ports.sort_unstable();
    task.expected_ports.dedup();
    if task.expected_ports.contains(&0) {
        return Err(Error::msg(
            "expected task ports must be between 1 and 65535",
        ));
    }
    task.dev_url_path = task
        .dev_url_path
        .take()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if task
        .dev_url_path
        .as_ref()
        .is_some_and(|path| !path.starts_with('/') || path.starts_with("//"))
    {
        return Err(Error::msg("development URL path must start with one slash"));
    }
    task.dev_url_scheme = task
        .dev_url_scheme
        .take()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    if task
        .dev_url_scheme
        .as_deref()
        .is_some_and(|scheme| !matches!(scheme, "http" | "https"))
    {
        return Err(Error::msg("development URL scheme must be http or https"));
    }
    Ok(())
}

fn attention_severity_rank(severity: &str) -> u8 {
    match severity {
        "error" => 3,
        "action" => 2,
        "warning" => 1,
        _ => 0,
    }
}

fn validate_collection(upsert: CollectionUpsert) -> Result<(String, Option<String>)> {
    let name = upsert.name.trim().to_string();
    if name.is_empty() {
        return Err(Error::msg("collection name is required"));
    }
    if name.chars().count() > 80 {
        return Err(Error::msg("collection name must be 80 characters or fewer"));
    }
    let description = upsert
        .description
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if description
        .as_ref()
        .is_some_and(|value| value.chars().count() > 500)
    {
        return Err(Error::msg(
            "collection description must be 500 characters or fewer",
        ));
    }
    Ok((name, description))
}

fn map_collection_constraint(error: rusqlite::Error) -> Error {
    if error.to_string().contains("UNIQUE constraint failed") {
        Error::msg("a collection with this name already exists")
    } else {
        error.into()
    }
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn stable_fingerprint(value: &str) -> String {
    // FNV-1a is intentionally simple and deterministic across processes and
    // Rust releases. This marks a condition version; it is not a cryptographic digest.
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn runtime_lock_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("repoatlas.sqlite");
    path.with_file_name(format!(".{file_name}.runtime.lock"))
}

/// Claim the desktop runtime lock without relying on SQLite's cooperative
/// locking.  The lock file lives beside the database and is intentionally
/// separate from the database itself so a second process can still open a
/// read/write SQLite connection for MCP data management while the desktop
/// broker owns the live-process recovery decision.
fn acquire_runtime_lock(path: &Path, required: bool) -> Result<Option<File>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let lock_path = runtime_lock_path(path);
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(lock_path)?;
    match file.try_lock_exclusive() {
        Ok(()) => Ok(Some(file)),
        Err(_error) if !required => {
            // The desktop process owns the lock.  Keep the file handle alive
            // only long enough for this branch; MCP deliberately proceeds
            // without recovery in this case.
            drop(file);
            Ok(None)
        }
        Err(error) => Err(Error::msg(format!(
            "another RepoAtlas desktop instance owns the database: {error}"
        ))),
    }
}
