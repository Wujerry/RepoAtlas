//! Development-only fixture seeder for local product screenshots.
//!
//! This binary is intentionally not wired into the desktop application or the
//! MCP adapter.  The PowerShell entry point performs the destructive safety
//! checks; this binary adds a second boundary by accepting only the desktop
//! application database and the fixed, marked showcase directory.

use chrono::{Duration, SecondsFormat, Utc};
use repoatlas_core::{db, detect, git, models::TaskDefinition, paths};
use rusqlite::{params, Connection};
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

const SHOWCASE_PATH: &str = r"C:\RepoAtlas Showcase";
const SHOWCASE_MARKER: &str = "RepoAtlas demo showcase marker v1";
const PROJECT_SLUGS: [&str; 8] = [
    "atlas-dashboard",
    "signal-console",
    "insight-notebooks",
    "harbor-api",
    "care-portal",
    "field-kit",
    "ops-playbook",
    "pulse-mobile",
];

#[derive(Clone, Copy)]
struct ProjectSpec {
    id: &'static str,
    slug: &'static str,
    display_name: &'static str,
    description: &'static str,
    notes: &'static str,
    favorite: bool,
    archived: bool,
    tags: &'static [&'static str],
}

const PROJECTS: [ProjectSpec; 8] = [
    ProjectSpec {
        id: "11111111-1111-4111-8111-111111111111",
        slug: "atlas-dashboard",
        display_name: "Atlas Dashboard",
        description: "A calm operations dashboard for exploring local code assets and their next useful action.",
        notes: "Primary showcase project for the Library and Overview workflows.",
        favorite: true,
        archived: false,
        tags: &["showcase", "web", "featured"],
    },
    ProjectSpec {
        id: "22222222-2222-4222-8222-222222222222",
        slug: "signal-console",
        display_name: "Signal Console",
        description: "A small Rust and Tauri desktop shell that keeps operator context close to the work.",
        notes: "Clean desktop checkout with a short release loop.",
        favorite: true,
        archived: false,
        tags: &["desktop", "rust", "tauri"],
    },
    ProjectSpec {
        id: "33333333-3333-4333-8333-333333333333",
        slug: "insight-notebooks",
        display_name: "Insight Notebooks",
        description: "A FastAPI notebook service for turning local observations into small, reviewable experiments.",
        notes: "Python runtime is intentionally marked as needing attention in the showcase data.",
        favorite: false,
        archived: false,
        tags: &["python", "data", "api"],
    },
    ProjectSpec {
        id: "44444444-4444-4444-8444-444444444444",
        slug: "harbor-api",
        display_name: "Harbor API",
        description: "A focused Go service with explicit health checks and a dependable test command.",
        notes: "A clean backend project for the stack and task views.",
        favorite: false,
        archived: false,
        tags: &["go", "backend", "api"],
    },
    ProjectSpec {
        id: "55555555-5555-4555-8555-555555555555",
        slug: "care-portal",
        display_name: "Care Portal",
        description: "A Spring Boot service prototype for coordinating care-team workflows.",
        notes: "Java project included to make cross-stack discovery visible at a glance.",
        favorite: false,
        archived: false,
        tags: &["java", "spring", "service"],
    },
    ProjectSpec {
        id: "66666666-6666-4666-8666-666666666666",
        slug: "field-kit",
        display_name: "Field Kit",
        description: "A Flutter companion app for field notes, checklists, and offline-first handoffs.",
        notes: "Mobile stack fixture with a lightweight build surface.",
        favorite: false,
        archived: false,
        tags: &["mobile", "dart", "flutter"],
    },
    ProjectSpec {
        id: "77777777-7777-4777-8777-777777777777",
        slug: "ops-playbook",
        display_name: "Ops Playbook",
        description: "A versioned set of operational notes with a tiny preview task and no remote repository.",
        notes: "Archived documentation project; useful for demonstrating scope filters.",
        favorite: false,
        archived: true,
        tags: &["docs", "archived", "operations"],
    },
    ProjectSpec {
        id: "88888888-8888-4888-8888-888888888888",
        slug: "pulse-mobile",
        display_name: "Pulse Mobile",
        description: "A React Native-style product surface with an intentionally staged and unstaged change.",
        notes: "Dirty checkout used by the Git workspace screenshot.",
        favorite: true,
        archived: false,
        tags: &["mobile", "typescript", "git"],
    },
];

