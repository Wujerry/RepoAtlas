use repoatlas_core::{Broker, Core, TaskSpec};
use std::fs;
use std::time::{Duration, Instant};
use tempfile::tempdir;

fn write(path: &std::path::Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

#[test]
fn completed_process_reports_exit_without_waiting_for_terminal_eof() {
    let dir = tempdir().unwrap();
    write(&dir.path().join("package.json"), r#"{"name":"exit-test"}"#);
    write(
        &dir.path().join("build.py"),
        "import sys, time\nprint('BUILD_COMPLETE', flush=True)\ntime.sleep(0.8)\nprint('FINAL_OUTPUT', flush=True)\nsys.exit(int(sys.argv[1]))\n",
    );
    let core = Core::open_in_memory().unwrap();
    let project = core.register_project(dir.path()).unwrap();
    let broker = Broker::new(dir.path().join("logs")).unwrap();
    for code in [0, 7] {
        let (tx, rx) = std::sync::mpsc::channel();
        let (output_tx, output_rx) = std::sync::mpsc::channel();
        let run = broker
            .start(
                core.connection(),
                TaskSpec {
                    project_id: project.id.clone(),
                    task_id: None,
                    kind: "build".into(),
                    executable: "python".into(),
                    argv: vec!["build.py".into(), code.to_string()],
                    cwd: Some(project.canonical_path.clone()),
                    shell_mode: false,
                },
                move |chunk| {
                    let _ = output_tx.send(chunk.text);
                },
                move |id, exit| {
                    tx.send((id, exit)).unwrap();
                },
            )
            .unwrap();
        let mut output = String::new();
        while !output.contains("BUILD_COMPLETE") {
            output.push_str(&output_rx.recv_timeout(Duration::from_secs(8)).unwrap());
        }
        assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());
        assert!(broker.active().unwrap().contains(&run.id));
        let result = rx.recv_timeout(Duration::from_secs(8));
        if result.is_err() {
            let _ = broker.stop(core.connection(), &run.id);
        }
        assert_eq!(result.unwrap(), (run.id.clone(), Some(code)));
        assert!(!broker.active().unwrap().contains(&run.id));
        assert!(fs::read_to_string(&run.log_path)
            .unwrap()
            .contains("FINAL_OUTPUT"));
    }
}

#[test]
fn command_broker_exposes_a_tty_and_preserves_ansi() {
    let probe = std::process::Command::new("python")
        .arg("-c")
        .arg("print(1)")
        .output();
    if probe
        .as_ref()
        .map(|output| !output.status.success())
        .unwrap_or(true)
    {
        return;
    }

    let dir = tempdir().unwrap();
    write(
        &dir.path().join("tty-app/package.json"),
        r#"{"name":"tty-app"}"#,
    );
    write(
        &dir.path().join("tty-app/probe.py"),
        "import sys\nlabel = 'tty-yes' if sys.stdout.isatty() else 'tty-no'\nsys.stdout.write(label + chr(10))\nsys.stdout.write(chr(27) + '[32mgreen' + chr(27) + '[0m' + chr(10))\nsys.stdout.write('progress' + chr(13) + 'progress-2' + chr(10))\nsys.stdout.flush()\n",
    );

    let core = Core::open(dir.path().join("core.sqlite")).unwrap();
    let project = core.register_project(dir.path().join("tty-app")).unwrap();
    let broker = Broker::new(dir.path().join("logs")).unwrap();
    let chunks = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let observed = chunks.clone();
    let run = broker
        .start(
            core.connection(),
            TaskSpec {
                project_id: project.id.clone(),
                task_id: None,
                kind: "run".into(),
                executable: "python".into(),
                argv: vec!["probe.py".into()],
                cwd: Some(project.canonical_path.clone()),
                shell_mode: false,
            },
            move |chunk| observed.lock().unwrap().push(chunk.text),
            |_, _| {},
        )
        .unwrap();
    let _ = broker.resize(&run.id, 100, 30);

    let started = Instant::now();
    let log_path = dir.path().join("logs").join(format!("{}.log", run.id));
    while started.elapsed() < Duration::from_secs(8) {
        let mut joined = chunks.lock().unwrap().join("");
        if let Ok(file) = fs::read_to_string(&log_path) {
            joined.push_str(&file);
        }
        if joined.contains("[6n") {
            let _ = broker.write_stdin(&run.id, "[24;80R");
        }
        if !broker.active().unwrap().iter().any(|id| id == &run.id) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let mut joined = chunks.lock().unwrap().join("");
    if let Ok(file) = fs::read_to_string(&log_path) {
        joined.push_str(&file);
    }

    assert!(joined.contains("tty-yes"), "{joined:?}");
    assert!(joined.contains("green"), "{joined:?}");
    assert!(joined.contains("progress-2"), "{joined:?}");
    let _ = broker.stop(core.connection(), &run.id);
}

#[cfg(windows)]
#[test]
fn windows_script_with_spaces_runs_the_resolved_path_not_a_local_namesake() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("project");
    let script = dir.path().join("My Tools & (fixture)").join("my build.cmd");
    write(&root.join("package.json"), r#"{"name":"script-fixture"}"#);
    write(&root.join("my build.cmd"), "@echo WRONG_SCRIPT\r\n");
    write(
        &script,
        "@echo off\r\necho APPROVED_SCRIPT\r\necho [%~1]\r\necho \"[%~2]\"\r\n",
    );
    let core = Core::open_in_memory().unwrap();
    let project = core.register_project(&root).unwrap();
    let broker = Broker::new(dir.path().join("logs")).unwrap();
    let chunks = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let observed = chunks.clone();
    let run = broker
        .start(
            core.connection(),
            TaskSpec {
                project_id: project.id,
                task_id: None,
                kind: "run".into(),
                executable: script.to_string_lossy().into_owned(),
                argv: vec!["hello world".into(), "nightly | smoke".into()],
                cwd: Some(project.canonical_path),
                shell_mode: false,
            },
            move |chunk| observed.lock().unwrap().push(chunk.text),
            |_, _| {},
        )
        .unwrap();
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(8) {
        let _ = broker.write_stdin(&run.id, "\x1b[24;80R");
        if !broker.active().unwrap().contains(&run.id) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let output = chunks.lock().unwrap().join("");
    let _ = broker.stop(core.connection(), &run.id);
    assert!(output.contains("APPROVED_SCRIPT"), "{output:?}");
    assert!(output.contains("[hello world]"), "{output:?}");
    assert!(output.contains("[nightly | smoke]"), "{output:?}");
    assert!(!output.contains("WRONG_SCRIPT"), "{output:?}");
}
