use crate::error::Result;
use rusqlite::{params, Connection};
use std::path::Path;

pub fn open(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    configure(&conn)?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn open_in_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    configure(&conn)?;
    migrate(&conn)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "busy_timeout", 5000)?;
    Ok(())
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS scan_roots (
            id TEXT PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            created_at TEXT NOT NULL,
            last_scanned_at TEXT
        );

        CREATE TABLE IF NOT EXISTS projects (
            id TEXT PRIMARY KEY,
            canonical_path TEXT NOT NULL UNIQUE,
            display_name TEXT NOT NULL,
            detected_name TEXT,
            notes TEXT,
            vcs_kind TEXT NOT NULL,
            availability TEXT NOT NULL,
            archived INTEGER NOT NULL DEFAULT 0,
            favorite INTEGER NOT NULL DEFAULT 0,
            origin TEXT NOT NULL,
            scan_root_id TEXT,
            languages_json TEXT NOT NULL DEFAULT '[]',
            frameworks_json TEXT NOT NULL DEFAULT '[]',
            package_managers_json TEXT NOT NULL DEFAULT '[]',
            facts_json TEXT NOT NULL DEFAULT '[]',
            tasks_json TEXT NOT NULL DEFAULT '[]',
            dependencies_json TEXT,
            git_json TEXT,
            readme_path TEXT,
            readme_excerpt TEXT,
            search_blob TEXT NOT NULL DEFAULT '',
            source_mtime TEXT,
            last_commit_at TEXT,
            last_opened_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY(scan_root_id) REFERENCES scan_roots(id) ON DELETE SET NULL
        );

        CREATE TABLE IF NOT EXISTS project_tags (
            project_id TEXT NOT NULL,
            tag TEXT NOT NULL,
            PRIMARY KEY (project_id, tag),
            FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS project_icon_overrides (
            project_id TEXT PRIMARY KEY,
            mime_type TEXT NOT NULL,
            bytes BLOB NOT NULL,
            source_name TEXT,
            updated_at TEXT NOT NULL,
            FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS project_fts_unicode USING fts5(
            project_id UNINDEXED,
            content,
            tokenize = 'unicode61'
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS project_fts_trigram USING fts5(
            project_id UNINDEXED,
            content,
            tokenize = 'trigram'
        );

        CREATE TABLE IF NOT EXISTS task_runs (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            task_id TEXT,
            kind TEXT NOT NULL,
            executable TEXT NOT NULL,
            argv_json TEXT NOT NULL,
            cwd TEXT NOT NULL,
            shell_mode INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL,
            exit_code INTEGER,
            log_path TEXT NOT NULL,
            started_at TEXT NOT NULL,
            finished_at TEXT,
            FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
        );
        "#,
    )?;

    let applied: i64 = conn.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 1",
        [],
        |row| row.get(0),
    )?;
    if applied == 0 {
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (1, datetime('now'))",
            params![],
        )?;
    }
    let applied2: i64 = conn.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 2",
        [],
        |row| row.get(0),
    )?;
    if applied2 == 0 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS task_runs (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                task_id TEXT,
                kind TEXT NOT NULL,
                executable TEXT NOT NULL,
                argv_json TEXT NOT NULL,
                cwd TEXT NOT NULL,
                shell_mode INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL,
                exit_code INTEGER,
                log_path TEXT NOT NULL,
                started_at TEXT NOT NULL,
                finished_at TEXT,
                FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
            );",
        )?;
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (2, datetime('now'))",
            params![],
        )?;
    }
    let applied3: i64 = conn.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 3",
        [],
        |row| row.get(0),
    )?;
    if applied3 == 0 {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS provider_profiles (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                protocol TEXT NOT NULL,
                base_url TEXT,
                model TEXT NOT NULL,
                credential_ref TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS ai_memory (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                text TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS ai_summaries (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                provider_id TEXT,
                model TEXT,
                evidence_snapshot TEXT NOT NULL,
                text TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS project_conversations (
                project_id TEXT PRIMARY KEY,
                messages JSON NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
            );
            "#,
        )?;
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (3, datetime('now'))",
            params![],
        )?;
    }
    let applied4: i64 = conn.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 4",
        [],
        |row| row.get(0),
    )?;
    if applied4 == 0 {
        let has_description: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('projects') WHERE name = 'description'",
            [],
            |row| row.get(0),
        )?;
        if has_description == 0 {
            conn.execute("ALTER TABLE projects ADD COLUMN description TEXT", [])?;
        }
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (4, datetime('now'))",
            params![],
        )?;
    }
    let applied5: i64 = conn.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 5",
        [],
        |row| row.get(0),
    )?;
    if applied5 == 0 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS project_icon_overrides (
                project_id TEXT PRIMARY KEY,
                mime_type TEXT NOT NULL,
                bytes BLOB NOT NULL,
                source_name TEXT,
                updated_at TEXT NOT NULL,
                FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
            );",
        )?;
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (5, datetime('now'))",
            params![],
        )?;
    }
    let applied6: i64 = conn.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 6",
        [],
        |row| row.get(0),
    )?;
    if applied6 == 0 {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS project_events (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                title TEXT NOT NULL,
                detail TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_project_events_project_created
                ON project_events(project_id, datetime(created_at) DESC);

            CREATE TABLE IF NOT EXISTS pending_approvals (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                title TEXT NOT NULL,
                detail TEXT NOT NULL,
                executable TEXT,
                argv_json TEXT NOT NULL DEFAULT '[]',
                cwd TEXT,
                task_id TEXT,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                resolved_at TEXT,
                FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
            );
            "#,
        )?;
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (6, datetime('now'))",
            params![],
        )?;
    }
    let applied7: i64 = conn.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 7",
        [],
        |row| row.get(0),
    )?;
    if applied7 == 0 {
        // Keep this migration additive so databases created by an earlier
        // development build remain readable and upgrade without a rebuild.
        for (column, definition) in [
            ("origin", "TEXT NOT NULL DEFAULT 'desktop'"),
            ("run_id", "TEXT"),
            ("error", "TEXT"),
        ] {
            let exists: i64 = conn.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('pending_approvals') WHERE name = ?1",
                [column],
                |row| row.get(0),
            )?;
            if exists == 0 {
                conn.execute(
                    &format!("ALTER TABLE pending_approvals ADD COLUMN {column} {definition}"),
                    [],
                )?;
            }
        }
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS audit_events (
                id TEXT PRIMARY KEY,
                origin TEXT NOT NULL,
                action TEXT NOT NULL,
                target_type TEXT NOT NULL,
                target_id TEXT,
                detail TEXT,
                outcome TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_audit_events_created
                ON audit_events(datetime(created_at) DESC);

            CREATE TABLE IF NOT EXISTS managed_file_cleanup (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                path TEXT NOT NULL UNIQUE,
                kind TEXT NOT NULL,
                created_at TEXT NOT NULL,
                last_error TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_managed_file_cleanup_project
                ON managed_file_cleanup(project_id);
            "#,
        )?;
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (7, datetime('now'))",
            params![],
        )?;
    }
    let applied8: i64 = conn.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 8",
        [],
        |row| row.get(0),
    )?;
    if applied8 == 0 {
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('pending_approvals') WHERE name = 'shell_mode'",
            [],
            |row| row.get(0),
        )?;
        if exists == 0 {
            conn.execute(
                "ALTER TABLE pending_approvals ADD COLUMN shell_mode INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (8, datetime('now'))",
            params![],
        )?;
    }
    let applied9: i64 = conn.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 9",
        [],
        |row| row.get(0),
    )?;
    if applied9 == 0 {
        conn.execute_batch(
            r#"
            CREATE INDEX IF NOT EXISTS idx_audit_events_target
                ON audit_events(target_type, target_id);
            "#,
        )?;
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (9, datetime('now'))",
            params![],
        )?;
    }
    Ok(())
}
