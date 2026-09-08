use repoatlas_core::{Core, GitOp, ProjectPatch, TaskSpec};
use rusqlite::params;
use std::{fs, path::Path, process::Command, time::Instant};
use tempfile::tempdir;

fn git(path: &Path, args: &[&str]) {
    let result = Command::new("git")
        .current_dir(path)
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "git failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn init(path: &Path) {
    fs::create_dir_all(path).unwrap();
    git(path, &["init", "-b", "main"]);
    git(path, &["config", "user.email", "review@example.invalid"]);
    git(path, &["config", "user.name", "Review Fixture"]);
    git(path, &["config", "commit.gpgsign", "false"]);
}

#[test]
fn completed_git_retains_its_outcome_and_audit_after_record_relocation_or_removal() {
    for remove in [false, true] {
        let dir = tempdir().unwrap();
        let root = dir.path().join("original");
        let replacement = dir.path().join("replacement");
        init(&root);
        init(&replacement);
        fs::write(root.join("file.txt"), "fixture\n").unwrap();
        let core = Core::open_in_memory().unwrap();
        let project = core.register_project(&root).unwrap();
        let operation = core
            .prepare_git_operation(
                &project.id,
                GitOp::Stage {
                    paths: vec!["file.txt".into()],
                },
            )
            .unwrap();
        let outcome = operation.execute();
        if remove {
            core.remove_project(&project.id).unwrap();
        } else {
            core.relocate_project(&project.id, &replacement).unwrap();
        }
        let result = core.finish_git_operation(&operation, outcome).unwrap();
        assert!(result.ok);
        assert!(result.stdout.contains("cached state was not updated"));
        let audited: i64 = core.connection().query_row(
            "SELECT COUNT(*) FROM audit_events WHERE target_id=?1 AND action='git_stage' AND outcome='success'",
            [&project.id], |row| row.get(0),
        ).unwrap();
        assert_eq!(audited, 1);
        if !remove {
            assert_eq!(
                core.get_project(&project.id).unwrap().git.unwrap().dirty,
                Some(false)
            );
        }
    }
}