fn main() {
    if let Err(error) = run() {
        eprintln!("repoatlas-demo: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let (db_path, showcase_path) = parse_args()?;
    validate_db_path(&db_path)?;
    validate_showcase_path(&showcase_path)?;

    let mut conn = db::open(&db_path)?;
    validate_existing_data(&conn, &showcase_path)?;
    let log_dir = db_path
        .parent()
        .ok_or("database path has no parent")?
        .join("task-logs");
    fs::create_dir_all(&log_dir)?;

    let now = Utc::now();
    let tx = conn.transaction()?;
    clear_demo_records(&tx)?;
    put_settings(&tx)?;

    let showcase = paths::canonicalize(&showcase_path)?;
    let showcase_string = paths::path_to_string(&showcase);
    tx.execute(
        "INSERT INTO scan_roots (id, path, created_at, last_scanned_at) VALUES (?1, ?2, ?3, ?4)",
        params![
            "demo-scan-root",
            showcase_string,
            stamp(now - Duration::days(2)),
            stamp(now)
        ],
    )?;

    for (index, spec) in PROJECTS.iter().enumerate() {
        let project_path = showcase.join(spec.slug);
        let project_path = paths::canonicalize(&project_path)?;
        let detection = detect::detect(&project_path);
        let git_snapshot = git::snapshot(&project_path);
        let tasks = demo_tasks(spec.slug);
        let facts_json = serde_json::to_string(&detection.facts)?;
        let tasks_json = serde_json::to_string(&tasks)?;
        let dependencies_json = detection
            .dependencies
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let git_json = git_snapshot
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let languages_json = serde_json::to_string(&detection.languages)?;
        let frameworks_json = serde_json::to_string(&detection.frameworks)?;
        let package_managers_json = serde_json::to_string(&detection.package_managers)?;
        let path_string = paths::path_to_string(&project_path);
        let search_blob = format!(
            "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
            spec.display_name,
            path_string,
            detection.languages.join(" "),
            detection.frameworks.join(" "),
            detection.package_managers.join(" "),
            detection.vcs_kind,
            spec.description,
            detection.readme_excerpt.clone().unwrap_or_default(),
        );
        let created_at = stamp(now - Duration::days(2) + Duration::minutes(index as i64 * 9));
        let updated_at = stamp(now - Duration::minutes((index * 7) as i64));
        let last_opened_at = if spec.archived {
            None
        } else {
            Some(stamp(now - Duration::hours((index + 1) as i64)))
        };
        tx.execute(
            r#"
            INSERT INTO projects (
                id, canonical_path, display_name, detected_name, notes, description,
                vcs_kind, availability, archived, favorite, origin, scan_root_id,
                languages_json, frameworks_json, package_managers_json, facts_json,
                tasks_json, dependencies_json, git_json, readme_path, readme_excerpt,
                search_blob, source_mtime, last_commit_at, last_opened_at, created_at, updated_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, 'ready', ?8, ?9, 'demo', 'demo-scan-root',
                ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24
            )
            "#,
            params![
                spec.id,
                path_string,
                spec.display_name,
                detection.name,
                spec.notes,
                spec.description,
                detection.vcs_kind,
                spec.archived as i64,
                spec.favorite as i64,
                languages_json,
                frameworks_json,
                package_managers_json,
                facts_json,
                tasks_json,
                dependencies_json,
                git_json,
                detection.readme_path,
                detection.readme_excerpt,
                search_blob,
                detection.source_mtime,
                git_snapshot.and_then(|snapshot| snapshot.last_commit_at),
                last_opened_at,
                created_at,
                updated_at,
            ],
        )?;

        for tag in spec.tags {
            tx.execute(
                "INSERT INTO project_tags (project_id, tag) VALUES (?1, ?2)",
                params![spec.id, tag],
            )?;
        }
        tx.execute(
            "INSERT INTO project_fts_unicode (project_id, content) VALUES (?1, ?2)",
            params![spec.id, search_blob],
        )?;
        tx.execute(
            "INSERT INTO project_fts_trigram (project_id, content) VALUES (?1, ?2)",
            params![spec.id, search_blob],
        )?;
    }

    seed_project_events(&tx, now)?;
    seed_ai_knowledge(&tx, now)?;
    seed_approvals(&tx, &showcase, now)?;
    seed_task_runs(&tx, &log_dir, &showcase, now)?;
    seed_audit_events(&tx, now)?;
    tx.commit()?;

    println!(
        "RepoAtlas demo ready: {} projects at {}",
        PROJECTS.len(),
        paths::path_to_string(&showcase),
    );
    println!("Database: {}", paths::path_to_string(&db_path));
    println!("Task runs: 5 | events: 8 | pending approvals: 2 | AI summaries: 2 | AI memory: 3");
    Ok(())
}

fn parse_args() -> Result<(PathBuf, PathBuf), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let mut db_path = None;
    let mut showcase_path = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--db" => db_path = Some(PathBuf::from(args.next().ok_or("--db needs a path")?)),
            "--showcase" => {
                showcase_path = Some(PathBuf::from(args.next().ok_or("--showcase needs a path")?))
            }
            "--help" | "-h" => {
                println!("Usage: repoatlas-demo --db <app-data-db> --showcase <showcase-root>");
                std::process::exit(0);
            }
            unknown => return Err(format!("unknown argument: {unknown}").into()),
        }
    }
    Ok((
        db_path.ok_or("--db is required")?,
        showcase_path.ok_or("--showcase is required")?,
    ))
}

