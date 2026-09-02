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
