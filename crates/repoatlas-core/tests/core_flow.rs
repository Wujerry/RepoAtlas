use repoatlas_core::{Broker, Core, GitOp, ProjectPatch, ProjectQuery, TaskSpec};
use std::fs;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tempfile::tempdir;

fn write(path: &std::path::Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

#[cfg(windows)]
fn system_tool(name: &str) -> String {
    std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()))
        .join("System32")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn detects_manifest_and_skips_nested_and_node_modules() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write(
        &root.join("hospital-print/package.json"),
        r#"{"name":"hospital-print","scripts":{"dev":"vite"}}"#,
    );
    write(&root.join("hospital-print/README.md"), "医院打印服务");
    write(
        &root.join("hospital-print/node_modules/nested/package.json"),
        r#"{"name":"should-skip"}"#,
    );
    write(
        &root.join("hospital-print/packages/app/package.json"),
        r#"{"name":"nested-app"}"#,
    );
    write(
        &root.join("rust-tool/Cargo.toml"),
        "[package]\nname = \"rust-tool\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    write(&root.join("rust-tool/src/main.rs"), "fn main() {}");

    let core = Core::open_in_memory().unwrap();
    let scan_root = core.add_scan_root(root).unwrap();
    let result = core
        .scan_root(&scan_root.id, &AtomicBool::new(false), &|_| {})
        .unwrap();
    assert!(!result.cancelled);
    let projects = core.list_projects(ProjectQuery::default()).unwrap();
    let names: Vec<_> = projects.iter().map(|p| p.display_name.clone()).collect();
    assert!(names.contains(&"hospital-print".into()), "{names:?}");
    assert!(names.contains(&"rust-tool".into()), "{names:?}");
    assert!(!names
        .iter()
        .any(|name| name == "should-skip" || name == "nested-app"));
}

#[test]
fn chinese_search_and_manual_metadata_roundtrip() {
    let dir = tempdir().unwrap();
    write(
        &dir.path().join("print-service/package.json"),
        r#"{"name":"print-service"}"#,
    );
    write(
        &dir.path().join("print-service/README.md"),
        "这是一个医院打印服务，负责急诊单据。",
    );
    let core = Core::open_in_memory().unwrap();
    core.add_scan_root(dir.path()).unwrap();
    core.scan_all(&AtomicBool::new(false), &|_| {}).unwrap();
    let hits = core.search_projects("医院打印", 10).unwrap();
    assert!(!hits.is_empty());
    let id = hits[0].project.id.clone();
    let updated = core
        .update_project(
            &id,
            ProjectPatch {
                favorite: Some(true),
                tags: Some(vec!["临床".into(), "print".into()]),
                notes: Some("keep local".into()),
                description: Some(Some("用于急诊单据打印的本地服务".into())),
                archived: Some(false),
                display_name: Some("急诊打印".into()),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(updated.favorite);
    assert_eq!(updated.display_name, "急诊打印");
    assert!(updated.tags.contains(&"临床".into()));
    assert_eq!(
        updated.description.as_deref(),
        Some("用于急诊单据打印的本地服务")
    );
    let refreshed = core.refresh_project(&id).unwrap();
    assert_eq!(refreshed.description, updated.description);
    let description_hits = core.search_projects("本地服务", 10).unwrap();
    assert!(description_hits.iter().any(|hit| hit.project.id == id));
    let cleared = core
        .update_project(
            &id,
            ProjectPatch {
                description: Some(None),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(cleared.description, None);
}

#[test]
fn removing_records_does_not_delete_directories() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("kept-project");
    write(
        &project.join("Cargo.toml"),
        "[package]\nname = \"kept-project\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    let core = Core::open_in_memory().unwrap();
    let root = core.add_scan_root(dir.path()).unwrap();
    core.scan_root(&root.id, &AtomicBool::new(false), &|_| {})
        .unwrap();
    let projects = core.list_projects(ProjectQuery::default()).unwrap();
    assert_eq!(projects.len(), 1);
    core.remove_project(&projects[0].id).unwrap();
    assert!(project.exists());
}

#[test]
fn project_removal_with_origin_commits_the_audit_atomically() {
    let dir = tempdir().unwrap();
    let project_path = dir.path().join("atomic-project");
    write(&project_path.join("README.md"), "# atomic");

    let core = Core::open_in_memory().unwrap();
    let project = core.register_project(&project_path).unwrap();
    let removal = core.remove_project_with_origin(&project.id, "mcp").unwrap();

    assert_eq!(removal.project.id, project.id);
    assert!(project_path.exists());
    assert!(core.get_project(&project.id).is_err());
    let events = core.list_audit_events(20).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].origin, "mcp");
    assert_eq!(events[0].action, "remove_project");
    assert_eq!(events[0].target_id.as_deref(), Some(project.id.as_str()));
    assert_eq!(events[0].outcome, "success");
}

#[test]
fn project_removal_with_origin_rolls_back_when_audit_fails() {
    let dir = tempdir().unwrap();
    let project_path = dir.path().join("atomic-project-failure");
    write(&project_path.join("README.md"), "# atomic failure");

    let core = Core::open_in_memory().unwrap();
    let project = core.register_project(&project_path).unwrap();
    core.connection()
        .execute_batch(
            "CREATE TRIGGER reject_repoatlas_audit
             BEFORE INSERT ON audit_events
             BEGIN SELECT RAISE(ABORT, 'audit unavailable'); END;",
        )
        .unwrap();

    let error = core
        .remove_project_with_origin(&project.id, "mcp")
        .unwrap_err();
    assert!(error.to_string().contains("audit unavailable"));
    assert!(core.get_project(&project.id).is_ok());
    assert!(core.list_audit_events(20).unwrap().is_empty());
    assert!(project_path.exists());
}

#[test]
fn scan_root_removal_with_origin_commits_the_audit_atomically() {
    let dir = tempdir().unwrap();
    let project_path = dir.path().join("root-project");
    write(&project_path.join("README.md"), "# root");

    let core = Core::open_in_memory().unwrap();
    let root = core.add_scan_root(dir.path()).unwrap();
    let project = core.register_project(&project_path).unwrap();
    let removal = core
        .remove_scan_root_with_origin(&root.id, false, "mcp")
        .unwrap();

    assert_eq!(removal.root.id, root.id);
    assert_eq!(removal.orphaned_projects, 1);
    assert!(project_path.exists());
    assert!(core.list_scan_roots().unwrap().is_empty());
    let detail = core.get_project(&project.id).unwrap();
    assert!(detail.project.scan_root_id.is_none());
    let events = core.list_audit_events(20).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].origin, "mcp");
    assert_eq!(events[0].action, "remove_scan_root");
    assert_eq!(events[0].target_id.as_deref(), Some(root.id.as_str()));
    assert_eq!(events[0].outcome, "success");
}

#[test]
fn folder_removal_with_origin_commits_records_and_audit_atomically() {
    let dir = tempdir().unwrap();
    let project_path = dir.path().join("folder-project");
    write(&project_path.join("README.md"), "# folder project");

    let core = Core::open_in_memory().unwrap();
    let root = core.add_scan_root(dir.path()).unwrap();
    let project = core.register_project(&project_path).unwrap();
    let removals = core
        .remove_folder_records_with_origin(
            std::slice::from_ref(&project.id),
            std::slice::from_ref(&root.id),
            "desktop",
        )
        .unwrap();

    assert_eq!(removals.len(), 1);
    assert_eq!(removals[0].project.id, project.id);
    assert!(project_path.exists());
    assert!(core.get_project(&project.id).is_err());
    assert!(core.list_scan_roots().unwrap().is_empty());
    let events = core.list_audit_events(20).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].origin, "desktop");
    assert_eq!(events[0].action, "remove_folder_records");
    assert_eq!(events[0].target_type, "folder");
    assert_eq!(events[0].target_id, None);
    assert_eq!(events[0].outcome, "success");
}

