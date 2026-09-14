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
    // Bound the connection page-cache budget to 64 MiB for large local history queries.
    conn.pragma_update(None, "cache_size", -65536)?;
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
            lineage_key TEXT,
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

        CREATE TABLE IF NOT EXISTS project_directory_groups (
            project_id TEXT PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS project_modules (
            canonical_path TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            snapshot_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS project_modules_owner ON project_modules(project_id);

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
    let applied10: i64 = conn.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 10",
        [],
        |row| row.get(0),
    )?;
    if applied10 == 0 {
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('provider_profiles') WHERE name = 'credential_blob'",
            [],
            |row| row.get(0),
        )?;
        if exists == 0 {
            conn.execute(
                "ALTER TABLE provider_profiles ADD COLUMN credential_blob BLOB",
                [],
            )?;
        }
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (10, datetime('now'))",
            params![],
        )?;
    }
    let applied11: i64 = conn.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 11",
        [],
        |row| row.get(0),
    )?;
    if applied11 == 0 {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS project_collections (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL COLLATE NOCASE UNIQUE,
                description TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS project_collection_members (
                collection_id TEXT NOT NULL,
                project_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                PRIMARY KEY (collection_id, project_id),
                FOREIGN KEY(collection_id) REFERENCES project_collections(id) ON DELETE CASCADE,
                FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_collection_members_project
                ON project_collection_members(project_id);

            CREATE TABLE IF NOT EXISTS attention_acknowledgements (
                item_id TEXT NOT NULL,
                source_version TEXT NOT NULL,
                acknowledged_at TEXT NOT NULL,
                PRIMARY KEY (item_id, source_version)
            );

            CREATE TABLE IF NOT EXISTS environment_observations (
                project_id TEXT PRIMARY KEY,
                inspection_json TEXT NOT NULL,
                observed_at TEXT NOT NULL,
                FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
            );
            "#,
        )?;
        for (table, column, definition) in [
            ("task_runs", "peak_cpu_percent", "REAL"),
            ("task_runs", "peak_memory_bytes", "INTEGER"),
            (
                "task_runs",
                "observed_ports_json",
                "TEXT NOT NULL DEFAULT '[]'",
            ),
            ("ai_summaries", "evidence_fingerprint", "TEXT"),
        ] {
            let exists: i64 = conn.query_row(
                &format!("SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name = ?1"),
                [column],
                |row| row.get(0),
            )?;
            if exists == 0 {
                conn.execute(
                    &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
                    [],
                )?;
            }
        }
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (11, datetime('now'))",
            params![],
        )?;
    }
    let applied12: i64 = conn.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 12",
        [],
        |row| row.get(0),
    )?;
    if applied12 == 0 {
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('environment_observations') WHERE name = 'condition_version'",
            [],
            |row| row.get(0),
        )?;
        if exists == 0 {
            conn.execute(
                "ALTER TABLE environment_observations ADD COLUMN condition_version TEXT NOT NULL DEFAULT ''",
                [],
            )?;
        }
        conn.execute(
            "UPDATE environment_observations SET condition_version = observed_at WHERE condition_version = ''",
            [],
        )?;
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (12, datetime('now'))",
            [],
        )?;
    }
    let applied13: i64 = conn.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 13",
        [],
        |row| row.get(0),
    )?;
    if applied13 == 0 {
        for (column, definition) in [
            ("expected_ports_json", "TEXT NOT NULL DEFAULT '[]'"),
            ("dev_url_path", "TEXT"),
            ("dev_url_scheme", "TEXT"),
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
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (13, datetime('now'))",
            [],
        )?;
    }
    let applied14: i64 = conn.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 14",
        [],
        |row| row.get(0),
    )?;
    if applied14 == 0 {
        conn.execute_batch(
            r#"
            CREATE INDEX IF NOT EXISTS idx_task_runs_project_started
                ON task_runs(project_id, started_at DESC);
            CREATE INDEX IF NOT EXISTS idx_task_runs_status_started
                ON task_runs(status, started_at DESC);
            CREATE INDEX IF NOT EXISTS idx_projects_availability_updated
                ON projects(availability, updated_at DESC);
            "#,
        )?;
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (14, datetime('now'))",
            [],
        )?;
    }
    let applied15: i64 = conn.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 15",
        [],
        |row| row.get(0),
    )?;
    if applied15 == 0 {
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('projects') WHERE name = 'lineage_key'",
            [],
            |row| row.get(0),
        )?;
        if exists == 0 {
            conn.execute("ALTER TABLE projects ADD COLUMN lineage_key TEXT", [])?;
        }
        let rows = {
            let mut stmt = conn.prepare("SELECT id, facts_json FROM projects")?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };
        for (id, facts_json) in rows {
            let lineage_key = serde_json::from_str::<Vec<crate::models::DetectedFact>>(&facts_json)
                .ok()
                .and_then(|facts| {
                    facts
                        .into_iter()
                        .find(|fact| fact.kind == "lineage")
                        .and_then(|fact| crate::git::normalize_remote_url(&fact.value))
                });
            if let Some(lineage_key) = lineage_key {
                conn.execute(
                    "UPDATE projects SET lineage_key = ?1 WHERE id = ?2",
                    params![lineage_key, id],
                )?;
            }
        }
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_projects_lineage_key ON projects(lineage_key);",
        )?;
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (15, datetime('now'))",
            [],
        )?;
    }
    let applied16: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = 16)",
        [],
        |row| row.get(0),
    )?;
    if !applied16 {
        let transaction = conn.unchecked_transaction()?;
        sanitize_project_remotes(&transaction)?;
        transaction.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_task_runs_project_latest
             ON task_runs(project_id, datetime(started_at) DESC, id DESC);
             INSERT INTO schema_migrations (version, applied_at) VALUES (16, datetime('now'));",
        )?;
        transaction.commit()?;
    }
    let applied17: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = 17)",
        [],
        |row| row.get(0),
    )?;
    if !applied17 {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS project_git_commits (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                sha TEXT NOT NULL,
                short_sha TEXT NOT NULL,
                subject TEXT NOT NULL,
                author_name TEXT NOT NULL,
                author_date TEXT NOT NULL,
                commit_date TEXT NOT NULL,
                message TEXT NOT NULL,
                cached_at TEXT NOT NULL,
                FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
                UNIQUE(project_id, sha)
            );
            CREATE INDEX IF NOT EXISTS idx_project_git_commits_project_date
                ON project_git_commits(project_id, datetime(commit_date) DESC);
            CREATE INDEX IF NOT EXISTS idx_project_git_commits_date
                ON project_git_commits(datetime(commit_date) DESC);
            INSERT INTO schema_migrations (version, applied_at) VALUES (17, datetime('now'));
            "#,
        )?;
    }
    conn.execute_batch(r#"
        CREATE INDEX IF NOT EXISTS idx_activity_git_time ON project_git_commits(unixepoch(commit_date) DESC, project_id, sha);
        CREATE INDEX IF NOT EXISTS idx_activity_git_project ON project_git_commits(project_id, unixepoch(commit_date) DESC, sha);
        CREATE INDEX IF NOT EXISTS idx_activity_event_time ON project_events(unixepoch(created_at) DESC, id);
        CREATE INDEX IF NOT EXISTS idx_activity_event_project ON project_events(project_id, unixepoch(created_at) DESC, id);
        CREATE INDEX IF NOT EXISTS idx_activity_run_time ON task_runs(unixepoch(started_at) DESC, id);
        CREATE INDEX IF NOT EXISTS idx_activity_run_project ON task_runs(project_id, unixepoch(started_at) DESC, id);
        CREATE TABLE IF NOT EXISTS git_history_coverage (
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            start_at INTEGER NOT NULL, end_at INTEGER NOT NULL,
            checked_at INTEGER NOT NULL, head TEXT, error TEXT,
            PRIMARY KEY(project_id, start_at, end_at)
        );
    "#)?;
    migrate_activity_counts(conn)?;
    crate::core::sessions::migrate(conn)?;
    let indexed: i64 = conn.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version=21",
        [],
        |r| r.get(0),
    )?;
    if indexed == 0 {
        let tx = conn.unchecked_transaction()?;
        for (source, table, time, id) in [
            (
                "git",
                "project_git_commits",
                "commit_date",
                "'git:'||project_id||':'||sha",
            ),
            ("run", "task_runs", "started_at", "'run:'||id"),
            ("event", "project_events", "created_at", "'event:'||id"),
        ] {
            tx.execute_batch(&format!("DROP INDEX IF EXISTS idx_activity_{source}_time; DROP INDEX IF EXISTS idx_activity_{source}_project; CREATE INDEX idx_activity_{source}_time ON {table}(unixepoch({time}) DESC,({id}) DESC); CREATE INDEX idx_activity_{source}_project ON {table}(project_id,unixepoch({time}) DESC,({id}) DESC);"))?;
        }
        tx.execute(
            "INSERT INTO schema_migrations VALUES(21,datetime('now'))",
            [],
        )?;
        tx.commit()?;
    }
    Ok(())
}

