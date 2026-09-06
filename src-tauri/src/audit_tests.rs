use super::*;
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

fn git(path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(path)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn git_waiting_for_a_hook_keeps_core_available_for_other_operations() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("checkout");
    fs::create_dir(&root).unwrap();
    git(&root, &["init", "-b", "main"]);
    git(&root, &["config", "user.name", "Review Fixture"]);
    git(&root, &["config", "user.email", "review@example.invalid"]);
    git(&root, &["config", "commit.gpgsign", "false"]);
    git(&root, &["config", "core.hooksPath", ".git/hooks"]);
    fs::write(root.join("file.txt"), "fixture\n").unwrap();
    git(&root, &["add", "."]);
    let core = Core::open_in_memory().unwrap();
    let project = core.register_project(&root).unwrap();
    let state = Arc::new(AppState {
        core: Mutex::new(core),
        broker: Broker::new(dir.path().join("logs")).unwrap(),
        cancel: Arc::new(AtomicBool::new(false)),
        runtime_configs: Mutex::new(HashMap::new()),
        runtime_sampler_started: AtomicBool::new(false),
        port_preflights: Mutex::new(HashMap::new()),
        file_indexes: Mutex::new(HashMap::new()),
        file_index_build_lock: Arc::new(Mutex::new(())),
        file_search_lock: Arc::new(Mutex::new(())),
        next_file_index_generation: AtomicU64::new(0),
        git_write_locks: Mutex::new(HashMap::new()),
    });
    let hook = root.join(".git/hooks/pre-commit");
    fs::write(&hook, "#!/bin/sh\nprintf started > .git/review-started\nwhile [ ! -f .git/review-release ]; do sleep 0.05; done\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let worker_state = state.clone();
    let worker = std::thread::spawn(move || {
        execute_git_operation(
            &worker_state,
            &project.id,
            GitOp::Commit {
                message: "fixture".into(),
            },
        )
    });
    let start = Instant::now();
    while !root.join(".git/review-started").exists() && start.elapsed() < Duration::from_secs(8) {
        std::thread::sleep(Duration::from_millis(20));
    }
    let reached_hook = root.join(".git/review-started").exists();
    let available = state
        .core
        .try_lock()
        .map(|core| core.dashboard_snapshot().is_ok())
        .unwrap_or(false);
    // Always release the hook before asserting, including on regressions.
    fs::write(root.join(".git/review-release"), "release").unwrap();
    let result = worker.join().unwrap().unwrap();
    assert!(reached_hook, "Git did not reach the blocking fixture hook");
    assert!(
        available,
        "Git held the global Core lock while waiting for a subprocess"
    );
    assert!(result.ok);
}