fn validate_db_path(path: &Path) -> Result<(), Box<dyn Error>> {
    if !cfg!(windows) {
        return Err("the local demo seeder is Windows-only".into());
    }
    let app_data = env::var_os("APPDATA").ok_or("APPDATA is not available")?;
    let expected = PathBuf::from(app_data)
        .join("io.repoatlas.desktop")
        .join("repoatlas.sqlite");
    let parent = path.parent().ok_or("database path has no parent")?;
    if !same_path(parent, expected.parent().expect("expected parent"))
        || path.file_name().and_then(|name| name.to_str()) != Some("repoatlas.sqlite")
    {
        return Err(format!("refusing database outside {}", expected.display()).into());
    }
    Ok(())
}

fn validate_showcase_path(path: &Path) -> Result<(), Box<dyn Error>> {
    if !same_path(path, Path::new(SHOWCASE_PATH)) {
        return Err(format!("refusing showcase path outside {SHOWCASE_PATH}").into());
    }
    if !path.is_dir() {
        return Err(format!("showcase root does not exist: {}", path.display()).into());
    }
    let marker = path.join(".repoatlas-demo-marker");
    let marker_text = fs::read_to_string(marker)?;
    if !marker_text
        .lines()
        .any(|line| line.trim() == SHOWCASE_MARKER)
    {
        return Err("showcase marker is missing or does not identify RepoAtlas demo data".into());
    }
    for slug in PROJECT_SLUGS {
        if !path.join(slug).is_dir() {
            return Err(format!("missing showcase fixture: {slug}").into());
        }
    }
    Ok(())
}

fn validate_existing_data(conn: &Connection, showcase: &Path) -> Result<(), Box<dyn Error>> {
    let mut stmt = conn.prepare("SELECT canonical_path FROM projects")?;
    let paths = stmt.query_map([], |row| row.get::<_, String>(0))?;
    for value in paths {
        let value = value?;
        if !paths::is_within(Path::new(&value), showcase) || same_path(Path::new(&value), showcase)
        {
            return Err(format!("refusing to replace project outside showcase: {value}").into());
        }
    }

    let mut roots = conn.prepare("SELECT path FROM scan_roots")?;
    let values = roots.query_map([], |row| row.get::<_, String>(0))?;
    for value in values {
        let value = value?;
        if !same_path(Path::new(&value), showcase) {
            return Err(format!("refusing to replace scan root outside showcase: {value}").into());
        }
    }

    let provider_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM provider_profiles", [], |row| {
            row.get(0)
        })?;
    if provider_count != 0 {
        return Err("refusing to replace a database containing provider profiles".into());
    }
    Ok(())
}

