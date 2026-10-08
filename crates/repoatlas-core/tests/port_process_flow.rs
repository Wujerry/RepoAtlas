#![cfg(windows)]
use repoatlas_core::{
    port_processes::{force_external, PortProcessManager, StopMode},
    Broker, Core,
};
use std::{
    fs,
    net::TcpListener,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[test]
fn listener_fixture() {
    let Ok(path) = std::env::var("REPOATLAS_TEST_LISTENER") else {
        return;
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    fs::write(&path, listener.local_addr().unwrap().port().to_string()).unwrap();
    let mut additional = None;
    loop {
        if additional.is_none() && std::path::Path::new("add-port").exists() {
            let socket = TcpListener::bind("127.0.0.1:0").unwrap();
            fs::write(
                "extra-port",
                socket.local_addr().unwrap().port().to_string(),
            )
            .unwrap();
            additional = Some(socket);
        }
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn shared_preflight_includes_ipv6_listeners() {
    let listener = TcpListener::bind("[::1]:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let data = tempfile::tempdir().unwrap();
    let broker = Broker::new(data.path().join("logs")).unwrap();
    let conflicts = broker.preflight_ports(&[port]).unwrap();
    assert!(conflicts
        .iter()
        .any(|c| c.port == port && c.pid == Some(std::process::id())));
}

struct Fixture {
    child: Child,
    dir: tempfile::TempDir,
    port: u16,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let ready = dir.path().join("port");
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", "listener_fixture", "--nocapture"])
            .env("REPOATLAS_TEST_LISTENER", &ready)
            .current_dir(dir.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
        let child = command.spawn().unwrap();
        let mut fixture = Self {
            child,
            dir,
            port: 0,
        };
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(10) {
            if let Ok(value) = fs::read_to_string(&ready) {
                if let Ok(port) = value.parse() {
                    fixture.port = port;
                    return fixture;
                }
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("listener fixture did not start");
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn listener_path_is_readable_without_synchronize_or_terminate_access() {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Security::{
        AddAccessAllowedAce, CreateWellKnownSid, InitializeAcl, InitializeSecurityDescriptor,
        SetKernelObjectSecurity, SetSecurityDescriptorDacl, WinWorldSid, ACL_REVISION,
        DACL_SECURITY_INFORMATION, SECURITY_DESCRIPTOR,
    };
    use windows_sys::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION;

    let mut fixture = Fixture::new();
    // Restrict only our disposable child. Its retained creation handle still permits cleanup.
    let mut sid = [0u64; 16];
    let mut sid_size = std::mem::size_of_val(&sid) as u32;
    let mut acl = [0u64; 32];
    let mut descriptor: SECURITY_DESCRIPTOR = unsafe { std::mem::zeroed() };
    unsafe {
        assert_ne!(
            CreateWellKnownSid(
                WinWorldSid,
                std::ptr::null_mut(),
                sid.as_mut_ptr().cast(),
                &mut sid_size
            ),
            0
        );
        assert_ne!(
            InitializeAcl(
                acl.as_mut_ptr().cast(),
                std::mem::size_of_val(&acl) as u32,
                ACL_REVISION
            ),
            0
        );
        assert_ne!(
            AddAccessAllowedAce(
                acl.as_mut_ptr().cast(),
                ACL_REVISION,
                PROCESS_QUERY_LIMITED_INFORMATION,
                sid.as_mut_ptr().cast()
            ),
            0
        );
        let descriptor = (&mut descriptor as *mut SECURITY_DESCRIPTOR).cast();
        assert_ne!(InitializeSecurityDescriptor(descriptor, 1), 0);
        assert_ne!(
            SetSecurityDescriptorDacl(descriptor, 1, acl.as_ptr().cast(), 0),
            0
        );
        assert_ne!(
            SetKernelObjectSecurity(
                fixture.child.as_raw_handle(),
                DACL_SECURITY_INFORMATION,
                descriptor
            ),
            0
        );
    }
    let data = tempfile::tempdir().unwrap();
    let broker = Broker::new(data.path().join("logs")).unwrap();
    let inventory = PortProcessManager::default()
        .inventory(&broker, &[])
        .unwrap();
    let row = inventory
        .processes
        .iter()
        .find(|p| p.pid == fixture.child.id())
        .unwrap();
    let identity = row
        .identity
        .as_ref()
        .expect("query-only access must reveal the path");
    assert!(!identity.executable.is_empty());
    assert_eq!(row.ports, vec![fixture.port]);
    assert_eq!(
        force_external(identity).unwrap_err().to_string(),
        "permission_denied"
    );
    assert!(fixture.child.try_wait().unwrap().is_none());
}

#[test]
fn external_listener_attribution_confirmation_and_real_termination() {
    let mut fixture = Fixture::new();
    let data = tempfile::tempdir().unwrap();
    let core = Core::open(data.path().join("test.sqlite")).unwrap();
    let broker = Broker::new(data.path().join("logs")).unwrap();
    let mut manager = PortProcessManager::default();
    let inventory = manager.inventory(&broker, &[]).unwrap();
    let row = inventory
        .processes
        .iter()
        .find(|p| p.pid == fixture.child.id())
        .unwrap();
    assert_eq!(row.ports, vec![fixture.port]);
    assert_eq!(row.project_id, None);
    assert_eq!(row.attribution, "unknown");
    let identity = row.identity.clone().unwrap();
    assert_eq!(
        manager
            .preview(&broker, &[], &identity, StopMode::Graceful)
            .unwrap_err()
            .to_string(),
        "graceful_unavailable"
    );
    let project = core
        .register_project(fixture.dir.path().to_str().unwrap())
        .unwrap();
    let projects = core.list_projects(Default::default()).unwrap();
    let auto = manager.inventory(&broker, &projects).unwrap();
    let auto = auto
        .processes
        .iter()
        .find(|p| p.pid == fixture.child.id())
        .unwrap();
    assert_eq!(auto.project_id.as_deref(), Some(project.id.as_str()));
    assert_eq!(auto.attribution, "cwd");
    manager
        .associate(identity.clone(), Some(project.id.clone()), &projects)
        .unwrap();
    let inventory = manager.inventory(&broker, &projects).unwrap();
    let row = inventory
        .processes
        .iter()
        .find(|p| p.pid == fixture.child.id())
        .unwrap();
    assert_eq!(row.project_id.as_deref(), Some(project.id.as_str()));
    assert_eq!(row.attribution, "manual");
    let mut reused = identity.clone();
    reused.created_at.push('1');
    assert_eq!(
        force_external(&reused).unwrap_err().to_string(),
        "identity_changed"
    );
    assert!(fixture.child.try_wait().unwrap().is_none());
    let preview = manager
        .preview(&broker, &projects, &identity, StopMode::Force)
        .unwrap();
    assert_eq!(preview.process_count, 1);
    assert_eq!(preview.affected.len(), 1);
    let confirmed = manager.consume(&broker, &projects, &preview.token).unwrap();
    assert!(manager.consume(&broker, &projects, &preview.token).is_err());
    force_external(confirmed.target.identity.as_ref().unwrap()).unwrap();
    assert!(fixture.child.try_wait().unwrap().is_some());
    assert!(broker.preflight_ports(&[fixture.port]).unwrap().is_empty());
    assert!(manager
        .preview(&broker, &projects, &identity, StopMode::Force)
        .is_err());
}

#[test]
fn changed_listening_ports_require_a_new_confirmation() {
    let fixture = Fixture::new();
    let data = tempfile::tempdir().unwrap();
    let broker = Broker::new(data.path().join("logs")).unwrap();
    let mut manager = PortProcessManager::default();
    let inventory = manager.inventory(&broker, &[]).unwrap();
    let identity = inventory
        .processes
        .iter()
        .find(|p| p.pid == fixture.child.id())
        .unwrap()
        .identity
        .as_ref()
        .unwrap();
    let preview = manager
        .preview(&broker, &[], identity, StopMode::Force)
        .unwrap();
    fs::write(fixture.dir.path().join("add-port"), "").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !fixture.dir.path().join("extra-port").exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(fixture.dir.path().join("extra-port").exists());
    assert_eq!(
        manager
            .consume(&broker, &[], &preview.token)
            .unwrap_err()
            .to_string(),
        "scope_changed"
    );
    assert!(TcpListener::bind(("127.0.0.1", fixture.port)).is_err());
}

#[test]
fn process_exits_between_preview_and_confirmation_without_touching_a_replacement() {
    let mut fixture = Fixture::new();
    let data = tempfile::tempdir().unwrap();
    let broker = Broker::new(data.path().join("logs")).unwrap();
    let mut manager = PortProcessManager::default();
    let inventory = manager.inventory(&broker, &[]).unwrap();
    let identity = inventory
        .processes
        .iter()
        .find(|p| p.pid == fixture.child.id())
        .unwrap()
        .identity
        .as_ref()
        .unwrap();
    let preview = manager
        .preview(&broker, &[], identity, StopMode::Force)
        .unwrap();
    fixture.child.kill().unwrap();
    fixture.child.wait().unwrap();
    let replacement = Fixture::new();
    assert!(manager.consume(&broker, &[], &preview.token).is_err());
    assert!(TcpListener::bind(("127.0.0.1", replacement.port)).is_err());
}

#[test]
fn managed_listener_uses_job_attribution_and_normal_stop_never_escalates() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let data = tempfile::tempdir().unwrap();
    fs::write(
        data.path().join("server.cjs"),
        r#"
const fs = require('node:fs');
const server = require('node:net').createServer();
process.on('SIGINT', () => fs.writeFileSync('interrupted', 'yes'));
server.listen(0, '127.0.0.1', () => fs.writeFileSync('ready', String(server.address().port)));
"#,
    )
    .unwrap();
    let core = Core::open(data.path().join("core.sqlite")).unwrap();
    let project = core.register_project(data.path()).unwrap();
    let broker = Broker::new(data.path().join("logs")).unwrap();
    let run = broker
        .start(
            core.connection(),
            repoatlas_core::TaskSpec {
                project_id: project.id.clone(),
                task_id: None,
                kind: "dev".into(),
                executable: "node".into(),
                argv: vec!["server.cjs".into()],
                cwd: Some(project.canonical_path.clone()),
                shell_mode: false,
            },
            |_| {},
            |_, _| {},
        )
        .unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let deadline = Instant::now() + Duration::from_secs(8);
        while !data.path().join("ready").exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        let port: u16 = fs::read_to_string(data.path().join("ready"))
            .unwrap()
            .parse()
            .unwrap();
        let projects = vec![project];
        let mut manager = PortProcessManager::default();
        let inventory = manager.inventory(&broker, &projects).unwrap();
        let target = inventory
            .processes
            .iter()
            .find(|p| p.ports.contains(&port))
            .unwrap();
        assert_eq!(target.run_id.as_deref(), Some(run.id.as_str()));
        assert_eq!(target.attribution, "task");
        let preview = manager
            .preview(
                &broker,
                &projects,
                target.identity.as_ref().unwrap(),
                StopMode::Graceful,
            )
            .unwrap();
        assert!(preview.process_count >= 1);
        manager.consume(&broker, &projects, &preview.token).unwrap();
        broker.request_normal_stop(&run.id).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !data.path().join("interrupted").exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert!(
            data.path().join("interrupted").exists(),
            "normal stop did not reach the managed PTY"
        );
        thread::sleep(Duration::from_millis(250));
        assert!(broker.active().unwrap().contains(&run.id));
        assert!(!broker.preflight_ports(&[port]).unwrap().is_empty());
        assert!(broker.take_stop_request(&run.id));
    }));
    broker.stop(core.connection(), &run.id).unwrap();
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}
