use crate::environment;
use crate::error::{Error, Result};
use crate::models::{AppSettings, ScanRoot};
use crate::paths;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportFile {
    pub format: String,
    pub version: u32,
    pub exported_at: String,
    pub settings: AppSettings,
    pub scan_roots: Vec<ScanRoot>,
    pub projects: Vec<ExportProject>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProject {
    pub canonical_path: String,
    pub display_name: String,
    pub notes: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub favorite: bool,
    pub archived: bool,
    pub tags: Vec<String>,
    #[serde(default)]
    pub last_opened_at: Option<String>,
    #[serde(default)]
    pub icon: Option<ExportProjectIcon>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProjectIcon {
    pub mime_type: String,
    pub data: String,
    pub source_name: Option<String>,
}

pub fn export(conn: &Connection) -> Result<String> {
    let settings = AppSettings {
        theme: setting(conn, "theme")?.unwrap_or_else(|| "system".into()),
        locale: setting(conn, "locale")?.unwrap_or_else(|| "system".into()),
        ui_font: setting(conn, "uiFont")?.unwrap_or_default(),
        console_font: setting(conn, "consoleFont")?.unwrap_or_default(),
    };
    let scan_roots = list_scan_roots(conn)?;
    let projects = export_projects(conn)?;
    let file = ExportFile {
        format: "repoatlas-export".into(),
        version: 3,
        exported_at: now(),
        settings,
        scan_roots,
        projects,
    };
    serde_json::to_string_pretty(&file).map_err(|err| Error::msg(err.to_string()))
}

pub fn import_into(
    conn: &Connection,
    text: &str,
    root_for: &dyn Fn(&Path) -> Option<String>,
) -> Result<usize> {
    let file: ExportFile =
        serde_json::from_str(text).map_err(|err| Error::msg(format!("invalid export: {err}")))?;
    if file.format != "repoatlas-export" {
        return Err(Error::msg("not a RepoAtlas export file"));
    }

    // Validate every filesystem-backed value before taking the database write
    // lock. An invalid root or icon must not leave a partially imported store.
    let scan_roots = file
        .scan_roots
        .iter()
        .map(validate_scan_root)
        .collect::<Result<Vec<_>>>()?;
    let icons = file
        .projects
        .iter()
        .map(|project| {
            project
                .icon
                .as_ref()
                .map(|icon| {
                    let bytes = decode_base64(&icon.data)?;
                    let mime = environment::validate_icon_payload(Some(&icon.mime_type), &bytes)?;
                    Ok(ValidatedIcon { mime, bytes })
                })
                .transpose()
        })
        .collect::<Result<Vec<_>>>()?;

    conn.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| {
        put_setting(conn, "theme", &file.settings.theme)?;
        put_setting(conn, "locale", &file.settings.locale)?;
        put_setting(conn, "uiFont", &file.settings.ui_font)?;
        put_setting(conn, "consoleFont", &file.settings.console_font)?;

        for root in &scan_roots {
            let existing_id: Option<String> = conn
                .query_row(
                    "SELECT id FROM scan_roots WHERE path = ?1",
                    params![root.path],
                    |row| row.get(0),
                )
                .optional()?;
            if existing_id.is_some() {
                continue;
            }
            let id_in_use: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM scan_roots WHERE id = ?1)",
                params![root.id],
                |row| row.get(0),
            )?;
            let id = if root.id.is_empty() || id_in_use {
                uuid::Uuid::new_v4().to_string()
            } else {
                root.id.clone()
            };
            conn.execute(
                "INSERT INTO scan_roots (id, path, created_at, last_scanned_at) VALUES (?1, ?2, ?3, ?4)",
                params![id, root.path, root.created_at, root.last_scanned_at],
            )?;
        }

        let mut count = 0;
        for (index, project) in file.projects.iter().enumerate() {
            let path = Path::new(&project.canonical_path);
            if !path.is_dir() {
                continue;
            }
            let canonical = paths::canonicalize(path)?;
            if !canonical.is_dir() {
                continue;
            }
            let scan_root_id = root_for(&canonical);
            let path_string = paths::path_to_string(&canonical);
            let id: Option<String> = conn
                .query_row(
                    "SELECT id FROM projects WHERE canonical_path = ?1",
                    params![path_string],
                    |row| row.get(0),
                )
                .optional()?;
            let id = id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let vcs_kind = if canonical.join(".git").exists() {
                "git"
            } else if canonical.join(".svn").exists() {
                "svn"
            } else {
                "none"
            };
            let now = now();
            conn.execute(
                r#"
                INSERT INTO projects (id, canonical_path, display_name, detected_name, notes, description, vcs_kind, availability,
                    archived, favorite, origin, scan_root_id, languages_json, frameworks_json, package_managers_json,
                    facts_json, tasks_json, search_blob, last_opened_at, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'ready', ?8, ?9, 'import', ?10, '[]', '[]', '[]', '[]', '[]', '', ?11, ?12, ?12)
                ON CONFLICT(canonical_path) DO UPDATE SET
                    display_name=excluded.display_name, notes=excluded.notes,
                    description=COALESCE(excluded.description, projects.description), archived=excluded.archived,
                    favorite=excluded.favorite, last_opened_at=excluded.last_opened_at, updated_at=excluded.updated_at
                "#,
                params![
                    id, path_string, project.display_name, project.display_name, project.notes, project.description,
                    vcs_kind, project.archived as i64, project.favorite as i64, scan_root_id,
                    project.last_opened_at, now,
                ],
            )?;
            conn.execute(
                "DELETE FROM project_tags WHERE project_id = ?1",
                params![id],
            )?;
            let mut tags_for_search = Vec::new();
            for tag in &project.tags {
                let tag = tag.trim();
                if !tag.is_empty() {
                    tags_for_search.push(tag.to_string());
                    conn.execute(
                        "INSERT OR IGNORE INTO project_tags (project_id, tag) VALUES (?1, ?2)",
                        params![id, tag],
                    )?;
                }
            }
            let search_blob = [
                project.display_name.clone(),
                path_string.clone(),
                project.notes.clone().unwrap_or_default(),
                project.description.clone().unwrap_or_default(),
                tags_for_search.join(" "),
            ]
            .join("\n");
            conn.execute(
                "UPDATE projects SET search_blob = ?1 WHERE id = ?2",
                params![search_blob, id],
            )?;
            if let Some(icon) = &project.icon {
                let validated = icons
                    .get(index)
                    .and_then(|icon| icon.as_ref())
                    .ok_or_else(|| Error::msg("validated project icon is missing"))?;
                conn.execute(
                    "INSERT INTO project_icon_overrides (project_id, mime_type, bytes, source_name, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(project_id) DO UPDATE SET mime_type = excluded.mime_type, bytes = excluded.bytes,
                     source_name = excluded.source_name, updated_at = excluded.updated_at",
                    params![id, validated.mime, validated.bytes, icon.source_name, now],
                )?;
            }
            for table in ["project_fts_unicode", "project_fts_trigram"] {
                conn.execute(
                    &format!("DELETE FROM {table} WHERE project_id = ?1"),
                    params![id],
                )?;
                conn.execute(
                    &format!("INSERT INTO {table} (project_id, content) VALUES (?1, ?2)"),
                    params![id, search_blob],
                )?;
            }
            count += 1;
        }
        Ok(count)
    })();
    match result {
        Ok(count) => match conn.execute_batch("COMMIT") {
            Ok(()) => Ok(count),
            Err(error) => {
                let _ = conn.execute_batch("ROLLBACK");
                Err(error.into())
            }
        },
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

struct ValidatedIcon {
    mime: String,
    bytes: Vec<u8>,
}

fn validate_scan_root(root: &ScanRoot) -> Result<ScanRoot> {
    let path = Path::new(&root.path);
    let metadata = fs::symlink_metadata(path)?;
    if is_reparse_point(&metadata) {
        return Err(Error::msg("scan root cannot be a symbolic link"));
    }
    let canonical = paths::canonicalize(path)?;
    if !canonical.is_dir() {
        return Err(Error::msg(format!(
            "scan root is not a directory: {}",
            paths::path_to_string(&canonical)
        )));
    }
    if canonical.parent().is_none() || canonical.parent() == Some(canonical.as_path()) {
        return Err(Error::msg(
            "filesystem root cannot be imported as a scan root",
        ));
    }
    Ok(ScanRoot {
        id: root.id.clone(),
        path: paths::path_to_string(&canonical),
        created_at: root.created_at.clone(),
        last_scanned_at: root.last_scanned_at.clone(),
    })
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

pub fn backup_to(conn: &Connection, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut backup_conn = Connection::open(dest)?;
    let backup = rusqlite::backup::Backup::new(conn, &mut backup_conn)?;
    backup.run_to_completion(1000, std::time::Duration::from_millis(50), None)?;
    Ok(())
}

fn export_projects(conn: &Connection) -> Result<Vec<ExportProject>> {
    let mut stmt = conn.prepare(
        "SELECT canonical_path, display_name, notes, description, favorite, archived, last_opened_at FROM projects ORDER BY display_name",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, i64>(4)? != 0,
            row.get::<_, i64>(5)? != 0,
            row.get::<_, Option<String>>(6)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (path, name, notes, description, favorite, archived, last_opened_at) = row?;
        let id: String = conn.query_row(
            "SELECT id FROM projects WHERE canonical_path = ?1",
            params![path],
            |r| r.get(0),
        )?;
        let mut stmt =
            conn.prepare("SELECT tag FROM project_tags WHERE project_id = ?1 ORDER BY tag")?;
        let tags = stmt
            .query_map(params![id], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let icon = conn
            .query_row(
                "SELECT mime_type, bytes, source_name FROM project_icon_overrides WHERE project_id = ?1",
                params![id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?, row.get::<_, Option<String>>(2)?)),
            )
            .optional()?
            .map(|(mime_type, bytes, source_name)| ExportProjectIcon {
                mime_type,
                data: environment::encode_base64(&bytes),
                source_name,
            });
        out.push(ExportProject {
            canonical_path: path,
            display_name: name,
            notes,
            description,
            favorite,
            archived,
            tags,
            last_opened_at,
            icon,
        });
    }
    Ok(out)
}

fn list_scan_roots(conn: &Connection) -> Result<Vec<ScanRoot>> {
    let mut stmt = conn.prepare(
        "SELECT id, path, created_at, last_scanned_at FROM scan_roots ORDER BY created_at",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ScanRoot {
                id: row.get(0)?,
                path: row.get(1)?,
                created_at: row.get(2)?,
                last_scanned_at: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |r| r.get(0),
        )
        .optional()?)
}

fn put_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute("INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value", params![key, value])?;
    Ok(())
}

fn decode_base64(value: &str) -> Result<Vec<u8>> {
    let bytes = value.as_bytes();
    if bytes.is_empty() || !bytes.len().is_multiple_of(4) {
        return Err(Error::msg("invalid project icon data"));
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks_exact(4) {
        let mut values = [0u8; 4];
        let mut padding = 0;
        for (index, byte) in chunk.iter().copied().enumerate() {
            values[index] = match byte {
                b'A'..=b'Z' => byte - b'A',
                b'a'..=b'z' => byte - b'a' + 26,
                b'0'..=b'9' => byte - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                b'=' if index >= 2 => {
                    padding += 1;
                    0
                }
                _ => return Err(Error::msg("invalid project icon data")),
            };
        }
        if padding > 2 || (padding > 0 && chunk[3] != b'=') {
            return Err(Error::msg("invalid project icon data"));
        }
        let value = (u32::from(values[0]) << 18)
            | (u32::from(values[1]) << 12)
            | (u32::from(values[2]) << 6)
            | u32::from(values[3]);
        out.push((value >> 16) as u8);
        if padding < 2 {
            out.push((value >> 8) as u8);
        }
        if padding == 0 {
            out.push(value as u8);
        }
    }
    Ok(out)
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}