#[test]
fn folder_removal_with_origin_rolls_back_when_audit_fails() {
    let dir = tempdir().unwrap();
    let project_path = dir.path().join("folder-project-failure");
    write(&project_path.join("README.md"), "# folder project failure");

    let core = Core::open_in_memory().unwrap();
    let root = core.add_scan_root(dir.path()).unwrap();
    let project = core.register_project(&project_path).unwrap();
    core.connection()
        .execute_batch(
            "CREATE TRIGGER reject_repoatlas_audit
             BEFORE INSERT ON audit_events
             BEGIN SELECT RAISE(ABORT, 'audit unavailable'); END;",
        )
        .unwrap();

    let error = core
        .remove_folder_records_with_origin(
            std::slice::from_ref(&project.id),
            std::slice::from_ref(&root.id),
            "desktop",
        )
        .unwrap_err();
    assert!(error.to_string().contains("audit unavailable"));
    assert!(core.get_project(&project.id).is_ok());
    assert_eq!(core.list_scan_roots().unwrap().len(), 1);
    assert!(core.list_audit_events(20).unwrap().is_empty());
    assert!(project_path.exists());
}

#[test]
fn scan_can_be_cancelled() {
    let dir = tempdir().unwrap();
    write(&dir.path().join("one/package.json"), "{}");
    let core = Core::open_in_memory().unwrap();
    let root = core.add_scan_root(dir.path()).unwrap();
    let cancel = AtomicBool::new(true);
    let result = core.scan_root(&root.id, &cancel, &|_| {}).unwrap();
    assert!(result.cancelled);
}

#[test]
fn git_status_and_typed_commit() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("git-app");
    fs::create_dir_all(&project).unwrap();
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(&project)
            .output()
            .unwrap()
    };
    git(&["init"]);
    git(&["config", "user.email", "repoatlas@example.com"]);
    git(&["config", "user.name", "RepoAtlas"]);
    write(&project.join("README.md"), "hello");
    let core = Core::open_in_memory().unwrap();
    let summary = core.register_project(&project).unwrap();
    let status = core.git_status(&summary.id).unwrap();
    assert!(status
        .files
        .iter()
        .any(|file| file.path.contains("README.md")));
    core.git_execute(
        &summary.id,
        GitOp::Stage {
            paths: vec!["README.md".into()],
        },
    )
    .unwrap();
    let result = core
        .git_execute(
            &summary.id,
            GitOp::Commit {
                message: "init".into(),
            },
        )
        .unwrap();
    assert!(result.ok, "{}", result.stderr);
    let status = core.git_status(&summary.id).unwrap();
    assert_eq!(status.files.len(), 0);
}

#[test]
fn git_project_inside_repository_uses_parent_root_without_changing_identity() {
    let dir = tempdir().unwrap();
    let repository = dir.path().join("mono-repo");
    let project = repository.join("packages/app");
    fs::create_dir_all(&project).unwrap();
    let git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(&repository)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    };
    git(&["init", "-b", "main"]);
    git(&["config", "user.email", "repoatlas@example.com"]);
    git(&["config", "user.name", "RepoAtlas"]);
    git(&[
        "remote",
        "add",
        "origin",
        "https://github.com/RepoAtlas/fixture.git",
    ]);
    write(
        &project.join("package.json"),
        r#"{"name":"nested-app","scripts":{"test":"echo ok"}}"#,
    );
    write(
        &repository.join("pnpm-workspace.yaml"),
        "packages:\n  - packages/*\n",
    );
    write(
        &repository.join("pnpm-lock.yaml"),
        "lockfileVersion: '9.0'\n",
    );
    git(&["add", "."]);
    git(&["commit", "-m", "initial"]);

    let core = Core::open_in_memory().unwrap();
    let summary = core.register_project(&project).unwrap();
    let project_path = repoatlas_core::paths::canonicalize(&project).unwrap();
    let repository_path = repoatlas_core::paths::canonicalize(&repository).unwrap();

    assert_eq!(
        repoatlas_core::git::repository_root(&project),
        Some(repository_path.clone())
    );
    assert_eq!(summary.canonical_path, project_path.to_string_lossy());
    assert_ne!(summary.canonical_path, repository_path.to_string_lossy());
    assert_eq!(summary.vcs_kind, "git");
    assert!(summary
        .package_managers
        .iter()
        .any(|manager| manager == "pnpm"));

    let detail = core.get_project(&summary.id).unwrap();
    let git_snapshot = detail.git.as_ref().expect("nested project Git snapshot");
    assert_eq!(git_snapshot.branch.as_deref(), Some("main"));
    let lineage = detail.lineage.as_ref().expect("nested project lineage");
    assert_eq!(
        lineage.normalized_url.as_deref(),
        Some("https://github.com/repoatlas/fixture")
    );

    let status = core.git_status(&summary.id).unwrap();
    assert_eq!(status.snapshot.branch.as_deref(), Some("main"));
    assert!(status.branches.iter().any(|branch| branch == "main"));
}