fn clear_demo_records(conn: &Connection) -> Result<(), Box<dyn Error>> {
    // validate_existing_data has already established that every managed
    // project/root belongs to this showcase.  Clearing these tables therefore
    // makes re-seeding deterministic without introducing a production reset
    // command into Core or MCP.
    for table in [
        "project_fts_unicode",
        "project_fts_trigram",
        "project_tags",
        "task_runs",
        "project_events",
        "pending_approvals",
        "ai_memory",
        "ai_summaries",
        "project_conversations",
        "audit_events",
        "managed_file_cleanup",
        "projects",
        "scan_roots",
    ] {
        conn.execute(&format!("DELETE FROM {table}"), [])?;
    }
    conn.execute("DELETE FROM settings", [])?;
    Ok(())
}

fn put_settings(conn: &Connection) -> Result<(), Box<dyn Error>> {
    for (key, value) in [
        ("theme", "dark"),
        ("locale", "en"),
        ("uiFont", ""),
        ("consoleFont", ""),
    ] {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
    }
    Ok(())
}

fn demo_tasks(slug: &str) -> Vec<TaskDefinition> {
    let task =
        |id: &str, kind: &str, name: &str, description: &str, executable: &str, argv: &[&str]| {
            TaskDefinition {
                id: id.to_string(),
                kind: kind.to_string(),
                name: name.to_string(),
                description: Some(description.to_string()),
                executable: executable.to_string(),
                argv: argv.iter().map(|value| (*value).to_string()).collect(),
                cwd: None,
                inferred: false,
                shell_mode: false,
            }
        };
    match slug {
        "atlas-dashboard" => vec![
            task(
                "dev",
                "dev",
                "Start preview",
                "Open the local dashboard preview.",
                "pnpm",
                &["dev"],
            ),
            task(
                "test",
                "test",
                "Run checks",
                "Run the focused frontend test suite.",
                "pnpm",
                &["test", "--", "--run"],
            ),
            task(
                "build",
                "build",
                "Production build",
                "Compile the dashboard for distribution.",
                "pnpm",
                &["build"],
            ),
        ],
        "signal-console" => vec![
            task(
                "dev",
                "dev",
                "Open desktop shell",
                "Launch the Tauri development window.",
                "cargo",
                &["tauri", "dev"],
            ),
            task(
                "test",
                "test",
                "Rust tests",
                "Run the Rust unit and integration tests.",
                "cargo",
                &["test"],
            ),
            task(
                "build",
                "build",
                "Release bundle",
                "Build the desktop release bundle.",
                "cargo",
                &["tauri", "build"],
            ),
            task(
                "check",
                "check",
                "Clippy",
                "Run static checks across the Rust targets.",
                "cargo",
                &["clippy", "--all-targets"],
            ),
        ],
        "insight-notebooks" => vec![
            task(
                "dev",
                "dev",
                "Notebook server",
                "Start the local notebook API.",
                "uv",
                &["run", "uvicorn", "main:app", "--reload"],
            ),
            task(
                "test",
                "test",
                "Pytest",
                "Run the Python test suite.",
                "uv",
                &["run", "pytest"],
            ),
            task(
                "build",
                "build",
                "Compile modules",
                "Check Python modules for syntax errors.",
                "uv",
                &["run", "python", "-m", "compileall", "src"],
            ),
        ],
        "harbor-api" => vec![
            task(
                "dev",
                "dev",
                "Run API",
                "Start the local Go API server.",
                "go",
                &["run", "."],
            ),
            task(
                "test",
                "test",
                "Go tests",
                "Run all Go package tests.",
                "go",
                &["test", "./..."],
            ),
            task(
                "build",
                "build",
                "Build service",
                "Compile the Go service.",
                "go",
                &["build", "./..."],
            ),
        ],
        "care-portal" => vec![
            task(
                "dev",
                "dev",
                "Spring Boot",
                "Start the Spring Boot development server.",
                "mvn",
                &["spring-boot:run"],
            ),
            task(
                "test",
                "test",
                "Maven tests",
                "Run the Java test suite.",
                "mvn",
                &["test"],
            ),
            task(
                "build",
                "build",
                "Package service",
                "Package the Spring Boot service.",
                "mvn",
                &["-DskipTests", "package"],
            ),
        ],
        "field-kit" => vec![
            task(
                "dev",
                "dev",
                "Flutter run",
                "Open the mobile development target.",
                "flutter",
                &["run"],
            ),
            task(
                "test",
                "test",
                "Flutter tests",
                "Run widget and unit tests.",
                "flutter",
                &["test"],
            ),
            task(
                "build",
                "build",
                "Android build",
                "Build the mobile APK.",
                "flutter",
                &["build", "apk"],
            ),
        ],
        "ops-playbook" => vec![
            task(
                "preview",
                "dev",
                "Preview playbook",
                "Serve the operational notes locally.",
                "python",
                &["-m", "http.server", "4173"],
            ),
            task(
                "check",
                "check",
                "Check links",
                "Validate the playbook's local links.",
                "make",
                &["check"],
            ),
        ],
        "pulse-mobile" => vec![
            task(
                "dev",
                "dev",
                "Start mobile preview",
                "Open the mobile product preview.",
                "pnpm",
                &["dev"],
            ),
            task(
                "test",
                "test",
                "Run mobile tests",
                "Run the mobile component tests.",
                "pnpm",
                &["test", "--", "--run"],
            ),
            task(
                "build",
                "build",
                "Build release",
                "Compile the mobile release surface.",
                "pnpm",
                &["build"],
            ),
        ],
        _ => Vec::new(),
    }
}