// Counts are derived, local-only data. Triggers also cover writes from a second MCP connection.
fn migrate_activity_counts(conn: &Connection) -> Result<()> {
    for (version, label, seconds) in [
        (18, "hour", 3600),
        (19, "sixhour", 21600),
        (20, "day", 86400),
    ] {
        let applied: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version=?1)",
            [version],
            |r| r.get(0),
        )?;
        if applied {
            conn.execute_batch(&format!("CREATE INDEX IF NOT EXISTS activity_{label}_counts_project ON activity_{label}_counts(project_id,bucket_at,category,n);"))?;
            continue;
        }
        let tx = conn.unchecked_transaction()?;
        let counts = format!("activity_{label}_counts");
        tx.execute_batch(&format!("CREATE TABLE {counts}(bucket_at INTEGER NOT NULL,project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,category TEXT NOT NULL,n INTEGER NOT NULL,PRIMARY KEY(bucket_at,project_id,category)) WITHOUT ROWID; CREATE INDEX {counts}_project ON {counts}(project_id,bucket_at,category,n);"))?;
        for (table,time,category) in [
            ("project_git_commits","commit_date","'git'"),
            ("task_runs","started_at","'task'"),
            ("project_events","created_at","CASE kind WHEN 'git' THEN 'git' WHEN 'task' THEN 'task' WHEN 'ide' THEN 'tool' WHEN 'agent' THEN 'tool' WHEN 'terminal' THEN 'tool' WHEN 'explorer' THEN 'tool' WHEN 'open' THEN 'open' ELSE 'maintenance' END"),
        ] {
            let bucket=format!("(unixepoch({time})-((unixepoch({time})%{seconds}+{seconds})%{seconds}))");
            tx.execute_batch(&format!("INSERT INTO {counts} SELECT {bucket},project_id,{category},COUNT(*) FROM {table} WHERE unixepoch({time}) IS NOT NULL GROUP BY 1,2,3 ON CONFLICT(bucket_at,project_id,category) DO UPDATE SET n=n+excluded.n;"))?;
            let new_bucket=bucket.replace(time,&format!("new.{time}"));
            let old_bucket=bucket.replace(time,&format!("old.{time}"));
            let new_category=category.replace("kind","new.kind");
            let old_category=category.replace("kind","old.kind");
            let add=format!("INSERT INTO {counts} SELECT {new_bucket},new.project_id,{new_category},1 WHERE unixepoch(new.{time}) IS NOT NULL ON CONFLICT(bucket_at,project_id,category) DO UPDATE SET n=n+1;");
            let remove=format!("UPDATE {counts} SET n=n-1 WHERE bucket_at={old_bucket} AND project_id=old.project_id AND category={old_category}; DELETE FROM {counts} WHERE bucket_at={old_bucket} AND project_id=old.project_id AND category={old_category} AND n<=0;");
            tx.execute_batch(&format!("CREATE TRIGGER activity_{label}_{table}_insert AFTER INSERT ON {table} BEGIN {add} END; CREATE TRIGGER activity_{label}_{table}_delete AFTER DELETE ON {table} BEGIN {remove} END; CREATE TRIGGER activity_{label}_{table}_update AFTER UPDATE OF {time},project_id{} ON {table} BEGIN {remove} {add} END;",if table=="project_events"{",kind"}else{""}))?;
        }
        tx.execute(
            "INSERT INTO schema_migrations VALUES(?1,datetime('now'))",
            [version],
        )?;
        tx.commit()?;
    }
    Ok(())
}

/// Also used on backup copies, which must not expose legacy credentials even
/// if an older application instance wrote a record after this migration.
pub(crate) fn sanitize_project_remotes(conn: &Connection) -> Result<()> {
    let rows = {
        let mut stmt = conn.prepare("SELECT id, facts_json, lineage_key FROM projects")?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    };
    for (id, original, old_key) in rows {
        let mut facts: Vec<crate::models::DetectedFact> = serde_json::from_str(&original)
            .map_err(|error| crate::error::Error::msg(error.to_string()))?;
        for fact in &mut facts {
            if fact.kind == "lineage" {
                fact.value = crate::git::sanitize_remote_url(&fact.value);
            }
        }
        let key = facts
            .iter()
            .find(|fact| fact.kind == "lineage")
            .and_then(|fact| crate::git::normalize_remote_url(&fact.value));
        let sanitized = serde_json::to_string(&facts)
            .map_err(|error| crate::error::Error::msg(error.to_string()))?;
        if sanitized != original || key != old_key {
            conn.execute(
                "UPDATE projects SET facts_json = ?1, lineage_key = ?2 WHERE id = ?3",
                params![sanitized, key, id],
            )?;
        }
    }
    Ok(())
}
