//! Desktop-only Windows listener inventory and identity-bound process controls.
//! No command lines, environment values, or process observations are persisted.
use crate::{broker, Broker, Error, ProjectSummary, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessIdentity {
    pub pid: u32,
    // Decimal FILETIME string: JavaScript numbers cannot preserve all 64 bits.
    pub created_at: String,
    pub executable: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortProcess {
    pub pid: u32,
    pub name: String,
    pub ports: Vec<u16>,
    pub identity: Option<ProcessIdentity>,
    pub project_id: Option<String>,
    pub attribution: String,
    pub run_id: Option<String>,
    pub restriction: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInventory {
    pub supported: bool,
    pub processes: Vec<PortProcess>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopMode {
    Graceful,
    Force,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StopPreview {
    pub token: String,
    pub target: PortProcess,
    pub mode: StopMode,
    pub affected: Vec<PortProcess>,
    pub process_count: usize,
}

struct PendingStop {
    preview: StopPreview,
    created: Instant,
}

#[derive(Default)]
pub struct PortProcessManager {
    associations: HashMap<ProcessIdentity, String>,
    pending: HashMap<String, PendingStop>,
}

fn failure(code: &str) -> Error {
    Error::msg(code)
}

impl PortProcessManager {
    pub fn inventory(
        &mut self,
        broker: &Broker,
        projects: &[ProjectSummary],
    ) -> Result<ProcessInventory> {
        if !cfg!(windows) {
            return Ok(ProcessInventory {
                supported: false,
                processes: vec![],
            });
        }
        let listeners =
            broker::listening_ports().ok_or_else(|| failure("inspection_unavailable"))?;
        let mut by_pid = BTreeMap::<u32, Vec<u16>>::new();
        for (port, pid) in listeners {
            by_pid.entry(pid).or_default().push(port);
        }
        // Fence metadata reads with exact creation timestamps; sysinfo start_time has only seconds.
        let before: HashMap<_, _> = by_pid
            .keys()
            .map(|pid| (*pid, native::inspect(*pid)))
            .collect();
        let mut system = sysinfo::System::new();
        system.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::All,
            true,
            sysinfo::ProcessRefreshKind::nothing().with_cwd(sysinfo::UpdateKind::Always),
        );
        let mut processes = Vec::new();
        for (pid, mut ports) in by_pid {
            ports.sort_unstable();
            ports.dedup();
            let observed = native::inspect(pid).and_then(|id| match before.get(&pid) {
                Some(Ok(previous)) => {
                    verify(previous, &id)?;
                    Ok(id)
                }
                _ => Err(failure("identity_unavailable")),
            });
            let process = system.process(sysinfo::Pid::from_u32(pid));
            let name = observed
                .as_ref()
                .map(|id| {
                    id.executable
                        .rsplit(['\\', '/'])
                        .next()
                        .unwrap_or("")
                        .to_string()
                })
                .unwrap_or_else(|_| {
                    process
                        .map(|p| p.name().to_string_lossy().into_owned())
                        .unwrap_or_default()
                });
            let restriction = observed.as_ref().err().map(ToString::to_string);
            let identity = observed.ok();
            let managed = identity
                .as_ref()
                .and_then(|id| broker.managed_process(id).ok().flatten());
            let (project_id, attribution) = if let Some((_, project)) = &managed {
                (Some(project.clone()), "task".into())
            } else if let Some(project) = identity
                .as_ref()
                .and_then(|id| self.associations.get(id))
                .filter(|id| projects.iter().any(|p| &p.id == *id))
            {
                (Some(project.clone()), "manual".into())
            } else if let Some(id) = &identity {
                // Only accept a cwd read from the same still-live process instance.
                let cwd = process
                    .and_then(|p| p.cwd())
                    .map(|p| p.to_string_lossy().into_owned());
                let evidence = if native::inspect(pid).as_ref().ok() == Some(id) {
                    cwd
                } else {
                    None
                };
                attribute(projects, evidence.as_deref(), &id.executable)
            } else {
                (None, "unknown".into())
            };
            let restriction = restriction
                .or_else(|| protected(pid, &name, &system).then(|| "protected_process".into()));
            processes.push(PortProcess {
                pid,
                name,
                ports,
                identity,
                project_id,
                attribution,
                run_id: managed.map(|(run, _)| run),
                restriction,
            });
        }
        // An association never follows a recycled PID, restarted service, or removed Project.
        self.associations.retain(|identity, project| {
            projects.iter().any(|p| &p.id == project)
                && native::inspect(identity.pid).as_ref().ok() == Some(identity)
        });
        Ok(ProcessInventory {
            supported: true,
            processes,
        })
    }

    pub fn associate(
        &mut self,
        identity: ProcessIdentity,
        project_id: Option<String>,
        projects: &[ProjectSummary],
    ) -> Result<()> {
        verify(&identity, &native::inspect(identity.pid)?)?;
        if let Some(project) = project_id {
            if !projects.iter().any(|p| p.id == project) {
                return Err(failure("project_unavailable"));
            }
            self.associations.insert(identity, project);
        } else {
            self.associations.remove(&identity);
        }
        Ok(())
    }

    pub fn preview(
        &mut self,
        broker: &Broker,
        projects: &[ProjectSummary],
        identity: &ProcessIdentity,
        mode: StopMode,
    ) -> Result<StopPreview> {
        verify(identity, &native::inspect(identity.pid)?)?;
        let inventory = self.inventory(broker, projects)?;
        let target = inventory
            .processes
            .iter()
            .find(|p| p.identity.as_ref() == Some(identity))
            .cloned()
            .ok_or_else(|| failure("process_exited"))?;
        if let Some(reason) = &target.restriction {
            return Err(failure(reason));
        }
        if mode == StopMode::Graceful && target.run_id.is_none() {
            return Err(failure("graceful_unavailable"));
        }
        let affected: Vec<_> = inventory
            .processes
            .into_iter()
            .filter(|p| {
                target
                    .run_id
                    .as_ref()
                    .map(|run| p.run_id.as_ref() == Some(run))
                    .unwrap_or(p.pid == target.pid)
            })
            .collect();
        let process_count = if let Some(run) = &target.run_id {
            broker.managed_process_count(run)?
        } else {
            1
        };
        self.pending
            .retain(|_, p| p.created.elapsed() < Duration::from_secs(60));
        if self.pending.len() >= 16 {
            self.pending.clear();
        }
        let preview = StopPreview {
            token: uuid::Uuid::new_v4().to_string(),
            target,
            mode,
            affected,
            process_count,
        };
        self.pending.insert(
            preview.token.clone(),
            PendingStop {
                preview: preview.clone(),
                created: Instant::now(),
            },
        );
        Ok(preview)
    }

    /// Consume a one-use confirmation and recheck both identity and the reviewed port scope.
    pub fn consume(
        &mut self,
        broker: &Broker,
        projects: &[ProjectSummary],
        token: &str,
    ) -> Result<StopPreview> {
        let pending = self
            .pending
            .remove(token)
            .ok_or_else(|| failure("confirmation_expired"))?;
        if pending.created.elapsed() >= Duration::from_secs(60) {
            return Err(failure("confirmation_expired"));
        }
        let preview = pending.preview;
        let identity = preview
            .target
            .identity
            .as_ref()
            .ok_or_else(|| failure("identity_unavailable"))?;
        verify(identity, &native::inspect(identity.pid)?)?;
        let current = self.inventory(broker, projects)?;
        let target = current
            .processes
            .iter()
            .find(|p| p.identity.as_ref() == Some(identity))
            .ok_or_else(|| failure("process_exited"))?;
        if let Some(reason) = &target.restriction {
            return Err(failure(reason));
        }
        if target.run_id != preview.target.run_id {
            return Err(failure("identity_changed"));
        }
        let scope: Vec<_> = current
            .processes
            .into_iter()
            .filter(|p| {
                preview
                    .target
                    .run_id
                    .as_ref()
                    .map(|run| p.run_id.as_ref() == Some(run))
                    .unwrap_or(p.pid == identity.pid)
            })
            .collect();
        if scope
            .iter()
            .map(|p| (&p.identity, &p.ports))
            .collect::<Vec<_>>()
            != preview
                .affected
                .iter()
                .map(|p| (&p.identity, &p.ports))
                .collect::<Vec<_>>()
        {
            return Err(failure("scope_changed"));
        }
        Ok(preview)
    }
}

pub fn force_external(identity: &ProcessIdentity) -> Result<()> {
    native::terminate(identity)
}

fn verify(expected: &ProcessIdentity, actual: &ProcessIdentity) -> Result<()> {
    if expected != actual {
        Err(failure("identity_changed"))
    } else {
        Ok(())
    }
}

#[cfg(any(windows, test))]
trait ProcessTarget {
    fn identity(&self, pid: u32) -> Result<ProcessIdentity>;
    fn terminate(&self) -> Result<()>;
}

#[cfg(any(windows, test))]
fn stop_verified(target: &impl ProcessTarget, expected: &ProcessIdentity) -> Result<()> {
    verify(expected, &target.identity(expected.pid)?)?;
    target.terminate()
}

fn normalized(path: &str) -> String {
    path.trim_start_matches(r"\\?\")
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}
fn beneath(path: &str, root: &str) -> bool {
    let path = normalized(path);
    let root = normalized(root);
    path == root || path.starts_with(&(root + "\\"))
}

fn attribute(
    projects: &[ProjectSummary],
    cwd: Option<&str>,
    executable: &str,
) -> (Option<String>, String) {
    // A cwd is stronger than a locally installed executable (shared toolchains are common).
    for (evidence, source) in [(cwd, "cwd"), (Some(executable), "executable")] {
        if let Some(path) = evidence {
            let mut candidates: Vec<_> = projects
                .iter()
                .filter(|p| beneath(path, &p.canonical_path))
                .collect();
            candidates.sort_by_key(|p| std::cmp::Reverse(normalized(&p.canonical_path).len()));
            if let Some(project) = candidates.first() {
                if candidates.get(1).is_some_and(|p| {
                    normalized(&p.canonical_path) == normalized(&project.canonical_path)
                }) {
                    return (None, "unknown".into());
                }
                return (Some(project.id.clone()), source.into());
            }
        }
    }
    (None, "unknown".into())
}

fn protected(pid: u32, name: &str, system: &sysinfo::System) -> bool {
    if pid <= 4
        || matches!(
            name.to_lowercase().as_str(),
            "wsl.exe"
                | "wslhost.exe"
                | "wslrelay.exe"
                | "docker.exe"
                | "com.docker.backend.exe"
                | "vmcompute.exe"
        )
    {
        return true;
    }
    let mut current = Some(sysinfo::Pid::from_u32(std::process::id()));
    let mut seen = std::collections::HashSet::new();
    while let Some(id) = current {
        if id.as_u32() == pid {
            return true;
        }
        if !seen.insert(id) {
            break;
        }
        current = system.process(id).and_then(|p| p.parent());
    }
    false
}

#[cfg(windows)]
pub(crate) mod native {
    use super::*;
    use windows_sys::Win32::{
        Foundation::{CloseHandle, GetLastError, FILETIME, HANDLE, STILL_ACTIVE, WAIT_OBJECT_0},
        System::Threading::{
            GetExitCodeProcess, GetProcessTimes, IsProcessCritical, OpenProcess,
            QueryFullProcessImageNameW, TerminateProcess, WaitForSingleObject,
            PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
        },
    };

    pub(crate) struct Handle(pub HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    fn os_error() -> Error {
        match unsafe { GetLastError() } {
            5 => failure("permission_denied"),
            87 => failure("process_exited"),
            _ => failure("identity_unavailable"),
        }
    }
    impl Handle {
        pub(crate) fn open(pid: u32, terminate: bool) -> Result<Self> {
            let access = PROCESS_QUERY_LIMITED_INFORMATION
                | if terminate {
                    PROCESS_TERMINATE | PROCESS_SYNCHRONIZE
                } else {
                    0
                };
            let handle = unsafe { OpenProcess(access, 0, pid) };
            if handle.is_null() {
                Err(os_error())
            } else {
                Ok(Self(handle))
            }
        }
        pub(crate) fn identity(&self, pid: u32) -> Result<ProcessIdentity> {
            unsafe {
                // Inventory needs only query rights; waiting would require SYNCHRONIZE.
                let mut exit_code = 0;
                if GetExitCodeProcess(self.0, &mut exit_code) == 0 {
                    return Err(os_error());
                }
                if exit_code != STILL_ACTIVE as u32 {
                    return Err(failure("process_exited"));
                }
                let mut created: FILETIME = std::mem::zeroed();
                let mut exit: FILETIME = std::mem::zeroed();
                let mut kernel: FILETIME = std::mem::zeroed();
                let mut user: FILETIME = std::mem::zeroed();
                if GetProcessTimes(self.0, &mut created, &mut exit, &mut kernel, &mut user) == 0 {
                    return Err(os_error());
                }
                let mut path = vec![0_u16; 32768];
                let mut size = path.len() as u32;
                if QueryFullProcessImageNameW(self.0, 0, path.as_mut_ptr(), &mut size) == 0 {
                    return Err(os_error());
                }
                Ok(ProcessIdentity {
                    pid,
                    created_at: (((created.dwHighDateTime as u64) << 32)
                        | created.dwLowDateTime as u64)
                        .to_string(),
                    executable: String::from_utf16_lossy(&path[..size as usize]),
                })
            }
        }
    }
    pub fn inspect(pid: u32) -> Result<ProcessIdentity> {
        Handle::open(pid, false)?.identity(pid)
    }
    pub fn terminate(identity: &ProcessIdentity) -> Result<()> {
        if identity.pid <= 4 || identity.pid == std::process::id() {
            return Err(failure("protected_process"));
        }
        let handle = Handle::open(identity.pid, true)?;
        stop_verified(&handle, identity)
    }
    impl ProcessTarget for Handle {
        fn identity(&self, pid: u32) -> Result<ProcessIdentity> {
            Handle::identity(self, pid)
        }
        fn terminate(&self) -> Result<()> {
            // Verify and act through the SAME kernel handle. No taskkill, parent walk, or PID reopen.
            unsafe {
                let mut critical = 0;
                if IsProcessCritical(self.0, &mut critical) == 0 {
                    return Err(os_error());
                }
                if critical != 0 {
                    return Err(failure("protected_process"));
                }
                if TerminateProcess(self.0, 1) == 0 {
                    return Err(os_error());
                }
                if WaitForSingleObject(self.0, 2000) != WAIT_OBJECT_0 {
                    return Err(failure("stop_pending"));
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    fn identity() -> ProcessIdentity {
        ProcessIdentity {
            pid: 123,
            created_at: "134000000000000001".into(),
            executable: r"C:\tools\node.exe".into(),
        }
    }
    struct Target {
        actual: Option<ProcessIdentity>,
        error: &'static str,
        calls: Cell<u32>,
    }
    impl ProcessTarget for Target {
        fn identity(&self, _: u32) -> Result<ProcessIdentity> {
            self.actual.clone().ok_or_else(|| failure(self.error))
        }
        fn terminate(&self) -> Result<()> {
            self.calls.set(self.calls.get() + 1);
            Ok(())
        }
    }
    #[test]
    fn recycled_pid_even_with_same_executable_is_never_terminated() {
        let expected = identity();
        let mut actual = expected.clone();
        actual.created_at = "134000000000000002".into();
        let target = Target {
            actual: Some(actual),
            error: "",
            calls: Cell::new(0),
        };
        assert_eq!(
            stop_verified(&target, &expected).unwrap_err().to_string(),
            "identity_changed"
        );
        assert_eq!(target.calls.get(), 0);
    }
    #[test]
    fn permission_denied_and_exited_fail_closed() {
        for error in [
            "permission_denied",
            "process_exited",
            "identity_unavailable",
        ] {
            let target = Target {
                actual: None,
                error,
                calls: Cell::new(0),
            };
            assert_eq!(
                stop_verified(&target, &identity()).unwrap_err().to_string(),
                error
            );
            assert_eq!(target.calls.get(), 0);
        }
    }
    #[test]
    fn executable_mismatch_blocks_and_verified_handle_is_used_once() {
        let mut actual = identity();
        actual.executable = r"C:\other\node.exe".into();
        let target = Target {
            actual: Some(actual),
            error: "",
            calls: Cell::new(0),
        };
        assert!(stop_verified(&target, &identity()).is_err());
        assert_eq!(target.calls.get(), 0);
        let target = Target {
            actual: Some(identity()),
            error: "",
            calls: Cell::new(0),
        };
        stop_verified(&target, &identity()).unwrap();
        assert_eq!(target.calls.get(), 1);
    }
    fn project(id: &str, path: &str) -> ProjectSummary {
        serde_json::from_value(serde_json::json!({"id":id,"canonicalPath":path,"displayName":id,"detectedName":null,"notes":null,"description":null,"vcsKind":"none","availability":"available","archived":false,"favorite":false,"origin":"manual","scanRootId":null,"languages":[],"frameworks":[],"packageManagers":[],"tags":[],"sourceMtime":null,"lastCommitAt":null,"lastOpenedAt":null,"updatedAt":""})).unwrap()
    }
    #[test]
    fn attribution_is_unknown_without_evidence_and_respects_directory_boundaries() {
        let projects = vec![
            project("one", r"C:\code\app"),
            project("two", r"C:\code\app\child"),
        ];
        assert_eq!(attribute(&projects, None, r"C:\tools\node.exe").0, None);
        assert_eq!(
            attribute(&projects, Some(r"C:\code\app-other"), r"C:\tools\node.exe").0,
            None
        );
        assert_eq!(
            attribute(
                &projects,
                Some(r"c:/CODE/app/child/src"),
                r"C:\tools\node.exe"
            ),
            (Some("two".into()), "cwd".into())
        );
        assert_eq!(
            attribute(&projects, None, r"C:\code\app\bin\server.exe"),
            (Some("one".into()), "executable".into())
        );
        let ambiguous = vec![
            project("one", r"C:\code\app"),
            project("duplicate", r"C:\CODE\APP"),
        ];
        assert_eq!(attribute(&ambiguous, Some(r"C:\code\app"), "").0, None);
    }
    #[test]
    fn associations_do_not_follow_pid_reuse() {
        let mut manager = PortProcessManager::default();
        let first = identity();
        manager.associations.insert(first.clone(), "one".into());
        let mut reused = first;
        reused.created_at.push('0');
        assert!(!manager.associations.contains_key(&reused));
    }
    #[test]
    fn expired_confirmation_is_consumed_without_inspecting_or_stopping() {
        let data = tempfile::tempdir().unwrap();
        let broker = Broker::new(data.path().into()).unwrap();
        let mut manager = PortProcessManager::default();
        let target = PortProcess {
            pid: 123,
            name: "node.exe".into(),
            ports: vec![5173],
            identity: Some(identity()),
            project_id: None,
            attribution: "unknown".into(),
            run_id: None,
            restriction: None,
        };
        let preview = StopPreview {
            token: "expired".into(),
            target,
            mode: StopMode::Force,
            affected: vec![],
            process_count: 1,
        };
        manager.pending.insert(
            "expired".into(),
            PendingStop {
                preview,
                created: Instant::now() - Duration::from_secs(61),
            },
        );
        assert_eq!(
            manager
                .consume(&broker, &[], "expired")
                .unwrap_err()
                .to_string(),
            "confirmation_expired"
        );
        assert!(manager.pending.is_empty());
    }
}

#[cfg(not(windows))]
mod native {
    use super::*;
    pub fn inspect(_: u32) -> Result<ProcessIdentity> {
        Err(failure("unsupported_platform"))
    }
    pub fn terminate(_: &ProcessIdentity) -> Result<()> {
        Err(failure("unsupported_platform"))
    }
}