fn seed_project_events(
    conn: &Connection,
    now: chrono::DateTime<Utc>,
) -> Result<(), Box<dyn Error>> {
    let events = [
        (
            "11111111-1111-4111-8111-111111111111",
            "scan",
            "Project discovered",
            "Detected React, Vite, and pnpm evidence.",
        ),
        (
            "11111111-1111-4111-8111-111111111111",
            "task",
            "Build completed",
            "Production build finished successfully.",
        ),
        (
            "22222222-2222-4222-8222-222222222222",
            "git",
            "Clean checkout observed",
            "main is ready for the next desktop change.",
        ),
        (
            "33333333-3333-4333-8333-333333333333",
            "environment",
            "Runtime attention",
            "Python requirement is not matched by this machine.",
        ),
        (
            "44444444-4444-4444-8444-444444444444",
            "scan",
            "Dependency snapshot refreshed",
            "Go module evidence is available offline.",
        ),
        (
            "66666666-6666-4666-8666-666666666666",
            "favorite",
            "Added to favorites",
            "Keep the mobile workflow close at hand.",
        ),
        (
            "77777777-7777-4777-8777-777777777777",
            "archive",
            "Project archived",
            "Documentation remains searchable without cluttering the active view.",
        ),
        (
            "88888888-8888-4888-8888-888888888888",
            "git",
            "Working tree changed",
            "One staged and one unstaged change are ready to inspect.",
        ),
    ];
    for (index, (project_id, kind, title, detail)) in events.into_iter().enumerate() {
        conn.execute(
            "INSERT INTO project_events (id, project_id, kind, title, detail, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                format!("demo-event-{index}"),
                project_id,
                kind,
                title,
                detail,
                stamp(now - Duration::minutes((index * 11 + 3) as i64)),
            ],
        )?;
    }
    Ok(())
}

