use repoatlas_core::{Core, ProjectQuery};
use std::{fs, path::Path, sync::atomic::AtomicBool};
use tempfile::tempdir;

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

#[test]
fn mixed_modules_have_scoped_tasks_and_promotion_survives_rescan() {
    let dir = tempdir().unwrap();
    write(
        dir.path(),
        "package.json",
        r#"{"name":"system","scripts":{"dev":"root"}}"#,
    );
    for name in ["web", "admin"] {
        write(
            dir.path(),
            &format!("{name}/package.json"),
            &format!(r#"{{"name":"{name}","scripts":{{"dev":"vite"}}}}"#),
        );
    }
    write(
        dir.path(),
        "server/pom.xml",
        "<project><artifactId>server</artifactId></project>",
    );
    write(dir.path(), "node_modules/ignored/package.json", "{}");
    let core = Core::open_in_memory().unwrap();
    let root = core.add_scan_root(dir.path()).unwrap();
    core.scan_root(&root.id, &AtomicBool::new(false), &|_| {})
        .unwrap();
    let projects = core.list_projects(ProjectQuery::default()).unwrap();
    assert_eq!(projects.len(), 1);
    let project = &projects[0];
    let detail = core.get_project(&project.id).unwrap();
    assert_eq!(detail.modules.len(), 3);
    assert!(detail.project.languages.contains(&"Java".to_string()));
    let web = detail
        .modules
        .iter()
        .find(|m| m.relative_path == "web")
        .unwrap();
    let admin = detail
        .modules
        .iter()
        .find(|m| m.relative_path == "admin")
        .unwrap();
    assert_ne!(web.task_ids, admin.task_ids);
    for module in [web, admin] {
        assert!(!module.task_ids.is_empty());
        for id in &module.task_ids {
            let (_, task) = core.task_for_start(&project.id, id).unwrap();
            assert_eq!(task.cwd.as_deref(), Some(module.canonical_path.as_str()));
        }
    }
    let original_ids = web.task_ids.clone();
    core.scan_root(&root.id, &AtomicBool::new(false), &|_| {})
        .unwrap();
    assert_eq!(
        core.list_modules(&project.id)
            .unwrap()
            .iter()
            .find(|m| m.id == web.id)
            .unwrap()
            .task_ids,
        original_ids
    );
    let promoted = core
        .promote_module_with_origin(&project.id, &web.id, "test")
        .unwrap();
    assert!(core
        .get_project(&project.id)
        .unwrap()
        .tasks
        .iter()
        .all(|t| !original_ids.contains(&t.id)));
    core.scan_root(&root.id, &AtomicBool::new(false), &|_| {})
        .unwrap();
    assert_eq!(
        core.list_projects(ProjectQuery::default()).unwrap().len(),
        2
    );
    assert_eq!(
        core.list_modules(&project.id)
            .unwrap()
            .iter()
            .find(|m| m.id == web.id)
            .unwrap()
            .project_id
            .as_deref(),
        Some(promoted.id.as_str())
    );
    assert_eq!(
        core.get_project(&promoted.id).unwrap().tasks.len(),
        original_ids.len()
    );
    assert!(dir.path().join("web/package.json").exists());
}

#[test]
fn explicit_grouping_promotes_modules_and_is_persistent_and_reversible() {
    let dir = tempdir().unwrap();
    write(dir.path(), "package.json", "{}");
    write(dir.path(), "web/package.json", "{}");
    write(dir.path(), "server/pom.xml", "<project/>");
    let core = Core::open_in_memory().unwrap();
    let project = core.register_project(dir.path()).unwrap();
    core.set_directory_group_with_origin(&project.id, true, "test")
        .unwrap();
    core.refresh_project(&project.id).unwrap();
    assert!(
        core.get_project(&project.id)
            .unwrap()
            .project
            .directory_group
    );
    assert_eq!(
        core.list_projects(ProjectQuery::default()).unwrap().len(),
        3
    );
    write(dir.path(), "later/package.json", "{}");
    core.refresh_project(&project.id).unwrap();
    assert_eq!(
        core.list_projects(ProjectQuery::default()).unwrap().len(),
        4
    );
    core.set_directory_group_with_origin(&project.id, false, "test")
        .unwrap();
    assert!(
        !core
            .get_project(&project.id)
            .unwrap()
            .project
            .directory_group
    );
    assert_eq!(
        core.list_projects(ProjectQuery::default()).unwrap().len(),
        4
    );
}

#[test]
fn nested_checkout_remains_independent_and_missing_module_is_retained() {
    let dir = tempdir().unwrap();
    write(dir.path(), "package.json", "{}");
    write(dir.path(), "independent/package.json", "{}");
    fs::create_dir(dir.path().join("independent/.git")).unwrap();
    write(dir.path(), "module/package.json", "{}");
    let core = Core::open_in_memory().unwrap();
    let project = core.register_project(dir.path()).unwrap();
    assert_eq!(
        core.list_projects(ProjectQuery::default()).unwrap().len(),
        2
    );
    fs::rename(dir.path().join("module"), dir.path().join("renamed")).unwrap();
    core.refresh_project(&project.id).unwrap();
    let modules = core.list_modules(&project.id).unwrap();
    assert_eq!(
        modules
            .iter()
            .find(|m| m.relative_path == "module")
            .unwrap()
            .availability,
        "unavailable"
    );
    assert!(core
        .resolve_module_path(
            &project.id,
            &modules
                .iter()
                .find(|m| m.relative_path == "module")
                .unwrap()
                .id
        )
        .is_err());
}

#[test]
fn explicit_membership_and_nested_promotion_reassign_tasks_without_duplicates() {
    let dir = tempdir().unwrap();
    write(
        dir.path(),
        "package.json",
        r#"{"workspaces":["packages/*"]}"#,
    );
    write(
        dir.path(),
        "packages/web/package.json",
        r#"{"name":"web","scripts":{"dev":"vite"}}"#,
    );
    write(
        dir.path(),
        "packages/web/inner/package.json",
        r#"{"name":"inner","scripts":{"dev":"vite"}}"#,
    );
    let core = Core::open_in_memory().unwrap();
    let project = core.register_project(dir.path()).unwrap();
    let modules = core.list_modules(&project.id).unwrap();
    let web = modules
        .iter()
        .find(|m| m.relative_path == "packages/web")
        .unwrap();
    assert_eq!(web.evidence, "package.json#workspaces");
    let promoted = core
        .promote_module_with_origin(&project.id, &web.id, "test")
        .unwrap();
    assert_eq!(core.list_modules(&project.id).unwrap().len(), 1);
    assert_eq!(core.list_modules(&promoted.id).unwrap().len(), 1);
    assert!(core.get_project(&project.id).unwrap().tasks.is_empty());
    core.refresh_project(&project.id).unwrap();
    assert_eq!(core.get_project(&promoted.id).unwrap().tasks.len(), 2);
}

#[test]
fn grouping_rolls_back_on_audit_failure_and_exports_preferences() {
    let dir = tempdir().unwrap();
    write(dir.path(), "package.json", "{}");
    write(
        dir.path(),
        "web/package.json",
        r#"{"scripts":{"dev":"vite"}}"#,
    );
    let core = Core::open_in_memory().unwrap();
    let project = core.register_project(dir.path()).unwrap();
    core.connection().execute_batch("CREATE TRIGGER fail_group BEFORE INSERT ON audit_events WHEN NEW.action = 'set_directory_group' BEGIN SELECT RAISE(ABORT, 'test audit failure'); END;").unwrap();
    assert!(core
        .set_directory_group_with_origin(&project.id, true, "test")
        .is_err());
    assert_eq!(
        core.list_projects(ProjectQuery::default()).unwrap().len(),
        1
    );
    assert!(
        !core
            .get_project(&project.id)
            .unwrap()
            .project
            .directory_group
    );
    assert!(!core.get_project(&project.id).unwrap().tasks.is_empty());
    core.connection()
        .execute_batch("DROP TRIGGER fail_group")
        .unwrap();
    core.set_directory_group_with_origin(&project.id, true, "test")
        .unwrap();
    let imported = Core::open_in_memory().unwrap();
    imported.import_json(&core.export_json().unwrap()).unwrap();
    assert!(imported
        .list_projects(ProjectQuery::default())
        .unwrap()
        .iter()
        .any(|p| p.directory_group));
}

#[test]
fn direct_registration_of_module_migrates_custom_tasks() {
    let dir = tempdir().unwrap();
    write(dir.path(), "package.json", "{}");
    write(dir.path(), "web/package.json", "{}");
    let core = Core::open_in_memory().unwrap();
    let project = core.register_project(dir.path()).unwrap();
    let module = core.list_modules(&project.id).unwrap().remove(0);
    core.add_task(
        &project.id,
        repoatlas_core::TaskDefinition {
            id: "custom".into(),
            kind: "test".into(),
            name: "Custom module check".into(),
            description: None,
            executable: "node".into(),
            argv: vec!["--version".into()],
            cwd: Some(module.canonical_path),
            inferred: false,
            shell_mode: false,
            expected_ports: vec![],
            dev_url_path: None,
            dev_url_scheme: None,
        },
    )
    .unwrap();
    let child = core
        .register_project_with_origin(dir.path().join("web"), "mcp")
        .unwrap();
    assert!(core.get_project(&project.id).unwrap().tasks.is_empty());
    assert_eq!(core.get_project(&child.id).unwrap().tasks[0].id, "custom");
}

#[test]
fn grouping_nested_modules_keeps_new_project_ownership() {
    let dir = tempdir().unwrap();
    write(dir.path(), "package.json", "{}");
    write(dir.path(), "web/package.json", "{}");
    write(dir.path(), "web/inner/package.json", "{}");
    let core = Core::open_in_memory().unwrap();
    let parent = core.register_project(dir.path()).unwrap();
    core.set_directory_group_with_origin(&parent.id, true, "test")
        .unwrap();
    let projects = core.list_projects(ProjectQuery::default()).unwrap();
    assert_eq!(projects.len(), 2);
    let web = projects.iter().find(|p| p.id != parent.id).unwrap();
    assert_eq!(
        core.list_modules(&web.id).unwrap()[0].relative_path,
        "inner"
    );
}

#[test]
fn narrower_scan_root_does_not_implicitly_promote_an_existing_module() {
    let dir = tempdir().unwrap();
    write(dir.path(), "package.json", "{}");
    write(dir.path(), "web/package.json", "{}");
    let core = Core::open_in_memory().unwrap();
    let parent = core.register_project(dir.path()).unwrap();
    let root = core.add_scan_root(dir.path().join("web")).unwrap();
    core.scan_root(&root.id, &AtomicBool::new(false), &|_| {})
        .unwrap();
    assert_eq!(
        core.list_projects(ProjectQuery::default()).unwrap().len(),
        1
    );
    assert!(core.list_modules(&parent.id).unwrap()[0]
        .project_id
        .is_none());
}

#[test]
fn discovery_and_launch_reject_replaced_module_links() {
    let dir = tempdir().unwrap();
    let external = tempdir().unwrap();
    write(dir.path(), "package.json", "{}");
    write(dir.path(), "web/package.json", "{}");
    write(external.path(), "package.json", r#"{"name":"outside"}"#);
    let core = Core::open_in_memory().unwrap();
    let project = core.register_project(dir.path()).unwrap();
    let module = core.list_modules(&project.id).unwrap().remove(0);
    fs::rename(dir.path().join("web"), dir.path().join("old-web")).unwrap();
    #[cfg(windows)]
    let linked = std::os::windows::fs::symlink_dir(external.path(), dir.path().join("web"));
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(external.path(), dir.path().join("web"));
    if let Err(error) = linked {
        eprintln!("symlink permission unavailable: {error}");
        return;
    }
    assert!(core.resolve_module_path(&project.id, &module.id).is_err());
    assert!(core
        .promote_module_with_origin(&project.id, &module.id, "test")
        .is_err());
    core.refresh_project(&project.id).unwrap();
    assert!(core
        .list_modules(&project.id)
        .unwrap()
        .iter()
        .all(|m| m.name != "outside"));
}