#[test]
fn remote_credentials_are_removed_on_detection_legacy_reads_backup_and_upgrade() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("project");
    init(&path);
    let remote = "https://fixture:FAKE_REVIEW_TOKEN@example.invalid/team/project.git?token=FAKE_QUERY#FAKE_FRAGMENT";
    git(&path, &["remote", "add", "origin", remote]);
    let db = dir.path().join("library.sqlite");
    let core = Core::open(&db).unwrap();
    let project = core.register_project(&path).unwrap();
    let detail = core.get_project(&project.id).unwrap();
    let encoded = serde_json::to_string(&detail).unwrap();
    assert!(!encoded.contains("FAKE_"));
    assert!(encoded.contains("https://example.invalid/team/project.git"));
    let facts: String = core
        .connection()
        .query_row(
            "SELECT facts_json FROM projects WHERE id=?1",
            [&project.id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!facts.contains("FAKE_"));

    // Simulate credentials persisted by a pre-fix process, including a process
    // sharing the database after the current desktop has already migrated it.
    let legacy = serde_json::json!([{"kind":"lineage", "value":remote, "confidence":0.9, "source":"git remote origin"}]).to_string();
    core.connection()
        .execute(
            "UPDATE projects SET facts_json=?1, lineage_key=?2 WHERE id=?3",
            params![legacy, remote, project.id],
        )
        .unwrap();
    assert!(
        !serde_json::to_string(&core.get_project(&project.id).unwrap())
            .unwrap()
            .contains("FAKE_")
    );
    assert!(!core
        .atlas_report(&project.id)
        .unwrap()
        .markdown
        .contains("FAKE_"));
    let backup = dir.path().join("backup.sqlite");
    core.backup_db(&backup).unwrap();
    assert!(!String::from_utf8_lossy(&fs::read(&backup).unwrap()).contains("FAKE_"));

    core.connection()
        .execute("DELETE FROM schema_migrations WHERE version=16", [])
        .unwrap();
    drop(core);
    let reopened = Core::open(&db).unwrap();
    let (facts, key): (String, String) = reopened
        .connection()
        .query_row(
            "SELECT facts_json, lineage_key FROM projects WHERE id=?1",
            [&project.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(!facts.contains("FAKE_"));
    assert_eq!(key, "https://example.invalid/team/project");
}

#[test]
fn nested_project_diff_stage_and_mixed_status_use_checkout_relative_paths() {
    let dir = tempdir().unwrap();
    init(dir.path());
    let nested = dir.path().join("packages/app");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("file.txt"), "initial\n").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-m", "initial"]);
    let core = Core::open_in_memory().unwrap();
    let project = core.register_project(&nested).unwrap();
    let file = "packages/app/file.txt";
    fs::write(nested.join("file.txt"), "staged\n").unwrap();
    assert!(core
        .git_diff(&project.id, Some(file), false)
        .unwrap()
        .patch
        .contains("+staged"));
    assert!(
        core.git_execute(
            &project.id,
            GitOp::Stage {
                paths: vec![file.into()]
            }
        )
        .unwrap()
        .ok
    );
    fs::write(nested.join("file.txt"), "working\n").unwrap();
    let status = core.git_status(&project.id).unwrap();
    let files: Vec<_> = status
        .files
        .iter()
        .filter(|item| item.path == file)
        .collect();
    assert_eq!(files.len(), 2);
    assert!(files.iter().any(|item| item.staged));
    assert!(files.iter().any(|item| !item.staged));
    assert!(core
        .git_diff(&project.id, Some(file), true)
        .unwrap()
        .patch
        .contains("+staged"));
    assert!(core
        .git_diff(&project.id, Some(file), false)
        .unwrap()
        .patch
        .contains("+working"));
    assert!(
        core.git_execute(
            &project.id,
            GitOp::Stage {
                paths: vec![file.into()]
            }
        )
        .unwrap()
        .ok
    );
    assert!(core
        .git_diff(&project.id, Some(file), true)
        .unwrap()
        .patch
        .contains("+working"));
    assert!(core
        .git_diff(&project.id, Some(file), false)
        .unwrap()
        .patch
        .is_empty());
}

#[test]
fn expired_approvals_are_hidden_and_cannot_start_even_without_listing_first() {
    let dir = tempdir().unwrap();
    let core = Core::open_in_memory().unwrap();
    let project = core.register_project(dir.path()).unwrap();
    let request = || {
        core.request_task_approval(TaskSpec {
            project_id: project.id.clone(),
            task_id: None,
            kind: "test".into(),
            executable: "fixture".into(),
            argv: vec![],
            cwd: None,
            shell_mode: false,
        })
        .unwrap()
    };
    let expired = request();
    let fresh = request();
    core.connection()
        .execute(
            "UPDATE pending_approvals SET created_at='2000-01-01T00:00:00Z' WHERE id=?1",
            [&expired.id],
        )
        .unwrap();
    assert!(core
        .resolve_task_approval(&expired.id, true)
        .unwrap_err()
        .to_string()
        .contains("expired"));
    assert_eq!(
        core.get_pending_approval(&expired.id).unwrap().status,
        "expired"
    );
    assert_eq!(
        core.list_pending_approvals()
            .unwrap()
            .iter()
            .map(|a| &a.id)
            .collect::<Vec<_>>(),
        vec![&fresh.id]
    );
    assert!(core.list_active_task_runs().unwrap().is_empty());
    assert!(core
        .list_audit_events(100)
        .unwrap()
        .iter()
        .any(|event| event.action == "expire_task_approval"));
    assert!(core
        .resolve_task_approval(&fresh.id, true)
        .unwrap()
        .1
        .is_some());

    let old_unresolved = request();
    core.connection()
        .execute(
            "UPDATE pending_approvals SET created_at='2000-01-01T00:00:00Z' WHERE id=?1",
            [&old_unresolved.id],
        )
        .unwrap();
    let writes = core.connection().total_changes();
    assert!(core.list_pending_approvals().unwrap().is_empty());
    assert_eq!(
        core.get_pending_approval(&old_unresolved.id)
            .unwrap()
            .status,
        "expired"
    );
    core.dashboard_snapshot().unwrap();
    assert_eq!(
        core.connection().total_changes(),
        writes,
        "read-only views must not mutate expired approvals"
    );
}

#[test]
fn database_version_detects_external_metadata_updates_and_deletions() {
    let dir = tempdir().unwrap();
    let db = dir.path().join("library.sqlite");
    let desktop = Core::open(&db).unwrap();
    let agent = Core::open_without_recovery(&db).unwrap();
    let initial = desktop.data_version().unwrap();
    let path = dir.path().join("project");
    fs::create_dir(&path).unwrap();
    let project = agent.register_project(&path).unwrap();
    assert_ne!(initial, desktop.data_version().unwrap());
    let version = desktop.data_version().unwrap();
    agent
        .update_project(
            &project.id,
            ProjectPatch {
                description: Some(Some("Updated by Agent".into())),
                ..Default::default()
            },
        )
        .unwrap();
    assert_ne!(version, desktop.data_version().unwrap());
    assert_eq!(
        desktop
            .get_project(&project.id)
            .unwrap()
            .project
            .description
            .as_deref(),
        Some("Updated by Agent")
    );
    let version = desktop.data_version().unwrap();
    agent.remove_project(&project.id).unwrap();
    assert_ne!(version, desktop.data_version().unwrap());
    assert!(path.is_dir());
}

#[test]
fn dashboard_latest_runs_remain_fast_with_long_history_and_timestamp_ties() {
    let dir = tempdir().unwrap();
    let core = Core::open_in_memory().unwrap();
    let p = core.register_project(dir.path()).unwrap();
    let second = dir.path().join("second");
    fs::create_dir(&second).unwrap();
    let q = core.register_project(&second).unwrap();
    core.mark_opened(&p.id).unwrap();
    core.mark_opened(&q.id).unwrap();
    let tx = core.connection().unchecked_transaction().unwrap();
    for index in 0..5000 {
        tx.execute("INSERT INTO task_runs(id,project_id,kind,executable,argv_json,cwd,status,log_path,started_at,finished_at)
            VALUES(?1,?2,'test','fixture','[]',?3,'succeeded','fixture.log',?4,?4)",
            params![format!("run-{index:05}"), p.id, p.canonical_path, if index % 2 == 0 { "2026-09-06T00:00:00Z" } else { "2026-09-06 00:00:00" }]).unwrap();
    }
    tx.commit().unwrap();
    let start = Instant::now();
    let snapshot = core.dashboard_snapshot().unwrap();
    let elapsed = start.elapsed();
    assert_eq!(snapshot.recent_projects.len(), 2);
    let latest = snapshot
        .recent_projects
        .iter()
        .find(|item| item.project.id == p.id)
        .unwrap()
        .latest_run
        .as_ref()
        .unwrap();
    assert_eq!(latest.started_at, "2026-09-06 00:00:00");
    eprintln!("Dashboard with 5,000 Task Runs: {elapsed:?}");
    if !cfg!(debug_assertions) {
        assert!(elapsed < std::time::Duration::from_secs(1), "{elapsed:?}");
    }
}

#[test]
fn literal_git_paths_and_rename_unstage_preserve_other_files() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("project");
    init(&root);
    for name in ["a[1].txt", "a1.txt", "old.txt"] {
        fs::write(root.join(name), "original\n").unwrap();
    }
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "fixture"]);
    let core = Core::open_in_memory().unwrap();
    let project = core.register_project(&root).unwrap();
    for name in ["a[1].txt", "a1.txt"] {
        fs::write(root.join(name), format!("changed {name}\n")).unwrap();
    }
    let execute = |op| {
        let operation = core.prepare_git_operation(&project.id, op).unwrap();
        let result = core
            .finish_git_operation(&operation, operation.execute())
            .unwrap();
        assert!(result.ok, "{}", result.stderr);
    };
    let diff = core.git_diff(&project.id, Some("a[1].txt"), false).unwrap();
    assert!(diff.patch.contains("changed a[1].txt"));
    assert!(!diff.patch.contains("changed a1.txt"));
    execute(GitOp::Stage {
        paths: vec!["a[1].txt".into()],
    });
    let status = core.git_status(&project.id).unwrap();
    assert_eq!(status.files.iter().filter(|f| f.staged).count(), 1);
    execute(GitOp::Stage {
        paths: vec!["a1.txt".into()],
    });
    execute(GitOp::Unstage {
        paths: vec!["a[1].txt".into()],
    });
    let status = core.git_status(&project.id).unwrap();
    assert_eq!(
        status
            .files
            .iter()
            .filter(|f| f.staged)
            .map(|f| f.path.as_str())
            .collect::<Vec<_>>(),
        vec!["a1.txt"]
    );
    git(&root, &["mv", "old.txt", "new.txt"]);
    let status = core.git_status(&project.id).unwrap();
    assert_eq!(
        status
            .files
            .iter()
            .find(|f| f.path == "new.txt")
            .unwrap()
            .original_path
            .as_deref(),
        Some("old.txt")
    );
    execute(GitOp::Unstage {
        paths: vec!["new.txt".into()],
    });
    let status = core.git_status(&project.id).unwrap();
    assert_eq!(
        status
            .files
            .iter()
            .filter(|f| f.staged)
            .map(|f| f.path.as_str())
            .collect::<Vec<_>>(),
        vec!["a1.txt"]
    );
    assert!(root.join("new.txt").exists());
}