fn seed_ai_knowledge(conn: &Connection, now: chrono::DateTime<Utc>) -> Result<(), Box<dyn Error>> {
    let summaries = [
        (
            "demo-summary-atlas",
            "11111111-1111-4111-8111-111111111111",
            "The dashboard is a small React/Vite surface organized around a fast discovery loop. Start with the Library, inspect the README and stack facts, then run the focused test or build task. The project is a good candidate for a reusable product shell because its task definitions are explicit and its evidence remains local.",
            r#"{"files":["package.json","README.md","src/App.tsx"],"characterCount":482,"source":"demo snapshot"}"#,
        ),
        (
            "demo-summary-pulse",
            "88888888-8888-4888-8888-888888888888",
            "Pulse Mobile is a compact TypeScript product surface with a healthy task set and a deliberately visible Git change. Review the staged release note separately from the unstaged status update before committing, then use the build task as the final handoff check.",
            r#"{"files":["package.json","README.md","src/status.ts"],"characterCount":391,"source":"demo snapshot"}"#,
        ),
    ];
    for (index, (id, project_id, text, evidence)) in summaries.into_iter().enumerate() {
        conn.execute(
            "INSERT INTO ai_summaries (id, project_id, provider_id, model, evidence_snapshot, text, created_at) VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6)",
            params![id, project_id, "local-demo-snapshot", evidence, text, stamp(now - Duration::hours((index + 1) as i64))],
        )?;
    }

    let memories = [
        ("demo-memory-atlas", "11111111-1111-4111-8111-111111111111", "Keep the Library entry point lightweight: discovery should remain useful even when network-dependent AI features are unavailable."),
        ("demo-memory-signal", "22222222-2222-4222-8222-222222222222", "Release builds require the desktop shell and the shared Rust core to stay on the same version."),
        ("demo-memory-pulse", "88888888-8888-4888-8888-888888888888", "Review staged and unstaged changes independently before committing a mobile release."),
    ];
    for (index, (id, project_id, text)) in memories.into_iter().enumerate() {
        conn.execute(
            "INSERT INTO ai_memory (id, project_id, text, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![
                id,
                project_id,
                text,
                stamp(now - Duration::days((index + 1) as i64))
            ],
        )?;
    }
    conn.execute(
        "INSERT INTO project_conversations (project_id, messages, updated_at) VALUES (?1, ?2, ?3)",
        params![
            "11111111-1111-4111-8111-111111111111",
            r#"[{"role":"user","content":"What should I inspect first?"},{"role":"assistant","content":"Start with the README, then run the focused test task. The local evidence points to a small, healthy React/Vite surface."}]"#,
            stamp(now - Duration::minutes(37)),
        ],
    )?;
    Ok(())
}

fn seed_approvals(
    conn: &Connection,
    showcase: &Path,
    now: chrono::DateTime<Utc>,
) -> Result<(), Box<dyn Error>> {
    let atlas = paths::path_to_string(&showcase.join("atlas-dashboard"));
    let signal = paths::path_to_string(&showcase.join("signal-console"));
    let approvals = [
        (
            "demo-approval-atlas",
            "11111111-1111-4111-8111-111111111111",
            "Run production build",
            "An external client requested the saved build task. Review the command before allowing the desktop broker to start it.",
            "pnpm",
            r#"["build"]"#,
            atlas,
            "build",
        ),
        (
            "demo-approval-signal",
            "22222222-2222-4222-8222-222222222222",
            "Open desktop shell",
            "The Tauri development task is waiting for explicit desktop approval.",
            "cargo",
            r#"["tauri","dev"]"#,
            signal,
            "dev",
        ),
    ];
    for (index, (id, project_id, title, detail, executable, argv, cwd, task_id)) in
        approvals.into_iter().enumerate()
    {
        conn.execute(
            r#"INSERT INTO pending_approvals
               (id, project_id, kind, title, detail, executable, argv_json, cwd, task_id, shell_mode, status, origin, run_id, error, created_at, resolved_at)
               VALUES (?1, ?2, 'task', ?3, ?4, ?5, ?6, ?7, ?8, 0, 'pending', 'mcp', NULL, NULL, ?9, NULL)"#,
            params![id, project_id, title, detail, executable, argv, cwd, task_id, stamp(now - Duration::minutes((index * 8 + 2) as i64))],
        )?;
    }
    Ok(())
}