#[test]
fn command_broker_runs_structured_argv_and_rejects_shell() {
    let dir = tempdir().unwrap();
    write(
        &dir.path().join("echo-app/package.json"),
        r#"{"name":"echo-app"}"#,
    );
    let db = dir.path().join("core.sqlite");
    let core = Core::open(&db).unwrap();
    let project = core.register_project(dir.path().join("echo-app")).unwrap();
    let logs = dir.path().join("logs");
    let broker = Broker::new(logs).unwrap();
    let rejected = broker.start(
        core.connection(),
        TaskSpec {
            project_id: project.id.clone(),
            task_id: None,
            kind: "run".into(),
            executable: "cmd".into(),
            argv: vec!["/C".into(), "echo".into(), "hi".into()],
            cwd: Some(project.canonical_path.clone()),
            shell_mode: true,
        },
        |_| {},
        |_, _| {},
    );
    assert!(rejected.is_err());

    let interpreter_bypass = broker.start(
        core.connection(),
        TaskSpec {
            project_id: project.id.clone(),
            task_id: None,
            kind: "run".into(),
            executable: "cmd.exe".into(),
            argv: vec!["/C".into(), "echo".into(), "bypass".into()],
            cwd: Some(project.canonical_path.clone()),
            shell_mode: false,
        },
        |_| {},
        |_, _| {},
    );
    assert!(interpreter_bypass.is_err());

    #[cfg(windows)]
    let spec = TaskSpec {
        project_id: project.id.clone(),
        task_id: None,
        kind: "run".into(),
        executable: system_tool("where.exe"),
        argv: vec!["where.exe".into()],
        cwd: Some(project.canonical_path.clone()),
        shell_mode: false,
    };
    #[cfg(not(windows))]
    let spec = TaskSpec {
        project_id: project.id.clone(),
        task_id: None,
        kind: "run".into(),
        executable: "echo".into(),
        argv: vec!["repoatlas-ok".into()],
        cwd: Some(project.canonical_path.clone()),
        shell_mode: false,
    };
    let run = broker
        .start(core.connection(), spec, |_| {}, |_, _| {})
        .unwrap();
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(5) {
        if let Ok(current) = core.get_task_run(&run.id) {
            if current.status != "running" {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(windows)]
#[test]
fn command_broker_stop_stops_descendant_output() {
    let probe = std::process::Command::new("node").arg("--version").output();
    if probe
        .as_ref()
        .map(|output| !output.status.success())
        .unwrap_or(true)
    {
        // The stop guarantee itself is Windows Job Object based; skip only
        // when node is unavailable to drive a multi-process fixture.
        return;
    }
    let dir = tempdir().unwrap();
    write(
        &dir.path().join("runner-app/package.json"),
        r#"{"name":"runner-app"}"#,
    );
    write(
        &dir.path().join("runner-app/runner.cjs"),
        r#"
const { spawn } = require('child_process');
const grand = spawn(process.execPath, ['-e', 'setInterval(function () { process.stdout.write("grand-tick\n"); }, 80);'], { stdio: ['ignore', 'inherit', 'inherit'] });
grand.on('error', function () {});
process.stdout.write('ready\n');
setInterval(function () { process.stdout.write('parent-tick\n'); }, 80);
"#,
    );
    let db = dir.path().join("core.sqlite");
    let core = Core::open(&db).unwrap();
    let project = core
        .register_project(dir.path().join("runner-app"))
        .unwrap();
    let broker = Broker::new(dir.path().join("logs")).unwrap();
    let run = broker
        .start(
            core.connection(),
            TaskSpec {
                project_id: project.id.clone(),
                task_id: None,
                kind: "run".into(),
                executable: "node".into(),
                argv: vec!["runner.cjs".into()],
                cwd: Some(project.canonical_path.clone()),
                shell_mode: false,
            },
            |_| {},
            |_, _| {},
        )
        .unwrap();

    let log_path = dir.path().join("logs").join(format!("{}.log", run.id));
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(5) {
        match fs::read_to_string(&log_path) {
            Ok(content) if content.contains("grand-tick") => break,
            _ => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    assert!(fs::read_to_string(&log_path)
        .unwrap()
        .contains("grand-tick"));

    broker.stop(core.connection(), &run.id).unwrap();
    assert!(broker.active().unwrap().iter().all(|id| id != &run.id));

    let size_after_stop = fs::metadata(&log_path).unwrap().len();
    std::thread::sleep(Duration::from_millis(1200));
    let size_later = fs::metadata(&log_path).unwrap().len();
    assert_eq!(
        size_after_stop, size_later,
        "descendant processes kept writing after stop"
    );
}

#[test]
fn runtime_lock_allows_mcp_without_duplicate_desktop_recovery() {
    let dir = tempdir().unwrap();
    let db = dir.path().join("core.sqlite");
    let desktop = Core::open(&db).unwrap();
    assert!(Core::open(&db).is_err());

    let mcp = Core::open_without_recovery(&db).unwrap();
    assert!(mcp.list_projects(ProjectQuery::default()).is_ok());
    drop(mcp);
    drop(desktop);

    assert!(Core::open(&db).is_ok());
}

#[cfg(windows)]
#[test]
fn windows_script_launcher_does_not_evaluate_task_arguments() {
    let dir = tempdir().unwrap();
    let project_path = dir.path().join("project & tools");
    write(
        &project_path.join("package.json"),
        r#"{"name":"script-safety"}"#,
    );
    // The directory deliberately mixes spaces and metacharacters: the
    // launcher addresses the script by its bare file name, which cmd.exe
    // resolves against the task working directory, while the quoted
    // argument must reach the script as data.
    let script = project_path.join("build.cmd");
    write(
        &script,
        "@echo off\r\n> \"%~dp0ran.txt\" echo ran\r\nexit /b 0\r\n",
    );
    let db = dir.path().join("core.sqlite");
    let core = Core::open(&db).unwrap();
    let project = core.register_project(&project_path).unwrap();
    let broker = Broker::new(dir.path().join("logs")).unwrap();
    let run = broker
        .start(
            core.connection(),
            TaskSpec {
                project_id: project.id,
                task_id: None,
                kind: "build".into(),
                executable: script.to_string_lossy().into_owned(),
                argv: vec!["safe & echo owned > injected.txt | type nul < nul ^ ! ( )".into()],
                cwd: Some(project.canonical_path),
                shell_mode: false,
            },
            |_| {},
            |_, _| {},
        )
        .unwrap();

    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(5)
        && broker.active().unwrap().iter().any(|id| id == &run.id)
    {
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(project_path.join("ran.txt").is_file());
    assert!(!project_path.join("injected.txt").exists());
}

#[cfg(windows)]
#[test]
fn windows_script_launcher_rejects_unrepresentable_script_paths_without_leaving_a_running_record() {
    let dir = tempdir().unwrap();
    let project_path = dir.path().join("project & tools");
    write(
        &project_path.join("package.json"),
        r#"{"name":"script-safety"}"#,
    );
    // cmd.exe /S strips the payload quotes, so a script whose own name
    // carries metacharacters has no safe command-line form. The start
    // attempt must fail loudly and close the database record instead of
    // launching something unintended.
    let script = project_path.join("build&release.cmd");
    write(&script, "@echo off\r\nexit /b 0\r\n");
    let db = dir.path().join("core.sqlite");
    let core = Core::open(&db).unwrap();
    let project = core.register_project(&project_path).unwrap();
    let broker = Broker::new(dir.path().join("logs")).unwrap();
    let error = broker
        .start(
            core.connection(),
            TaskSpec {
                project_id: project.id,
                task_id: None,
                kind: "build".into(),
                executable: script.to_string_lossy().into_owned(),
                argv: vec![],
                cwd: Some(project.canonical_path),
                shell_mode: false,
            },
            |_| {},
            |_, _| {},
        )
        .unwrap_err();
    assert!(error.to_string().contains("cannot be expressed safely"));
    assert!(core.list_active_task_runs().unwrap().is_empty());
}

#[cfg(windows)]
#[test]
fn command_broker_executes_project_local_windows_tool_shims() {
    let dir = tempdir().unwrap();
    let project_path = dir.path().join("local-tool-project");
    write(
        &project_path.join("package.json"),
        r#"{"name":"local-tool-project"}"#,
    );
    write(
        &project_path.join("node_modules/.bin/repoatlas-tool.cmd"),
        "@echo off\r\n> \"%~dp0\\..\\..\\executed.txt\" echo %~1\r\nexit /b 0\r\n",
    );
    let core = Core::open(dir.path().join("core.sqlite")).unwrap();
    let project = core.register_project(&project_path).unwrap();
    let broker = Broker::new(dir.path().join("logs")).unwrap();
    let run = broker
        .start(
            core.connection(),
            TaskSpec {
                project_id: project.id,
                task_id: None,
                kind: "build".into(),
                executable: "repoatlas-tool".into(),
                argv: vec!["local-ok".into()],
                cwd: Some(project.canonical_path),
                shell_mode: false,
            },
            |_| {},
            |_, _| {},
        )
        .unwrap();

    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(5)
        && broker.active().unwrap().iter().any(|id| id == &run.id)
    {
        std::thread::sleep(Duration::from_millis(25));
    }
    assert_eq!(
        fs::read_to_string(project_path.join("executed.txt"))
            .unwrap()
            .trim(),
        "local-ok"
    );
}

#[test]
fn command_broker_forwards_stdin() {
    let dir = tempdir().unwrap();
    write(
        &dir.path().join("echo-app/package.json"),
        r#"{"name":"echo-app"}"#,
    );
    let db = dir.path().join("core.sqlite");
    let core = Core::open(&db).unwrap();
    let project = core.register_project(dir.path().join("echo-app")).unwrap();
    let logs = dir.path().join("logs");
    let broker = Broker::new(logs).unwrap();
    #[cfg(windows)]
    let spec = TaskSpec {
        project_id: project.id.clone(),
        task_id: None,
        kind: "run".into(),
        executable: system_tool("more.com"),
        argv: vec![],
        cwd: Some(project.canonical_path.clone()),
        shell_mode: false,
    };
    #[cfg(not(windows))]
    let spec = TaskSpec {
        project_id: project.id.clone(),
        task_id: None,
        kind: "run".into(),
        executable: "cat".into(),
        argv: vec![],
        cwd: Some(project.canonical_path.clone()),
        shell_mode: false,
    };
    let chunks = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let observed = chunks.clone();
    let run = broker
        .start(
            core.connection(),
            spec,
            move |chunk| observed.lock().unwrap().push(chunk.text),
            |_, _| {},
        )
        .unwrap();
    std::thread::sleep(Duration::from_millis(200));
    broker.write_stdin(&run.id, "repoatlas-in\n").unwrap();
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(8) {
        let joined = chunks.lock().unwrap().join("");
        if joined.contains("repoatlas-in") {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let joined = chunks.lock().unwrap().join("");
    assert!(joined.contains("repoatlas-in"), "{joined}");
    let _ = broker.stop(core.connection(), &run.id);
}
#[test]
fn export_import_and_backup_roundtrip() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("export-app");
    write(&project.join("package.json"), r#"{"name":"export-app"}"#);
    let core = Core::open_in_memory().unwrap();
    let summary = core.register_project(&project).unwrap();
    core.update_project(
        &summary.id,
        ProjectPatch {
            favorite: Some(true),
            notes: Some("keep me".into()),
            description: Some(Some("exported description".into())),
            tags: Some(vec!["临床".into()]),
            ..Default::default()
        },
    )
    .unwrap();
    let icon_path = dir.path().join("export-icon.png");
    let mut icon_bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
    icon_bytes.extend_from_slice(b"IHDR");
    icon_bytes.extend_from_slice(&24u32.to_be_bytes());
    icon_bytes.extend_from_slice(&24u32.to_be_bytes());
    fs::write(&icon_path, icon_bytes).unwrap();
    core.set_project_icon(&summary.id, &icon_path).unwrap();
    let exported = core.export_json().unwrap();
    assert!(exported.contains("export-app"));
    assert!(exported.contains("keep me"));
    assert!(exported.contains("exported description"));
    // import into a fresh store
    let other = Core::open_in_memory().unwrap();
    let count = other.import_json(&exported).unwrap();
    assert_eq!(count, 1);
    let projects = other.list_projects(ProjectQuery::default()).unwrap();
    assert_eq!(projects.len(), 1);
    assert!(projects[0].favorite);
    assert_eq!(
        projects[0].description.as_deref(),
        Some("exported description")
    );
    assert!(other
        .search_projects("exported description", 10)
        .unwrap()
        .iter()
        .any(|hit| hit.project.id == projects[0].id));
    assert_eq!(
        other.read_project_icons(&[projects[0].id.clone()]).unwrap()[0].kind,
        "override"
    );

    let mut old_file: serde_json::Value = serde_json::from_str(&exported).unwrap();
    old_file["version"] = serde_json::json!(1);
    old_file["projects"][0]
        .as_object_mut()
        .unwrap()
        .remove("description");
    old_file["projects"][0]
        .as_object_mut()
        .unwrap()
        .remove("icon");
    let old_import = Core::open_in_memory().unwrap();
    assert_eq!(old_import.import_json(&old_file.to_string()).unwrap(), 1);
    assert_eq!(
        old_import.list_projects(ProjectQuery::default()).unwrap()[0].description,
        None
    );
    // backup
    let dest = dir.path().join("backup.sqlite");
    other.backup_db(&dest).unwrap();
    let reloaded = Core::open(&dest).unwrap();
    assert_eq!(
        reloaded
            .list_projects(ProjectQuery::default())
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn import_rejects_filesystem_root_and_non_directory_without_writes() {
    let dir = tempdir().unwrap();
    let non_directory = dir.path().join("not-a-directory");
    write(&non_directory, "not a directory");
    let filesystem_root = if cfg!(windows) {
        std::path::PathBuf::from(r"C:\")
    } else {
        std::path::PathBuf::from("/")
    };

    for invalid_path in [filesystem_root, non_directory] {
        let text = serde_json::json!({
            "format": "repoatlas-export",
            "version": 3,
            "exportedAt": "2026-08-31T00:00:00Z",
            "settings": {
                "theme": "dark",
                "locale": "en",
                "uiFont": "",
                "consoleFont": ""
            },
            "scanRoots": [{
                "id": "imported-root",
                "path": invalid_path.to_string_lossy(),
                "createdAt": "2026-08-31T00:00:00Z",
                "lastScannedAt": null
            }],
            "projects": []
        })
        .to_string();
        let core = Core::open_in_memory().unwrap();

        assert!(core.import_json(&text).is_err());
        assert!(core.list_scan_roots().unwrap().is_empty());
        assert!(core
            .list_projects(ProjectQuery::default())
            .unwrap()
            .is_empty());
        let settings: i64 = core
            .connection()
            .query_row("SELECT COUNT(*) FROM settings", [], |row| row.get(0))
            .unwrap();
        assert_eq!(settings, 0);
    }
}

#[test]
fn invalid_import_icon_rolls_back_settings_roots_and_projects() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("icon-import-app");
    write(
        &project.join("package.json"),
        r#"{"name":"icon-import-app"}"#,
    );
    let text = serde_json::json!({
        "format": "repoatlas-export",
        "version": 3,
        "exportedAt": "2026-08-31T00:00:00Z",
        "settings": {
            "theme": "dark",
            "locale": "en",
            "uiFont": "",
            "consoleFont": ""
        },
        "scanRoots": [{
            "id": "imported-root",
            "path": dir.path().to_string_lossy(),
            "createdAt": "2026-08-31T00:00:00Z",
            "lastScannedAt": null
        }],
        "projects": [{
            "canonicalPath": project.to_string_lossy(),
            "displayName": "icon-import-app",
            "notes": null,
            "description": null,
            "favorite": false,
            "archived": false,
            "tags": [],
            "lastOpenedAt": null,
            "icon": {
                "mimeType": "image/png",
                "data": "not-base64",
                "sourceName": "invalid.png"
            }
        }]
    })
    .to_string();
    let core = Core::open_in_memory().unwrap();

    assert!(core.import_json(&text).is_err());
    assert!(core.list_scan_roots().unwrap().is_empty());
    assert!(core
        .list_projects(ProjectQuery::default())
        .unwrap()
        .is_empty());
    let settings: i64 = core
        .connection()
        .query_row("SELECT COUNT(*) FROM settings", [], |row| row.get(0))
        .unwrap();
    assert_eq!(settings, 0);
}

#[test]
fn authorized_scan_root_survives_export_import_and_links_projects() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("rooted-app");
    write(&project.join("package.json"), r#"{"name":"rooted-app"}"#);

    let source = Core::open_in_memory().unwrap();
    let root = source.add_scan_root(dir.path()).unwrap();
    let registered = source.register_project(&project).unwrap();
    assert_eq!(registered.scan_root_id.as_deref(), Some(root.id.as_str()));
    let exported = source.export_json().unwrap();

    let destination = Core::open_in_memory().unwrap();
    assert_eq!(destination.import_json(&exported).unwrap(), 1);
    let roots = destination.list_scan_roots().unwrap();
    assert_eq!(roots.len(), 1);
    assert_eq!(
        roots[0].path,
        repoatlas_core::paths::canonicalize(dir.path())
            .unwrap()
            .to_string_lossy()
    );
    let imported = destination.list_projects(ProjectQuery::default()).unwrap();
    assert_eq!(imported.len(), 1);
    assert_eq!(
        imported[0].scan_root_id.as_deref(),
        Some(roots[0].id.as_str())
    );
}

#[test]
fn project_readme_reader_rejects_an_external_symlink() {
    let project = tempdir().unwrap();
    let outside = tempdir().unwrap();
    fs::write(outside.path().join("secret.md"), "outside secret").unwrap();

    #[cfg(unix)]
    std::os::unix::fs::symlink(
        outside.path().join("secret.md"),
        project.path().join("README.md"),
    )
    .unwrap();
    #[cfg(windows)]
    if std::os::windows::fs::symlink_file(
        outside.path().join("secret.md"),
        project.path().join("README.md"),
    )
    .is_err()
    {
        return;
    }
    #[cfg(not(any(unix, windows)))]
    return;

    let core = Core::open_in_memory().unwrap();
    let summary = core.register_project(project.path()).unwrap();
    assert!(core.read_project_readme(&summary.id).is_err());
    assert!(core
        .read_project_document(&summary.id, "README.md")
        .is_err());
}

#[test]
fn readme_document_is_bounded_and_detected_facts_are_stable() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("facts-app");
    write(
        &project.join("package.json"),
        r#"{"name":"facts-app","dependencies":{"react":"18"},"devDependencies":{"vite":"5"}}"#,
    );
    write(&project.join("pnpm-lock.yaml"), "lockfileVersion: '9.0'");
    write(&project.join("tsconfig.json"), "{}");
    write(&project.join("Dockerfile"), "FROM node:22");
    write(&project.join("README.md"), "small readme");
    let core = Core::open_in_memory().unwrap();
    let summary = core.register_project(&project).unwrap();
    let detail = core.get_project(&summary.id).unwrap();
    let facts = &detail.facts;
    for (kind, value) in [
        ("manifest", "package.json"),
        ("manifest", "Dockerfile"),
        ("language", "JavaScript"),
        ("language", "TypeScript"),
        ("framework", "React"),
        ("framework", "Vite"),
        ("packageManager", "pnpm"),
        ("container", "docker"),
        ("lockfile", "pnpm-lock.yaml"),
    ] {
        assert!(
            facts
                .iter()
                .any(|fact| fact.kind == kind && fact.value == value),
            "missing {kind}={value}: {facts:?}"
        );
    }
    for pair in facts.windows(2) {
        assert!(
            (fact_rank(&pair[0].kind), &pair[0].value, &pair[0].source)
                <= (fact_rank(&pair[1].kind), &pair[1].value, &pair[1].source),
            "facts are not stable: {facts:?}"
        );
    }
    assert_eq!(
        facts
            .iter()
            .filter(|fact| fact.kind == "manifest" && fact.value == "package.json")
            .count(),
        1
    );

    let readme = core.read_project_readme(&summary.id).unwrap();
    assert!(!readme.truncated);
    assert_eq!(readme.path, "README.md");
    assert_eq!(readme.content, "small readme");

    let large = "x".repeat(1024 * 1024 + 1);
    fs::write(project.join("README.md"), large).unwrap();
    let refreshed = core.refresh_project(&summary.id).unwrap();
    let bounded = core.read_project_readme(&refreshed.id).unwrap();
    assert!(bounded.truncated);
    assert_eq!(bounded.content.len(), 1024 * 1024);
}

fn fact_rank(kind: &str) -> usize {
    match kind {
        "language" => 0,
        "framework" => 1,
        "packageManager" => 2,
        "manifest" => 3,
        "container" => 4,
        "lockfile" => 5,
        _ => 6,
    }
}
#[test]
fn detects_mainstream_stacks_and_task_descriptions() {
    let dir = tempdir().unwrap();
    write(
        &dir.path().join("web/package.json"),
        r#"{"name":"web","scripts":{"dev":"vite"},"dependencies":{"next":"14.0.0","react":"18.0.0"}}"#,
    );
    write(
        &dir.path().join("app/pubspec.yaml"),
        "name: mobile\nflutter:\n  assets: []\n",
    );
    write(
        &dir.path().join("service/go.mod"),
        "module example.com/service\ngo 1.22\n",
    );
    write(
        &dir.path().join("api/Pipfile"),
        "[[source]]\nname = \"pypi\"\n",
    );
    let web = repoatlas_core::detect(&dir.path().join("web"));
    assert!(web.frameworks.iter().any(|item| item == "Next.js"));
    assert!(web
        .tasks
        .iter()
        .any(|task| task.name == "dev" && task.description.is_some()));
    let app = repoatlas_core::detect(&dir.path().join("app"));
    assert!(app.frameworks.iter().any(|item| item == "Flutter"));
    assert!(app
        .tasks
        .iter()
        .any(|task| task.executable == "flutter" && task.kind == "dev"));
    let service = repoatlas_core::detect(&dir.path().join("service"));
    assert!(service.languages.iter().any(|item| item == "Go"));
    assert!(service.tasks.iter().any(|task| task.kind == "test"));
    let api = repoatlas_core::detect(&dir.path().join("api"));
    assert!(api.package_managers.iter().any(|item| item == "pipenv"));
}

#[test]
fn avoids_inferred_tasks_without_a_runnable_entry_point() {
    let dir = tempdir().unwrap();
    write(
        &dir.path().join("rust-lib/Cargo.toml"),
        "[package]\nname = \"rust-lib\"\nversion = \"0.1.0\"\n",
    );
    write(
        &dir.path().join("rust-lib/src/lib.rs"),
        "pub fn value() -> u8 { 1 }\n",
    );
    write(
        &dir.path().join("python-lib/pyproject.toml"),
        "[project]\nname = \"python-lib\"\ndependencies = [\"fastapi\"]\n",
    );

    let rust = repoatlas_core::detect(&dir.path().join("rust-lib"));
    assert!(!rust
        .tasks
        .iter()
        .any(|task| task.executable == "cargo" && task.argv == ["run"]));
    let python = repoatlas_core::detect(&dir.path().join("python-lib"));
    assert!(!python
        .tasks
        .iter()
        .any(|task| task.argv.iter().any(|arg| arg == "main:app")));
    assert!(!python.tasks.iter().any(|task| task.kind == "run"));
}

#[test]
fn add_task_appends_a_described_custom_task() {
    let dir = tempdir().unwrap();
    write(&dir.path().join("demo/package.json"), r#"{"name":"demo"}"#);
    let core = Core::open_in_memory().unwrap();
    let project = core.register_project(dir.path().join("demo")).unwrap();
    let detail = core
        .add_task(
            &project.id,
            repoatlas_core::TaskDefinition {
                id: String::new(),
                kind: "dev".into(),
                name: "preview".into(),
                description: Some("Open the local preview".into()),
                executable: "pnpm".into(),
                argv: vec!["run".into(), "preview".into()],
                cwd: None,
                inferred: true,
                shell_mode: false,
            },
        )
        .unwrap();
    let added = detail
        .tasks
        .iter()
        .find(|task| task.name == "preview")
        .unwrap();
    assert_eq!(added.description.as_deref(), Some("Open the local preview"));
    assert!(!added.inferred);
}
#[test]
fn detects_java_kotlin_android_and_rust_tooling() {
    let dir = tempdir().unwrap();
    write(
        &dir.path().join("maven-app/pom.xml"),
        r#"<project><artifactId>clinic-api</artifactId><dependencies><dependency><artifactId>spring-boot-starter-web</artifactId></dependency></dependencies></project>"#,
    );
    write(&dir.path().join("maven-app/mvnw"), "#!/bin/sh\n");
    write(&dir.path().join("maven-app/mvnw.cmd"), "@echo off\n");
    write(
        &dir.path().join("android-app/build.gradle.kts"),
        "plugins { id(\"com.android.application\") }\ndependencies { implementation(\"org.jetbrains.kotlin:kotlin-stdlib\") }\n",
    );
    write(&dir.path().join("android-app/gradlew"), "#!/bin/sh\n");
    write(&dir.path().join("android-app/gradlew.bat"), "@echo off\n");
    write(
        &dir.path()
            .join("android-app/app/src/main/AndroidManifest.xml"),
        "<manifest />\n",
    );
    write(
        &dir.path().join("rust-web/Cargo.toml"),
        "[package]\nname = \"rust-web\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\naxum = \"0.7\"\n",
    );
    write(
        &dir.path().join("rust-web/src-tauri/tauri.conf.json"),
        "{}\n",
    );
    let maven = repoatlas_core::detect(&dir.path().join("maven-app"));
    assert_eq!(maven.name, "clinic-api");
    assert!(maven.languages.iter().any(|item| item == "Java"));
    assert!(maven.frameworks.iter().any(|item| item == "Spring Boot"));
    assert!(maven
        .tasks
        .iter()
        .any(|task| task.argv.iter().any(|arg| arg == "spring-boot:run")));
    let expected_maven = if cfg!(windows) { "mvnw.cmd" } else { "./mvnw" };
    assert!(maven
        .tasks
        .iter()
        .all(|task| task.executable == expected_maven));
    let android = repoatlas_core::detect(&dir.path().join("android-app"));
    assert!(android.languages.iter().any(|item| item == "Kotlin"));
    assert!(android.frameworks.iter().any(|item| item == "Android"));
    assert!(android
        .tasks
        .iter()
        .any(|task| task.argv.iter().any(|arg| arg == "assembleDebug")));
    let rust = repoatlas_core::detect(&dir.path().join("rust-web"));
    assert!(rust.languages.iter().any(|item| item == "Rust"));
    assert!(rust.frameworks.iter().any(|item| item == "Tauri"));
    assert!(rust.frameworks.iter().any(|item| item == "Rust Web"));
    assert!(rust
        .tasks
        .iter()
        .any(|task| task.argv.iter().any(|arg| arg == "clippy")));
    assert!(rust
        .tasks
        .iter()
        .any(|task| task.argv.iter().any(|arg| arg == "tauri")));
}
#[test]
fn detects_python_go_php_ruby_dotnet_and_native_stacks() {
    let dir = tempdir().unwrap();
    write(&dir.path().join("py/pyproject.toml"), "[project]\nname = \"clinic\"\n\n[tool.uv]\n\n[project.dependencies]\nfastapi = \"^0.115\"\n");
    write(&dir.path().join("py/uv.lock"), "version = 1\n");
    write(
        &dir.path().join("py/main.py"),
        "from fastapi import FastAPI\napp = FastAPI()\n",
    );
    write(
        &dir.path().join("go/go.mod"),
        "module example.com/api\n\nrequire github.com/gin-gonic/gin v1.10.0\n",
    );
    write(
        &dir.path().join("php/composer.json"),
        r#"{"require":{"laravel/framework":"^11.0"}}"#,
    );
    write(&dir.path().join("php/artisan"), "#!/usr/bin/env php\n");
    write(
        &dir.path().join("rb/Gemfile"),
        "source \"https://rubygems.org\"\ngem \"rails\"\n",
    );
    write(
        &dir.path().join("rb/config/application.rb"),
        "module App; class Application; end; end\n",
    );
    write(
        &dir.path().join("dotnet/api.csproj"),
        "<Project Sdk=\"Microsoft.NET.Sdk.Web\"></Project>\n",
    );
    write(
        &dir.path().join("native/CMakeLists.txt"),
        "cmake_minimum_required(VERSION 3.20)\nproject(demo)\n",
    );
    write(
        &dir.path().join("swift/Package.swift"),
        "// swift-tools-version: 5.9\nimport PackageDescription\n",
    );
    let py = repoatlas_core::detect(&dir.path().join("py"));
    assert!(py.package_managers.iter().any(|item| item == "uv"));
    assert!(py.frameworks.iter().any(|item| item == "FastAPI"));
    assert!(py
        .tasks
        .iter()
        .any(|task| task.executable == "uv" && task.argv.iter().any(|arg| arg == "uvicorn")));
    let go = repoatlas_core::detect(&dir.path().join("go"));
    assert!(go.frameworks.iter().any(|item| item == "Gin"));
    assert!(go
        .tasks
        .iter()
        .any(|task| task.argv.iter().any(|arg| arg == "vet")));
    let php = repoatlas_core::detect(&dir.path().join("php"));
    assert!(php.frameworks.iter().any(|item| item == "Laravel"));
    let rb = repoatlas_core::detect(&dir.path().join("rb"));
    assert!(rb.frameworks.iter().any(|item| item == "Rails"));
    let dotnet = repoatlas_core::detect(&dir.path().join("dotnet"));
    assert!(dotnet.languages.iter().any(|item| item == "C#"));
    assert!(dotnet.frameworks.iter().any(|item| item == "ASP.NET"));
    let native = repoatlas_core::detect(&dir.path().join("native"));
    assert!(native.package_managers.iter().any(|item| item == "cmake"));
    let swift = repoatlas_core::detect(&dir.path().join("swift"));
    assert!(swift.languages.iter().any(|item| item == "Swift"));
    assert!(swift.package_managers.iter().any(|item| item == "spm"));
}

#[test]
fn detects_runtime_constraints_and_project_icon() {
    let dir = tempdir().unwrap();
    write(
        &dir.path().join("js/package.json"),
        r#"{"name":"atlas","engines":{"node":"^18.18.0"}}"#,
    );
    write(
        &dir.path().join("py/pyproject.toml"),
        "[project]\nname = \"clinic\"\nrequires-python = \">=3.11\"\n",
    );
    write(
        &dir.path().join("rust/Cargo.toml"),
        "[package]\nname = \"tool\"\nversion = \"0.1.0\"\nrust-version = \"1.80\"\n",
    );
    write(
        &dir.path().join("go/go.mod"),
        "module example.com/api\n\ngo 1.22.3\n",
    );
    let js = repoatlas_core::detect(&dir.path().join("js"));
    assert!(js
        .facts
        .iter()
        .any(|fact| fact.kind == "runtime" && fact.value.contains("node@")));
    let py = repoatlas_core::detect(&dir.path().join("py"));
    assert!(py.facts.iter().any(|fact| fact.value.contains("python@")));
    let rust = repoatlas_core::detect(&dir.path().join("rust"));
    assert!(rust
        .facts
        .iter()
        .any(|fact| fact.value.contains("rust@1.80")));
    let go = repoatlas_core::detect(&dir.path().join("go"));
    assert!(go.facts.iter().any(|fact| fact.value.contains("go@1.22.3")));
}

#[test]
fn custom_project_icon_overrides_and_restores_detection() {
    let dir = tempdir().unwrap();
    let project_root = dir.path().join("icon-app");
    write(&project_root.join("package.json"), r#"{"name":"icon-app"}"#);
    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
    png.extend_from_slice(b"IHDR");
    png.extend_from_slice(&32u32.to_be_bytes());
    png.extend_from_slice(&32u32.to_be_bytes());
    fs::write(project_root.join("icon.png"), png).unwrap();
    let custom = dir.path().join("custom.webp");
    let mut webp = vec![b'R', b'I', b'F', b'F', 22, 0, 0, 0, b'W', b'E', b'B', b'P'];
    webp.extend_from_slice(b"VP8X");
    webp.extend_from_slice(&[10, 0, 0, 0, 0, 0, 0, 0]);
    webp.extend_from_slice(&[31, 0, 0, 31, 0, 0]);
    fs::write(&custom, webp).unwrap();

    let core = Core::open_in_memory().unwrap();
    let project = core.register_project(&project_root).unwrap();
    let automatic = core
        .read_project_icons(std::slice::from_ref(&project.id))
        .unwrap();
    assert_eq!(automatic[0].kind, "asset");

    let overridden = core.set_project_icon(&project.id, &custom).unwrap();
    assert_eq!(overridden.kind, "override");
    assert_eq!(overridden.mime_type.as_deref(), Some("image/webp"));
    assert_eq!(overridden.source.as_deref(), Some("custom.webp"));
    let persisted = core
        .read_project_icons(std::slice::from_ref(&project.id))
        .unwrap();
    assert_eq!(persisted[0].kind, "override");

    let restored = core.clear_project_icon(&project.id).unwrap();
    assert_eq!(restored.kind, "asset");
    assert!(core
        .set_project_icon(&project.id, dir.path().join("missing.png"))
        .is_err());
    let unsupported = dir.path().join("icon.svg");
    write(&unsupported, "<svg />");
    assert!(core.set_project_icon(&project.id, unsupported).is_err());
    let huge = dir.path().join("huge.png");
    let mut huge_png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
    huge_png.extend_from_slice(b"IHDR");
    huge_png.extend_from_slice(&4096u32.to_be_bytes());
    huge_png.extend_from_slice(&4096u32.to_be_bytes());
    fs::write(&huge, huge_png).unwrap();
    assert!(core.set_project_icon(&project.id, huge).is_err());
}

#[test]
fn relocation_preserves_the_record_and_rejects_registered_targets() {
    let dir = tempdir().unwrap();
    let old_path = dir.path().join("old-app");
    let new_path = dir.path().join("new-app");
    let occupied_path = dir.path().join("occupied-app");
    write(&old_path.join("package.json"), r#"{"name":"old-app"}"#);
    write(&new_path.join("package.json"), r#"{"name":"new-app"}"#);
    write(
        &occupied_path.join("package.json"),
        r#"{"name":"occupied-app"}"#,
    );

    let core = Core::open_in_memory().unwrap();
    let original = core.register_project(&old_path).unwrap();
    let occupied = core.register_project(&occupied_path).unwrap();
    let relocated = core.relocate_project(&original.id, &new_path).unwrap();

    assert_eq!(relocated.id, original.id);
    assert!(relocated.canonical_path.ends_with("new-app"));
    assert_eq!(
        core.list_projects(ProjectQuery::default()).unwrap().len(),
        2
    );
    assert!(core.relocate_project(&original.id, &occupied_path).is_err());
    assert_eq!(
        core.get_project(&occupied.id).unwrap().project.id,
        occupied.id
    );
}

#[test]
fn repeated_project_opens_do_not_fill_recent_activity() {
    let dir = tempdir().unwrap();
    let project_path = dir.path().join("open-app");
    write(&project_path.join("README.md"), "# open-app");
    let core = Core::open_in_memory().unwrap();
    let project = core.register_project(&project_path).unwrap();

    core.mark_opened(&project.id).unwrap();
    core.mark_opened(&project.id).unwrap();
    core.mark_opened(&project.id).unwrap();

    let events = core.list_project_events(&project.id, 20).unwrap();
    let opens = events
        .iter()
        .filter(|event| event.kind == "open" && event.title == "Opened project")
        .count();
    assert_eq!(opens, 1);
}

#[test]
fn project_events_reports_and_task_approvals_form_a_local_p1_loop() {
    let dir = tempdir().unwrap();
    let project_path = dir.path().join("p1-app");
    write(
        &project_path.join("package.json"),
        r#"{"name":"p1-app","scripts":{"dev":"vite"}}"#,
    );
    let core = Core::open_in_memory().unwrap();
    let project = core.register_project(&project_path).unwrap();

    core.record_project_event(&project.id, "ide", "Opened IDE", Some("vscode"))
        .unwrap();
    let events = core.list_project_events(&project.id, 5).unwrap();
    assert_eq!(events[0].kind, "ide");
    assert!(core
        .atlas_report(&project.id)
        .unwrap()
        .markdown
        .contains("## Start here"));

    let approval = core
        .request_task_approval(TaskSpec {
            project_id: project.id.clone(),
            task_id: None,
            kind: "dev".into(),
            executable: "pnpm".into(),
            argv: vec!["dev".into()],
            cwd: Some(project.canonical_path.clone()),
            shell_mode: false,
        })
        .unwrap();
    assert_eq!(approval.status, "pending");
    assert_eq!(core.list_pending_approvals().unwrap().len(), 1);

    let (resolved, spec) = core.resolve_task_approval(&approval.id, true).unwrap();
    assert_eq!(resolved.status, "starting");
    assert_eq!(spec.unwrap().kind, "dev");
    assert!(core.list_pending_approvals().unwrap().is_empty());
}

#[test]
fn task_approval_resolves_relative_working_directory_against_project() {
    let dir = tempdir().unwrap();
    let project_path = dir.path().join("relative-cwd");
    write(
        &project_path.join("package.json"),
        r#"{"name":"relative-cwd"}"#,
    );
    fs::create_dir_all(project_path.join("scripts")).unwrap();
    let core = Core::open_in_memory().unwrap();
    let project = core.register_project(&project_path).unwrap();
    let detail = core
        .add_task(
            &project.id,
            repoatlas_core::TaskDefinition {
                id: String::new(),
                kind: "test".into(),
                name: "relative".into(),
                description: None,
                executable: "node".into(),
                argv: vec!["test.js".into()],
                cwd: Some("scripts".into()),
                inferred: false,
                shell_mode: false,
            },
        )
        .unwrap();
    let task = detail
        .tasks
        .iter()
        .find(|task| task.name == "relative")
        .unwrap();
    let approval = core
        .request_task_approval(TaskSpec {
            project_id: project.id,
            task_id: Some(task.id.clone()),
            kind: String::new(),
            executable: String::new(),
            argv: Vec::new(),
            cwd: None,
            shell_mode: false,
        })
        .unwrap();
    let expected = project_path.join("scripts").canonicalize().unwrap();
    let approval_cwd = std::path::PathBuf::from(approval.cwd.as_deref().unwrap());
    assert!(repoatlas_core::paths::is_within(&approval_cwd, &expected));
    assert!(repoatlas_core::paths::is_within(&expected, &approval_cwd));
    let (_, spec) = core.resolve_task_approval(&approval.id, true).unwrap();
    let spec_cwd = std::path::PathBuf::from(spec.unwrap().cwd.unwrap());
    assert!(repoatlas_core::paths::is_within(&spec_cwd, &expected));
    assert!(repoatlas_core::paths::is_within(&expected, &spec_cwd));
}

#[test]
fn task_approval_is_denied_when_saved_definition_changes() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("approval-consistency");
    fs::create_dir_all(&project).unwrap();
    let core = Core::open_in_memory().unwrap();
    let summary = core.register_project(&project).unwrap();
    let detail = core
        .add_task(
            &summary.id,
            repoatlas_core::TaskDefinition {
                id: "dev".into(),
                kind: "dev".into(),
                name: "Dev server".into(),
                description: None,
                executable: "node".into(),
                argv: vec!["server.js".into()],
                cwd: None,
                inferred: false,
                shell_mode: false,
            },
        )
        .unwrap();
    let approval = core
        .request_task_approval(repoatlas_core::TaskSpec {
            project_id: summary.id.clone(),
            task_id: Some(detail.tasks[0].id.clone()),
            kind: "dev".into(),
            executable: "node".into(),
            argv: vec!["server.js".into()],
            cwd: None,
            shell_mode: false,
        })
        .unwrap();
    core.update_task(
        &summary.id,
        "dev",
        repoatlas_core::TaskDefinition {
            id: "dev".into(),
            kind: "dev".into(),
            name: "Dev server".into(),
            description: None,
            executable: "node".into(),
            argv: vec!["changed.js".into()],
            cwd: None,
            inferred: false,
            shell_mode: false,
        },
    )
    .unwrap();

    let error = core.resolve_task_approval(&approval.id, true).unwrap_err();
    assert!(error.to_string().contains("task definition changed"));
    let stored = core.get_pending_approval(&approval.id).unwrap();
    assert_eq!(stored.status, "denied");
    assert!(stored.resolved_at.is_some());
    assert_eq!(
        stored.error.as_deref(),
        Some("task definition changed; approval denied")
    );
}

#[test]
fn removing_a_project_cleans_all_project_scoped_records_and_audits() {
    let dir = tempdir().unwrap();
    let project_path = dir.path().join("cleanup-app");
    write(
        &project_path.join("package.json"),
        r#"{"name":"cleanup-app","scripts":{"dev":"vite"}}"#,
    );
    let core = Core::open_in_memory().unwrap();
    let project = core.register_project(&project_path).unwrap();
    let detail = core.get_project(&project.id).unwrap();
    let task_id = detail.tasks[0].id.clone();
    let memory = core
        .add_memory(&project.id, "keep the local setup documented")
        .unwrap();
    let summary = repoatlas_core::ai::save_summary(
        core.connection(),
        &project.id,
        None,
        None,
        "local summary",
        "package.json",
    )
    .unwrap();
    repoatlas_core::ai::append_conversation(
        core.connection(),
        &project.id,
        vec![repoatlas_core::ChatMessage {
            role: "user".into(),
            content: "What is this?".into(),
        }],
    )
    .unwrap();
    let approval = core
        .request_task_approval(TaskSpec {
            project_id: project.id.clone(),
            task_id: Some(task_id.clone()),
            kind: "dev".into(),
            executable: "pnpm".into(),
            argv: vec!["dev".into()],
            cwd: Some(project.canonical_path.clone()),
            shell_mode: false,
        })
        .unwrap();
    core.record_audit_event("mcp", "memory", "memory", Some(&memory.id), None, "success")
        .unwrap();
    core.record_audit_event(
        "mcp",
        "summary",
        "summary",
        Some(&summary.id),
        None,
        "success",
    )
    .unwrap();
    core.record_audit_event(
        "mcp",
        "approval",
        "task_approval",
        Some(&approval.id),
        None,
        "pending",
    )
    .unwrap();
    core.record_audit_event("mcp", "task", "task", Some(&task_id), None, "success")
        .unwrap();

    let removal = core.remove_project(&project.id).unwrap();
    assert_eq!(removal.removed_memory_items, 1);
    assert_eq!(removal.removed_summaries, 1);
    assert!(removal.removed_conversation);
    assert_eq!(removal.removed_pending_approvals, 1);
    assert!(removal.removed_audit_events >= 5);
    assert!(!removal.filesystem_deleted);
    assert!(project_path.exists());
    assert!(core.list_audit_events(100).unwrap().is_empty());
}

#[test]
fn checkout_remote_urls_normalize_across_ssh_and_https_forms() {
    let ssh = repoatlas_core::git::normalize_remote_url("git@github.com:OpenAI/Repo.git");
    let https = repoatlas_core::git::normalize_remote_url("https://github.com/openai/repo.git");
    assert_eq!(ssh, https);
    assert_eq!(ssh.as_deref(), Some("https://github.com/openai/repo"));
}

#[test]
fn restart_recovers_starting_approvals_and_running_runs() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("repoatlas.sqlite");
    let project_path = dir.path().join("restart-app");
    fs::create_dir_all(&project_path).unwrap();

    let core = Core::open(&db_path).unwrap();
    let project = core.register_project(&project_path).unwrap();
    let task = core
        .add_task(
            &project.id,
            repoatlas_core::TaskDefinition {
                id: "dev".into(),
                kind: "dev".into(),
                name: "Dev".into(),
                description: None,
                executable: "node".into(),
                argv: vec!["server.js".into()],
                cwd: None,
                inferred: false,
                shell_mode: false,
            },
        )
        .unwrap();
    let approval = core
        .request_task_approval(repoatlas_core::TaskSpec {
            project_id: project.id.clone(),
            task_id: Some(task.tasks[0].id.clone()),
            kind: "dev".into(),
            executable: "node".into(),
            argv: vec!["server.js".into()],
            cwd: None,
            shell_mode: false,
        })
        .unwrap();
    let (_, _) = core.resolve_task_approval(&approval.id, true).unwrap();
    core.connection()
        .execute(
            "INSERT INTO task_runs (id, project_id, task_id, kind, executable, argv_json, cwd, shell_mode, status, log_path, started_at) VALUES ('run-restart', ?1, ?2, 'dev', 'node', '[\"server.js\"]', ?3, 0, 'running', ?4, datetime('now'))",
            rusqlite::params![
                project.id,
                task.tasks[0].id,
                project_path.to_string_lossy().to_string(),
                dir.path().join("run.log").to_string_lossy().to_string()
            ],
        )
        .unwrap();
    drop(core);

    let reopened = Core::open(&db_path).unwrap();
    assert_eq!(
        reopened.get_pending_approval(&approval.id).unwrap().status,
        "failed"
    );
    assert_eq!(
        reopened.get_task_run("run-restart").unwrap().status,
        "failed"
    );
    assert!(reopened
        .list_project_events(&project.id, 20)
        .unwrap()
        .iter()
        .any(|event| event.title.contains("interrupted")));
}

#[test]
fn removal_rejects_a_starting_task_approval() {
    let dir = tempdir().unwrap();
    let project_path = dir.path().join("approval-active");
    fs::create_dir_all(&project_path).unwrap();
    let core = Core::open_in_memory().unwrap();
    let project = core.register_project(&project_path).unwrap();
    let task = core
        .add_task(
            &project.id,
            repoatlas_core::TaskDefinition {
                id: "run".into(),
                kind: "run".into(),
                name: "Run".into(),
                description: None,
                executable: "node".into(),
                argv: vec!["index.js".into()],
                cwd: None,
                inferred: false,
                shell_mode: false,
            },
        )
        .unwrap();
    let approval = core
        .request_task_approval(repoatlas_core::TaskSpec {
            project_id: project.id.clone(),
            task_id: Some(task.tasks[0].id.clone()),
            kind: "run".into(),
            executable: "node".into(),
            argv: vec!["index.js".into()],
            cwd: None,
            shell_mode: false,
        })
        .unwrap();
    core.resolve_task_approval(&approval.id, true).unwrap();
    let error = core.remove_project(&project.id).unwrap_err();
    assert!(error.to_string().contains("starting"));
    assert_eq!(
        core.get_project(&project.id).unwrap().project.id,
        project.id
    );
}
