use super::*;
use crate::models::ProjectModule;

impl Core {
    pub(super) fn module_at_path(&self, path: &Path) -> Result<Option<(String, String)>> {
        let row: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT project_id, snapshot_json FROM project_modules WHERE canonical_path = ?1",
                [paths::path_to_string(path)],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        row.map(|(owner, json)| Ok((owner, from_json::<ProjectModule>(json)?.id)))
            .transpose()
    }

    pub(super) fn is_directory_group(&self, id: &str) -> Result<bool> {
        Ok(self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM project_directory_groups WHERE project_id = ?1)",
            [id],
            |row| row.get(0),
        )?)
    }
    pub fn list_modules(&self, project_id: &str) -> Result<Vec<ProjectModule>> {
        let mut stmt = self.conn.prepare("SELECT snapshot_json FROM project_modules WHERE project_id = ?1 ORDER BY canonical_path")?;
        let rows = stmt
            .query_map([project_id], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut modules: Vec<ProjectModule> =
            rows.into_iter().map(from_json).collect::<Result<_>>()?;
        let (root, tasks_json): (String, String) = self.conn.query_row(
            "SELECT canonical_path, tasks_json FROM projects WHERE id = ?1",
            [project_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let tasks: Vec<TaskDefinition> = from_json(tasks_json)?;
        for module in &mut modules {
            module.project_id = self
                .conn
                .query_row(
                    "SELECT id FROM projects WHERE canonical_path = ?1",
                    [&module.canonical_path],
                    |row| row.get(0),
                )
                .optional()?;
            module.task_ids.clear();
        }
        for task in tasks {
            let Some(cwd) = task.cwd.as_deref() else {
                continue;
            };
            let cwd = Path::new(&root).join(cwd);
            if let Some(module) = modules
                .iter_mut()
                .filter(|m| paths::is_within(&cwd, Path::new(&m.canonical_path)))
                .max_by_key(|m| m.canonical_path.len())
            {
                if module.project_id.is_none() {
                    module.task_ids.push(task.id);
                }
            }
        }
        Ok(modules)
    }

    // Called in parent-before-child discovery order. Existing Projects always
    // retain identity, including manually promoted directories without VCS.
    pub(super) fn ingest_directory(
        &self,
        path: &Path,
        root_id: Option<&str>,
        boundary: &Path,
    ) -> Result<String> {
        let path = paths::canonicalize(path)?;
        if !paths::is_within(&path, boundary) {
            return Err(Error::msg("discovery escaped its authorization boundary"));
        }
        let mut parent = None;
        for ancestor in path.ancestors().skip(1) {
            // Ownership comes from cached Project identity, including an
            // ancestor outside a narrower Scan Root. No ancestor files are read.
            if let Some(id) = self
                .conn
                .query_row(
                    "SELECT id FROM projects WHERE canonical_path = ?1",
                    [paths::path_to_string(ancestor)],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
            {
                parent = self.get_project_summary(&id)?;
                break;
            }
        }
        let existing: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM projects WHERE canonical_path = ?1",
                [paths::path_to_string(&path)],
                |row| row.get(0),
            )
            .optional()?;
        let independent = existing.is_some()
            || path.join(".git").exists()
            || path.join(".svn").exists()
            || parent.as_ref().is_none_or(|p| p.directory_group);
        let project = if independent {
            Some(self.upsert_project(&path, root_id, "scan")?)
        } else {
            None
        };
        if let Some(parent) = parent {
            self.observe_module(&parent, &path, project.as_ref().map(|p| p.id.clone()))?;
            Ok(project.map(|p| p.id).unwrap_or(parent.id))
        } else {
            Ok(project.expect("root is independent").id)
        }
    }

    fn observe_module(
        &self,
        parent: &ProjectSummary,
        path: &Path,
        project_id: Option<String>,
    ) -> Result<()> {
        let canonical_path = paths::path_to_string(path);
        let old: Option<String> = self
            .conn
            .query_row(
                "SELECT snapshot_json FROM project_modules WHERE canonical_path = ?1",
                [&canonical_path],
                |row| row.get(0),
            )
            .optional()?;
        let old: Option<ProjectModule> = old.map(from_json).transpose()?;
        let id = old
            .as_ref()
            .map(|m| m.id.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let relative_path = path
            .strip_prefix(&parent.canonical_path)
            .map_err(|_| Error::msg("invalid module location"))?
            .to_string_lossy()
            .replace('\\', "/");
        let detection = detect::detect(path);
        let mut tasks: Vec<TaskDefinition> = from_json(self.conn.query_row(
            "SELECT tasks_json FROM projects WHERE id = ?1",
            [&parent.id],
            |row| row.get(0),
        )?)?;
        let prefix = format!("module:{id}:");
        let old_owner: Option<String> = self
            .conn
            .query_row(
                "SELECT project_id FROM project_modules WHERE canonical_path = ?1",
                [&canonical_path],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(owner) = old_owner.filter(|owner| owner != &parent.id) {
            let moving_ids: HashSet<_> = self
                .list_modules(&owner)?
                .into_iter()
                .find(|module| module.id == id)
                .map(|module| module.task_ids)
                .unwrap_or_default()
                .into_iter()
                .collect();
            let mut old_tasks: Vec<TaskDefinition> = from_json(self.conn.query_row(
                "SELECT tasks_json FROM projects WHERE id = ?1",
                [&owner],
                |row| row.get(0),
            )?)?;
            tasks.extend(
                old_tasks
                    .iter()
                    .filter(|task| task.id.starts_with(&prefix) || moving_ids.contains(&task.id))
                    .cloned(),
            );
            old_tasks
                .retain(|task| !task.id.starts_with(&prefix) && !moving_ids.contains(&task.id));
            self.conn.execute(
                "UPDATE projects SET tasks_json = ?1, updated_at = ?2 WHERE id = ?3",
                params![to_json(&old_tasks)?, now(), owner],
            )?;
        }
        let previous: Vec<_> = tasks
            .iter()
            .filter(|task| task.id.starts_with(&prefix))
            .cloned()
            .collect();
        tasks.retain(|task| !task.id.starts_with(&prefix) || !task.inferred);
        let mut task_ids = Vec::new();
        if project_id.is_none() {
            for mut task in detection.tasks {
                task.cwd = Some(match task.cwd.as_deref() {
                    Some(cwd) => paths::path_to_string(&path.join(cwd)),
                    None => canonical_path.clone(),
                });
                task.name = format!("{relative_path} · {}", task.name);
                let existing = previous.iter().find(|old| {
                    old.executable == task.executable
                        && old.argv == task.argv
                        && old.cwd == task.cwd
                });
                task.id = existing
                    .map(|old| old.id.clone())
                    .unwrap_or_else(|| format!("{prefix}{}", Uuid::new_v4()));
                task_ids.push(task.id.clone());
                if !tasks.iter().any(|saved| saved.id == task.id) {
                    tasks.push(task);
                }
            }
            for task in &previous {
                if !task.inferred && !task_ids.contains(&task.id) {
                    task_ids.push(task.id.clone());
                }
            }
        }
        let module = ProjectModule {
            id,
            canonical_path,
            relative_path: relative_path.clone(),
            name: detection.name,
            languages: detection.languages,
            frameworks: detection.frameworks,
            package_managers: detection.package_managers,
            runtime_requirements: environment::runtime_requirements_from_facts(&detection.facts),
            evidence: if project_id.is_some() {
                "independent-project".into()
            } else {
                workspace_evidence(Path::new(&parent.canonical_path), &relative_path)
                    .unwrap_or_else(|| "manifest-candidate".into())
            },
            facts: detection.facts,
            task_ids,
            observed_at: now(),
            availability: "ready".into(),
            project_id,
        };
        self.conn.execute("INSERT INTO project_modules (canonical_path, project_id, snapshot_json) VALUES (?1, ?2, ?3) ON CONFLICT(canonical_path) DO UPDATE SET project_id = excluded.project_id, snapshot_json = excluded.snapshot_json", params![module.canonical_path, parent.id, to_json(&module)?])?;
        self.conn.execute(
            "UPDATE projects SET tasks_json = ?1, updated_at = ?2 WHERE id = ?3",
            params![to_json(&tasks)?, now(), parent.id],
        )?;
        let union = |own: &[String], nested: &[String]| {
            let mut values = own.to_vec();
            values.extend_from_slice(nested);
            values.sort();
            values.dedup();
            values
        };
        self.conn.execute("UPDATE projects SET languages_json = ?1, frameworks_json = ?2, package_managers_json = ?3 WHERE id = ?4", params![to_json(&union(&parent.languages, &module.languages))?, to_json(&union(&parent.frameworks, &module.frameworks))?, to_json(&union(&parent.package_managers, &module.package_managers))?, parent.id])?;
        self.reindex_project(&parent.id)?;
        Ok(())
    }

    pub(super) fn discover_modules(&self, project_id: &str) -> Result<()> {
        let project = self
            .get_project_summary(project_id)?
            .ok_or_else(|| Error::NotFound(project_id.into()))?;
        let boundary = PathBuf::from(&project.canonical_path);
        let cancel = AtomicBool::new(false);
        let engine = ScanEngine {
            scan_id: Uuid::new_v4().to_string(),
            root: boundary.clone(),
            cancel: &cancel,
            on_progress: &|_| {},
        };
        let (items, _, errors, _) = engine.walk()?;
        for item in items.iter().filter(|item| item.path != boundary) {
            self.ingest_directory(&item.path, project.scan_root_id.as_deref(), &boundary)?;
        }
        self.update_missing_modules(project_id)?;
        if !errors.is_empty() {
            return Err(Error::msg(format!(
                "Module discovery incomplete: {}",
                errors.join("; ")
            )));
        }
        Ok(())
    }

    pub(super) fn update_missing_modules(&self, project_id: &str) -> Result<()> {
        for mut module in self.list_modules(project_id)? {
            let path = Path::new(&module.canonical_path);
            if !fs::symlink_metadata(path)
                .is_ok_and(|meta| !crate::scan::is_link(&meta) && meta.is_dir())
                || (module.project_id.is_none() && !detect::is_project_root(path))
            {
                module.availability = "unavailable".into();
                self.conn.execute(
                    "UPDATE project_modules SET snapshot_json = ?1 WHERE canonical_path = ?2",
                    params![to_json(&module)?, module.canonical_path],
                )?;
            }
        }
        Ok(())
    }

    pub fn resolve_module_path(&self, project_id: &str, module_id: &str) -> Result<String> {
        let project = self
            .get_project_summary(project_id)?
            .ok_or_else(|| Error::NotFound(project_id.into()))?;
        let module = self
            .list_modules(project_id)?
            .into_iter()
            .find(|m| m.id == module_id)
            .ok_or_else(|| Error::NotFound(module_id.into()))?;
        let path = Path::new(&module.canonical_path);
        // Reject replaced directories/junctions before launch or promotion.
        for ancestor in path.ancestors() {
            if crate::scan::is_link(&fs::symlink_metadata(ancestor)?) {
                return Err(Error::msg("module links are not allowed"));
            }
            if ancestor == Path::new(&project.canonical_path) {
                break;
            }
        }
        let canonical = paths::canonicalize(path)?;
        if !canonical.is_dir() || !paths::is_within(&canonical, Path::new(&project.canonical_path))
        {
            return Err(Error::msg("module escaped Project boundary"));
        }
        Ok(paths::path_to_string(&canonical))
    }

    pub fn promote_module_with_origin(
        &self,
        project_id: &str,
        module_id: &str,
        origin: &str,
    ) -> Result<ProjectSummary> {
        let path = self.resolve_module_path(project_id, module_id)?;
        self.with_immediate_transaction(|core| {
            core.promote_module_inner(project_id, module_id, origin, &path)
        })
    }

    pub(super) fn promote_module_inner(
        &self,
        project_id: &str,
        module_id: &str,
        origin: &str,
        path: &str,
    ) -> Result<ProjectSummary> {
        let core = self;
        core.ensure_no_running_projects(&[project_id.to_string()])?;
        let parent = core.get_project(project_id)?;
        let module = parent
            .modules
            .iter()
            .find(|m| m.id == module_id)
            .ok_or_else(|| Error::NotFound(module_id.into()))?;
        let project = core.upsert_project(
            Path::new(&path),
            parent.project.scan_root_id.as_deref(),
            "manual",
        )?;
        let mut child = core.get_project(&project.id)?.tasks;
        for task in parent
            .tasks
            .iter()
            .filter(|task| module.task_ids.contains(&task.id))
        {
            // Preserve task IDs and user edits; runs remain with the original owner.
            child.retain(|saved| {
                !(saved.inferred && saved.executable == task.executable && saved.argv == task.argv)
            });
            if !child.iter().any(|saved| saved.id == task.id) {
                let mut migrated = task.clone();
                if migrated.cwd.as_deref() == Some(path) {
                    migrated.cwd = None;
                }
                child.push(migrated);
            }
        }
        let retained: Vec<_> = parent
            .tasks
            .into_iter()
            .filter(|task| !module.task_ids.contains(&task.id))
            .collect();
        core.conn.execute(
            "UPDATE projects SET tasks_json = ?1 WHERE id = ?2",
            params![to_json(&retained)?, project_id],
        )?;
        core.conn.execute(
            "UPDATE projects SET tasks_json = ?1 WHERE id = ?2",
            params![to_json(&child)?, project.id],
        )?;
        core.observe_module(&parent.project, Path::new(&path), Some(project.id.clone()))?;
        core.record_audit_event(
            origin,
            "promote_module",
            "project",
            Some(&project.id),
            Some("Module promoted; filesystem preserved"),
            "success",
        )?;
        core.discover_modules(&project.id)?;
        Ok(project)
    }

    pub fn set_directory_group_with_origin(
        &self,
        project_id: &str,
        enabled: bool,
        origin: &str,
    ) -> Result<ProjectSummary> {
        self.with_immediate_transaction(|core| {
            core.get_project(project_id)?;
            if enabled {
                // Promotion is explicit here. Disabling grouping never demotes Projects.
                for module in core.list_modules(project_id)? {
                    if module.project_id.is_none()
                        && core
                            .list_modules(project_id)?
                            .iter()
                            .any(|current| current.id == module.id)
                    {
                        let path = core.resolve_module_path(project_id, &module.id)?;
                        core.promote_module_inner(project_id, &module.id, origin, &path)?;
                    }
                }
            }
            if enabled {
                core.conn.execute(
                    "INSERT OR IGNORE INTO project_directory_groups (project_id) VALUES (?1)",
                    [project_id],
                )?;
            } else {
                core.conn.execute(
                    "DELETE FROM project_directory_groups WHERE project_id = ?1",
                    [project_id],
                )?;
            }
            core.conn.execute(
                "UPDATE projects SET updated_at = ?1 WHERE id = ?2",
                params![now(), project_id],
            )?;
            core.record_audit_event(
                origin,
                "set_directory_group",
                "project",
                Some(project_id),
                Some(if enabled { "group" } else { "project" }),
                "success",
            )?;
            core.get_project_summary(project_id)?
                .ok_or_else(|| Error::NotFound(project_id.into()))
        })
    }
}

// Only literal declarative membership is interpreted. No build scripts run.
fn workspace_evidence(root: &Path, relative: &str) -> Option<String> {
    let mut declarations: Vec<(&str, Vec<String>)> = Vec::new();
    if let Some(text) = detect::read_text(&root.join("package.json")) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
            let workspaces = &value["workspaces"];
            let entries = workspaces
                .as_array()
                .or_else(|| workspaces["packages"].as_array());
            declarations.push((
                "package.json#workspaces",
                entries
                    .into_iter()
                    .flatten()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect(),
            ));
        }
    }
    if let Some(text) = detect::read_text(&root.join("Cargo.toml")) {
        if let Ok(value) = text.parse::<toml::Value>() {
            let entries = value
                .get("workspace")
                .and_then(|v| v.get("members"))
                .and_then(|v| v.as_array());
            declarations.push((
                "Cargo.toml#workspace.members",
                entries
                    .into_iter()
                    .flatten()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect(),
            ));
        }
    }
    if let Some(text) = detect::read_text(&root.join("pom.xml")) {
        if let Some(section) = text
            .split("<modules>")
            .nth(1)
            .and_then(|v| v.split("</modules>").next())
        {
            declarations.push((
                "pom.xml#modules",
                section
                    .split("<module>")
                    .skip(1)
                    .filter_map(|v| v.split_once("</module>").map(|(v, _)| v.trim().to_owned()))
                    .collect(),
            ));
        }
    }
    if let Some(text) = detect::read_text(&root.join("pnpm-workspace.yaml")) {
        let mut in_packages = false;
        let mut entries = Vec::new();
        for line in text.lines() {
            if line.trim() == "packages:" {
                in_packages = true;
                continue;
            }
            if !line.trim().is_empty()
                && !line.starts_with(char::is_whitespace)
                && !line.trim_start().starts_with('#')
            {
                in_packages = false;
            }
            if in_packages {
                if let Some(value) = line.trim().strip_prefix("- ") {
                    entries.push(
                        value
                            .split(" #")
                            .next()
                            .unwrap_or(value)
                            .trim()
                            .trim_matches(['\'', '"'])
                            .to_string(),
                    );
                }
            }
        }
        declarations.push(("pnpm-workspace.yaml#packages", entries));
    }
    declarations.into_iter().find_map(|(source, patterns)| {
        let included = patterns
            .iter()
            .filter(|p| !p.starts_with('!'))
            .any(|p| member_matches(p.trim_start_matches("./"), relative));
        let excluded = patterns
            .iter()
            .filter_map(|p| p.strip_prefix('!'))
            .any(|p| member_matches(p, relative));
        (included && !excluded).then(|| source.to_string())
    })
}

fn member_matches(pattern: &str, relative: &str) -> bool {
    let pattern: Vec<_> = pattern.trim_end_matches('/').split('/').collect();
    let relative: Vec<_> = relative.split('/').collect();
    if pattern.len() > 128 || relative.len() > 256 {
        return false;
    }
    let mut next = vec![false; relative.len() + 1];
    next[relative.len()] = true;
    for part in pattern.iter().rev() {
        let mut row = vec![false; relative.len() + 1];
        for index in (0..=relative.len()).rev() {
            row[index] = if *part == "**" {
                next[index] || (index < relative.len() && row[index + 1])
            } else {
                index < relative.len()
                    && (*part == "*" || *part == relative[index])
                    && next[index + 1]
            };
        }
        next = row;
    }
    next[0]
}