fn seed_task_runs(
    conn: &Connection,
    log_dir: &Path,
    showcase: &Path,
    now: chrono::DateTime<Utc>,
) -> Result<(), Box<dyn Error>> {
    let atlas = paths::path_to_string(&showcase.join("atlas-dashboard"));
    let signal = paths::path_to_string(&showcase.join("signal-console"));
    let pulse = paths::path_to_string(&showcase.join("pulse-mobile"));
    let runs = [
        ("demo-run-atlas-build", "11111111-1111-4111-8111-111111111111", "build", "pnpm", r#"["build"]"#, atlas.clone(), "succeeded", Some(0_i64), "Build complete\n  dist/index.html  42.8 kB\n  ✓ 128 modules transformed\n"),
        ("demo-run-atlas-test", "11111111-1111-4111-8111-111111111111", "test", "pnpm", r#"["test","--","--run"]"#, atlas, "succeeded", Some(0_i64), " RUN  v4.1.11\n ✓ src/lib/search.test.ts (8 tests)\n Test Files  1 passed\n"),
        ("demo-run-signal-check", "22222222-2222-4222-8222-222222222222", "check", "cargo", r#"["clippy","--all-targets"]"#, signal, "failed", Some(1_i64), "Checking signal-console v0.1.0\nwarning: unused import\nerror: clippy found 1 warning as error\n"),
        ("demo-run-pulse-test", "88888888-8888-4888-8888-888888888888", "test", "pnpm", r#"["test","--","--run"]"#, pulse.clone(), "succeeded", Some(0_i64), " RUN  v4.1.11\n ✓ src/status.test.ts (12 tests)\n Test Files  1 passed\n"),
        ("demo-run-pulse-build", "88888888-8888-4888-8888-888888888888", "build", "pnpm", r#"["build"]"#, pulse, "succeeded", Some(0_i64), "vite v7.0.4 building for production...\n✓ built in 1.24s\n"),
    ];
    for (index, (id, project_id, task_id, executable, argv, cwd, status, exit_code, log)) in
        runs.into_iter().enumerate()
    {
        let log_path = log_dir.join(format!("{id}.log"));
        fs::write(&log_path, log)?;
        conn.execute(
            r#"INSERT INTO task_runs
               (id, project_id, task_id, kind, executable, argv_json, cwd, shell_mode, status, exit_code, log_path, started_at, finished_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?9, ?10, ?11, ?12)"#,
            params![
                id,
                project_id,
                task_id,
                if task_id == "test" { "test" } else if task_id == "build" { "build" } else { "check" },
                executable,
                argv,
                cwd,
                status,
                exit_code,
                paths::path_to_string(&log_path),
                stamp(now - Duration::hours((index + 1) as i64)),
                stamp(now - Duration::hours(index as i64)),
            ],
        )?;
    }
    Ok(())
}

fn seed_audit_events(conn: &Connection, now: chrono::DateTime<Utc>) -> Result<(), Box<dyn Error>> {
    let events = [
        (
            "demo-audit-seed",
            "demo_seed",
            "database",
            "success",
            "Created local screenshot fixtures.",
        ),
        (
            "demo-audit-scan",
            "scan_root",
            "scan_root",
            "success",
            "Manual showcase scan completed.",
        ),
        (
            "demo-audit-approval",
            "request_task_approval",
            "task_approval",
            "pending",
            "MCP request is waiting for desktop approval.",
        ),
        (
            "demo-audit-memory",
            "add_memory",
            "memory",
            "success",
            "User-controlled demo memory is available for review.",
        ),
    ];
    for (index, (id, action, target_type, outcome, detail)) in events.into_iter().enumerate() {
        conn.execute(
            "INSERT INTO audit_events (id, origin, action, target_type, target_id, detail, outcome, created_at) VALUES (?1, 'demo', ?2, ?3, NULL, ?4, ?5, ?6)",
            params![id, action, target_type, detail, outcome, stamp(now - Duration::minutes((index * 13 + 5) as i64))],
        )?;
    }
    Ok(())
}

fn stamp(value: chrono::DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn same_path(left: &Path, right: &Path) -> bool {
    let left = paths::normalize(left).to_string_lossy().replace('/', "\\");
    let right = paths::normalize(right).to_string_lossy().replace('/', "\\");
    left.eq_ignore_ascii_case(&right)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn showcase_catalog_has_eight_unique_fixture_slugs() {
        assert_eq!(PROJECTS.len(), 8);
        let mut slugs = PROJECTS
            .iter()
            .map(|project| project.slug)
            .collect::<Vec<_>>();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(slugs.len(), PROJECTS.len());
        assert!(PROJECTS.iter().all(|project| !project.tags.is_empty()));
    }

    #[test]
    fn showcase_paths_are_compared_case_insensitively() {
        assert!(same_path(
            Path::new(r"C:\RepoAtlas Showcase\atlas-dashboard"),
            Path::new(r"c:/repoatlas showcase/atlas-dashboard"),
        ));
        assert!(!same_path(
            Path::new(r"C:\RepoAtlas Showcase\atlas-dashboard"),
            Path::new(r"C:\RepoAtlas Showcase\pulse-mobile"),
        ));
    }

    #[test]
    fn every_project_has_a_visible_demo_task_set() {
        for project in PROJECTS {
            assert!(
                !demo_tasks(project.slug).is_empty(),
                "{} has no tasks",
                project.slug
            );
        }
    }
}
