use crate::error::{Error, Result};
use crate::models::{LogChunk, TaskRun, TaskSpec};
use crate::paths;
use chrono::{SecondsFormat, Utc};
use portable_pty::{CommandBuilder, MasterPty, PtySize};
use rusqlite::{params, Connection};
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use uuid::Uuid;

const DEFAULT_CONCURRENCY: usize = 4;
const MAX_LOG_BYTES: u64 = 10 * 1024 * 1024;
const DEFAULT_PTY_COLS: u16 = 80;
const DEFAULT_PTY_ROWS: u16 = 24;

/// Format a start failure with enough structured context to fix the task.
/// Environment variables are intentionally omitted because they may contain
/// credentials.
fn start_error(spec: &TaskSpec, cwd: &Path, reason: impl std::fmt::Display) -> Error {
    Error::msg(format!(
        "Could not start task.\nCommand: {}\nWorking directory: {}\nReason: {reason}\nCheck that the tool is installed and available on PATH, then verify the task arguments and working directory.",
        display_command(&spec.executable, &spec.argv),
        paths::path_to_string(cwd),
    ))
}

fn display_command(executable: &str, argv: &[String]) -> String {
    std::iter::once(executable)
        .chain(argv.iter().map(String::as_str))
        .map(|value| {
            if value.chars().any(char::is_whitespace) {
                format!("{value:?}")
            } else {
                value.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Clone)]
pub struct Broker {
    inner: Arc<Mutex<BrokerInner>>,
}

struct BrokerInner {
    log_dir: PathBuf,
    max_concurrency: usize,
    runs: Vec<LiveRun>,
}

struct LiveRun {
    id: String,
    project_id: String,
    kind: String,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    writer: Option<Box<dyn Write + Send>>,
    master: Option<Box<dyn MasterPty + Send>>,
    #[cfg(windows)]
    job: job_object::JobHandle,
}

impl Broker {
    pub fn new(log_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&log_dir)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(BrokerInner {
                log_dir,
                max_concurrency: DEFAULT_CONCURRENCY,
                runs: Vec::new(),
            })),
        })
    }

    pub fn start<F, E>(
        &self,
        conn: &Connection,
        spec: TaskSpec,
        on_chunk: F,
        on_exit: E,
    ) -> Result<TaskRun>
    where
        F: Fn(LogChunk) + Send + Sync + 'static,
        E: Fn(String, Option<i32>) + Send + 'static,
    {
        if spec.shell_mode {
            return Err(Error::msg("shell mode is not enabled in this milestone"));
        }
        if is_shell_interpreter(&spec.executable) {
            return Err(Error::msg(
                "shell interpreter tasks must use the disabled shell mode",
            ));
        }
        if spec.executable.trim().is_empty() {
            return Err(Error::msg("executable is required"));
        }
        if spec
            .executable
            .chars()
            .any(|character| matches!(character, '\0' | '\r' | '\n'))
            || spec.argv.iter().any(|arg| {
                arg.chars()
                    .any(|character| matches!(character, '\0' | '\r' | '\n'))
            })
        {
            return Err(Error::msg("invalid executable or argument"));
        }
        let cwd = spec
            .cwd
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        if !cwd.is_dir() {
            return Err(start_error(
                &spec,
                &cwd,
                "the configured working directory does not exist",
            ));
        }
        {
            let inner = self.inner.lock().map_err(|_| Error::msg("broker lock"))?;
            if inner.runs.len() >= inner.max_concurrency {
                return Err(Error::msg("global task concurrency limit reached"));
            }
            if inner
                .runs
                .iter()
                .any(|run| run.project_id == spec.project_id && run.kind == spec.kind)
            {
                return Err(Error::msg("this task definition is already running"));
            }
        }

        let id = Uuid::new_v4().to_string();
        let started_at = now();
        let log_path = {
            let inner = self.inner.lock().map_err(|_| Error::msg("broker lock"))?;
            inner.log_dir.join(format!("{id}.log"))
        };
        fs::write(&log_path, "")?;
        let (resolved_executable, resolved_argv, task_path) =
            resolve_task_command(&spec.executable, &spec.argv, &cwd)
                .map_err(|reason| start_error(&spec, &cwd, reason))?;

        let run = TaskRun {
            id: id.clone(),
            project_id: spec.project_id.clone(),
            task_id: spec.task_id.clone(),
            kind: spec.kind.clone(),
            executable: spec.executable.clone(),
            argv: spec.argv.clone(),
            cwd: paths::path_to_string(&cwd),
            shell_mode: false,
            status: "running".into(),
            exit_code: None,
            log_path: paths::path_to_string(&log_path),
            started_at: started_at.clone(),
            finished_at: None,
        };
        insert_run(conn, &run)?;

        let mut command = match pty_command(&resolved_executable, &resolved_argv) {
            Ok(command) => command,
            Err(reason) => {
                let _ = crate::broker::finish_run(conn, &id, "failed", None);
                return Err(start_error(&spec, &cwd, reason));
            }
        };
        command.cwd(&cwd);
        command.env("PATH", task_path);
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        let pty_system = portable_pty::native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: DEFAULT_PTY_ROWS,
                cols: DEFAULT_PTY_COLS,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|err| {
                let _ = crate::broker::finish_run(conn, &id, "failed", None);
                start_error(&spec, &cwd, err)
            })?;
        let mut child = match pair.slave.spawn_command(command) {
            Ok(child) => child,
            Err(err) => {
                let _ = crate::broker::finish_run(conn, &id, "failed", None);
                return Err(start_error(&spec, &cwd, err));
            }
        };
        let reader = match pair.master.try_clone_reader() {
            Ok(reader) => reader,
            Err(err) => {
                abort_spawn(child.as_mut());
                let _ = crate::broker::finish_run(conn, &id, "failed", None);
                return Err(start_error(&spec, &cwd, err));
            }
        };
        let writer = match pair.master.take_writer() {
            Ok(writer) => writer,
            Err(err) => {
                abort_spawn(child.as_mut());
                let _ = crate::broker::finish_run(conn, &id, "failed", None);
                return Err(start_error(&spec, &cwd, err));
            }
        };
        #[cfg(windows)]
        let job = {
            // Assign the fresh child to a kill-on-close Job Object before it
            // can spawn descendants. Job membership is inherited by every
            // later descendant regardless of parent-chain changes, so stop
            // can terminate processes that taskkill tree-walks miss when an
            // intermediate process exits or is reparented first.
            // Cleanup is only guaranteed while the job exists, so a failure
            // here is fatal for the start attempt instead of a silent
            // downgrade to best-effort termination.
            let job = match job_object::JobHandle::new() {
                Some(job) => job,
                None => {
                    abort_spawn(child.as_mut());
                    let _ = crate::broker::finish_run(conn, &id, "failed", None);
                    return Err(start_error(
                        &spec,
                        &cwd,
                        "failed to create the Windows Job Object that guarantees task cleanup",
                    ));
                }
            };
            let Some(handle) = child.as_raw_handle() else {
                abort_spawn(child.as_mut());
                let _ = crate::broker::finish_run(conn, &id, "failed", None);
                return Err(start_error(
                    &spec,
                    &cwd,
                    "the task process handle is unavailable",
                ));
            };
            if !job.assign(handle) {
                abort_spawn(child.as_mut());
                let _ = crate::broker::finish_run(conn, &id, "failed", None);
                return Err(start_error(
                    &spec,
                    &cwd,
                    "failed to assign the task process to the cleanup Job Object",
                ));
            }
            job
        };
        {
            let mut inner = self.inner.lock().map_err(|_| Error::msg("broker lock"))?;
            inner.runs.push(LiveRun {
                id: id.clone(),
                project_id: spec.project_id.clone(),
                kind: spec.kind.clone(),
                child,
                writer: Some(writer),
                master: Some(pair.master),
                #[cfg(windows)]
                job,
            });
        }

        let broker = self.clone();
        let log_path_thread = log_path.clone();
        let run_id = id.clone();
        let log_budget = Arc::new(AtomicU64::new(MAX_LOG_BYTES));
        thread::spawn(move || {
            let on_chunk = Arc::new(on_chunk);
            let on_exit = on_exit;
            pump(
                reader,
                "pty",
                &run_id,
                &log_path_thread,
                on_chunk.as_ref(),
                &log_budget,
                || {
                    let reply = format!("{}[24;80R", char::from_u32(0x1b).unwrap());
                    let _ = broker.write_stdin(&run_id, &reply);
                },
            );
            let code = broker.reap(&run_id).ok().flatten();
            on_exit(run_id, code);
        });
        Ok(run)
    }

    pub fn write_stdin(&self, run_id: &str, text: &str) -> Result<()> {
        let mut inner = self.inner.lock().map_err(|_| Error::msg("broker lock"))?;
        let Some(run) = inner.runs.iter_mut().find(|run| run.id == run_id) else {
            return Err(Error::msg("task is not running"));
        };
        let Some(stdin) = run.writer.as_mut() else {
            return Err(Error::msg("task stdin is not available"));
        };
        stdin
            .write_all(text.as_bytes())
            .map_err(|err| Error::msg(format!("failed to write task input: {err}")))?;
        stdin
            .flush()
            .map_err(|err| Error::msg(format!("failed to flush task input: {err}")))?;
        Ok(())
    }

    pub fn stop(&self, conn: &Connection, run_id: &str) -> Result<TaskRun> {
        let child_pid = {
            let mut inner = self.inner.lock().map_err(|_| Error::msg("broker lock"))?;
            inner
                .runs
                .iter_mut()
                .find(|run| run.id == run_id)
                .map(|run| {
                    run.writer.take();
                    let _ = run.master.take();
                    run.child.process_id().unwrap_or(0)
                })
                .unwrap_or(0)
        };
        if child_pid > 0 {
            // Do not run the Windows process manager while holding the Broker
            // mutex: a hung taskkill must not block log callbacks or other
            // task controls. Fall back to the direct handle only after the
            // bounded tree-termination attempt returns.
            let tree_killed = terminate_process_tree(child_pid);
            let mut inner = self.inner.lock().map_err(|_| Error::msg("broker lock"))?;
            if let Some(run) = inner.runs.iter_mut().find(|run| run.id == run_id) {
                #[cfg(windows)]
                {
                    // The Job Object is the authoritative cleanup guarantee:
                    // it terminates every descendant that joined the run,
                    // including grandchildren that taskkill tree-walks miss
                    // because their parent exited or was reparented first.
                    // Closing the handle during reap would also kill them,
                    // but terminating here stops output immediately.
                    run.job.terminate();
                }
                if !tree_killed {
                    terminate_child(run.child.as_mut());
                }
            }
        } else {
            let mut inner = self.inner.lock().map_err(|_| Error::msg("broker lock"))?;
            if let Some(run) = inner.runs.iter_mut().find(|run| run.id == run_id) {
                #[cfg(windows)]
                run.job.terminate();
                terminate_child(run.child.as_mut());
            }
        }
        self.reap(run_id)?;
        get_run(conn, run_id)
    }

    pub fn active(&self) -> Result<Vec<String>> {
        let inner = self.inner.lock().map_err(|_| Error::msg("broker lock"))?;
        Ok(inner.runs.iter().map(|run| run.id.clone()).collect())
    }

    pub fn resize(&self, run_id: &str, cols: u16, rows: u16) -> Result<()> {
        if cols == 0 || rows == 0 {
            return Err(Error::msg("terminal size must be greater than zero"));
        }
        let inner = self.inner.lock().map_err(|_| Error::msg("broker lock"))?;
        let Some(run) = inner.runs.iter().find(|run| run.id == run_id) else {
            return Err(Error::msg("task is not running"));
        };
        let Some(master) = run.master.as_ref() else {
            return Err(Error::msg("task terminal is closing"));
        };
        master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|err| Error::msg(format!("failed to resize task terminal: {err}")))
    }

    fn reap(&self, run_id: &str) -> Result<Option<i32>> {
        let mut inner = self.inner.lock().map_err(|_| Error::msg("broker lock"))?;
        if let Some(index) = inner.runs.iter().position(|run| run.id == run_id) {
            let mut run = inner.runs.remove(index);
            run.writer.take();
            let code = run
                .child
                .wait()
                .ok()
                .map(|status| status.exit_code() as i32);
            return Ok(code);
        }
        Ok(None)
    }
}

fn resolve_task_command(
    executable: &str,
    argv: &[String],
    cwd: &Path,
) -> std::result::Result<(String, Vec<String>, OsString), String> {
    let search_path = task_search_path(cwd);
    if let Some(resolved) = find_executable(executable, cwd, &search_path) {
        return Ok((paths::path_to_string(&resolved), argv.to_vec(), search_path));
    }

    // Corepack is the standard distribution path for pnpm and Yarn on many
    // Node installations. Using it as a structured fallback covers those
    // projects without introducing shell evaluation.
    let manager = executable.to_ascii_lowercase();
    if matches!(manager.as_str(), "pnpm" | "yarn") {
        if let Some(corepack) = find_executable("corepack", cwd, &search_path) {
            let mut resolved_argv = Vec::with_capacity(argv.len() + 1);
            resolved_argv.push(manager);
            resolved_argv.extend_from_slice(argv);
            return Ok((paths::path_to_string(&corepack), resolved_argv, search_path));
        }
    }

    Err(format!(
        "executable `{executable}` was not found in the project tool directories or PATH"
    ))
}

fn task_search_path(cwd: &Path) -> OsString {
    let stop = crate::git::repository_root(cwd).unwrap_or_else(|| cwd.to_path_buf());
    // Keep the child PATH consistent with find_executable, which also
    // searches the working directory first: script tokens are resolved by
    // cmd.exe against this PATH, so a script sitting directly in the task
    // directory must stay reachable after the swap.
    let mut entries = vec![cwd.to_path_buf()];
    let mut current = Some(cwd);
    while let Some(directory) = current {
        entries.extend([
            directory.join("node_modules").join(".bin"),
            directory
                .join(".venv")
                .join(if cfg!(windows) { "Scripts" } else { "bin" }),
            directory
                .join("venv")
                .join(if cfg!(windows) { "Scripts" } else { "bin" }),
        ]);
        if directory == stop {
            break;
        }
        current = directory.parent();
    }
    entries.retain(|path| path.is_dir());
    if let Some(path) = std::env::var_os("PATH") {
        entries.extend(std::env::split_paths(&path));
    }
    std::env::join_paths(entries).unwrap_or_else(|_| std::env::var_os("PATH").unwrap_or_default())
}

fn find_executable(executable: &str, cwd: &Path, search_path: &OsString) -> Option<PathBuf> {
    let value = Path::new(executable);
    if value.is_absolute() {
        return value.is_file().then(|| value.to_path_buf());
    }
    if value.components().count() > 1 {
        let candidate = cwd.join(value);
        return candidate.is_file().then_some(candidate);
    }

    let mut directories = vec![cwd.to_path_buf()];
    directories.extend(std::env::split_paths(search_path));
    for directory in directories {
        for candidate in executable_candidates(&directory, executable) {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn executable_candidates(directory: &Path, executable: &str) -> Vec<PathBuf> {
    let exact = directory.join(executable);
    #[cfg(windows)]
    {
        if Path::new(executable).extension().is_some() {
            return vec![exact];
        }
        let extensions = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
        let mut candidates = vec![exact];
        candidates.extend(
            extensions
                .split(';')
                .filter(|extension| !extension.is_empty())
                .map(|extension| directory.join(format!("{executable}{extension}"))),
        );
        candidates
    }
    #[cfg(not(windows))]
    {
        vec![exact]
    }
}

/// Kernel-enforced process-tree cleanup for structured tasks on Windows.
///
/// taskkill /T walks parent PIDs and misses descendants whose parent exited
/// or was reparented between enumeration and termination, which let dev
/// servers keep streaming output after an explicit stop. A Job Object with
/// JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE inherits membership to every descendant
/// regardless of parentage, so terminating (or closing) the job reliably
/// stops the whole run.
#[cfg(windows)]
mod job_object {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    /// Owned Job Object handle. The raw handle is not naturally Send, but
    /// ownership only moves with the LiveRun it belongs to behind the broker
    /// mutex, so moving it between threads is sound.
    pub(super) struct JobHandle(HANDLE);

    unsafe impl Send for JobHandle {}

    impl JobHandle {
        pub(super) fn new() -> Option<Self> {
            unsafe {
                let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if handle.is_null() {
                    return None;
                }
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let configured = SetInformationJobObject(
                    handle,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const core::ffi::c_void,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                if configured == 0 {
                    CloseHandle(handle);
                    return None;
                }
                Some(Self(handle))
            }
        }

        pub(super) fn assign(&self, process: std::os::windows::io::RawHandle) -> bool {
            unsafe { AssignProcessToJobObject(self.0, process as HANDLE) != 0 }
        }

        /// Kill every process currently in the job. Descendants that spawn
        /// after the stop request cannot survive: the root is terminated in
        /// the same critical section, and any straggler dies on Drop.
        pub(super) fn terminate(&self) {
            unsafe {
                TerminateJobObject(self.0, 1);
            }
        }
    }

    impl Drop for JobHandle {
        fn drop(&mut self) {
            // KILL_ON_JOB_CLOSE makes this the final cleanup guarantee for
            // descendants that survived a failed or partial termination.
            unsafe { CloseHandle(self.0) };
        }
    }
}

/// Kill a spawned-but-not-yet-tracked child and collect it, so a failed
/// start cannot leak a live process behind the error.
fn abort_spawn(child: &mut dyn portable_pty::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn terminate_child(child: &mut dyn portable_pty::Child) {
    let _ = child.kill();
}

/// Shell interpreters are intentionally not accepted as structured tasks.
/// Passing an interpreter through `Command + argv` still gives a task the
/// ability to reinterpret arbitrary arguments as a second command.  Shell
/// mode remains an explicit, disabled-in-this-milestone escape hatch at the
/// API boundary instead of being silently inferred from the executable name.
fn is_shell_interpreter(executable: &str) -> bool {
    let name = executable
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(executable)
        .to_ascii_lowercase();
    matches!(
        name.as_str(),
        "cmd"
            | "cmd.exe"
            | "command"
            | "command.com"
            | "powershell"
            | "powershell.exe"
            | "pwsh"
            | "pwsh.exe"
            | "bash"
            | "bash.exe"
            | "sh"
            | "sh.exe"
            | "zsh"
            | "zsh.exe"
            | "fish"
            | "fish.exe"
            | "wsl"
            | "wsl.exe"
    )
}

fn terminate_process_tree(pid: u32) -> bool {
    #[cfg(windows)]
    {
        let pid = pid.to_string();
        let mut command = Command::new(windows_system_tool("taskkill.exe"));
        command
            .args(["/PID", pid.as_str(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        crate::process::suppress_console_window(&mut command);
        let Ok(mut child) = command.spawn() else {
            return false;
        };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return status.success(),
                Ok(None) if std::time::Instant::now() < deadline => {
                    thread::sleep(std::time::Duration::from_millis(10));
                }
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return false;
                }
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return false;
                }
            }
        }
    }
    #[cfg(not(windows))]
    {
        // portable-pty starts the child in its own session, so the negative
        // pid signals the whole process group. Ask politely first; a task
        // that ignores SIGTERM must still die, so escalate to SIGKILL after
        // a bounded grace period instead of leaving the stop request
        // half-done.
        let group = -(pid as i32);
        let pid = pid as i32;
        unsafe {
            if libc::kill(group, libc::SIGTERM) != 0 && libc::kill(pid, libc::SIGTERM) != 0 {
                // Nothing is left to signal; reap() reports the real status.
                return true;
            }
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            // kill(pid, 0) keeps succeeding for a zombie until the output
            // pump reaps it, so this check converges as soon as the child
            // is actually gone.
            let alive = unsafe { libc::kill(pid, 0) == 0 };
            if !alive {
                return true;
            }
            if std::time::Instant::now() >= deadline {
                break;
            }
            thread::sleep(std::time::Duration::from_millis(10));
        }
        unsafe {
            let _ = libc::kill(group, libc::SIGKILL);
            let _ = libc::kill(pid, libc::SIGKILL);
        }
        true
    }
}

#[cfg(windows)]
fn windows_system_tool(name: &str) -> PathBuf {
    let root = std::env::var_os("SystemRoot")
        .or_else(|| std::env::var_os("WINDIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    root.join("System32").join(name)
}

pub fn finish_run(
    conn: &Connection,
    run_id: &str,
    status: &str,
    exit_code: Option<i32>,
) -> Result<TaskRun> {
    conn.execute(
        // A stop request reaps the child before the output pump's exit
        // callback gets a chance to run. Make completion idempotent so that
        // the late callback cannot overwrite a deliberate `cancelled`
        // result with a synthetic `failed` status (or race another terminal
        // transition).
        "UPDATE task_runs SET status = ?1, exit_code = ?2, finished_at = ?3 WHERE id = ?4 AND status IN ('running', 'starting')",
        params![status, exit_code, now(), run_id],
    )?;
    get_run(conn, run_id)
}

pub fn list_runs(conn: &Connection, project_id: &str) -> Result<Vec<TaskRun>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, task_id, kind, executable, argv_json, cwd, shell_mode, status, exit_code, log_path, started_at, finished_at FROM task_runs WHERE project_id = ?1 ORDER BY datetime(started_at) DESC LIMIT 50",
    )?;
    let rows = stmt
        .query_map(params![project_id], row_to_run)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn list_active_runs(conn: &Connection) -> Result<Vec<TaskRun>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, task_id, kind, executable, argv_json, cwd, shell_mode, status, exit_code, log_path, started_at, finished_at FROM task_runs WHERE status IN ('running', 'starting') ORDER BY datetime(started_at) ASC",
    )?;
    let rows = stmt
        .query_map([], row_to_run)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn get_run(conn: &Connection, run_id: &str) -> Result<TaskRun> {
    conn.query_row(
        "SELECT id, project_id, task_id, kind, executable, argv_json, cwd, shell_mode, status, exit_code, log_path, started_at, finished_at FROM task_runs WHERE id = ?1",
        params![run_id],
        row_to_run,
    )
    .map_err(|_| Error::NotFound(run_id.into()))
}

pub fn read_log(run: &TaskRun, max_bytes: usize) -> Result<String> {
    let mut file = match fs::File::open(&run.log_path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(error) => return Err(error.into()),
    };
    let length = file.metadata()?.len();
    let start = length.saturating_sub(max_bytes as u64);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::with_capacity(length.saturating_sub(start).min(max_bytes as u64) as usize);
    file.take(max_bytes as u64).read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn insert_run(conn: &Connection, run: &TaskRun) -> Result<()> {
    conn.execute(
        "INSERT INTO task_runs (id, project_id, task_id, kind, executable, argv_json, cwd, shell_mode, status, exit_code, log_path, started_at, finished_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            run.id,
            run.project_id,
            run.task_id,
            run.kind,
            run.executable,
            serde_json::to_string(&run.argv).unwrap_or_else(|_| "[]".into()),
            run.cwd,
            run.shell_mode as i64,
            run.status,
            run.exit_code,
            run.log_path,
            run.started_at,
            run.finished_at
        ],
    )?;
    Ok(())
}

fn row_to_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskRun> {
    let argv_json: String = row.get(5)?;
    Ok(TaskRun {
        id: row.get(0)?,
        project_id: row.get(1)?,
        task_id: row.get(2)?,
        kind: row.get(3)?,
        executable: row.get(4)?,
        argv: serde_json::from_str(&argv_json).unwrap_or_default(),
        cwd: row.get(6)?,
        shell_mode: row.get::<_, i64>(7)? != 0,
        status: row.get(8)?,
        exit_code: row.get(9)?,
        log_path: row.get(10)?,
        started_at: row.get(11)?,
        finished_at: row.get(12)?,
    })
}

fn pump<T: std::io::Read>(
    stream: T,
    name: &str,
    run_id: &str,
    log_path: &Path,
    on_chunk: &dyn Fn(LogChunk),
    budget: &AtomicU64,
    mut on_cursor_query: impl FnMut(),
) {
    // Bound the amount of data retained for each captured stream. Without a
    // `take` wrapper a command which never emits a newline could make
    // `BufRead::lines` allocate until the process exits, even though the log
    // file itself is capped below.
    // PTY output is a mixed byte stream of text, ANSI, and in-place progress
    // updates, so keep the original bytes instead of waiting for newlines.
    let mut reader = BufReader::new(stream);
    let mut buffer = [0_u8; 4096];
    while let Ok(read) = reader.read(&mut buffer) {
        if read == 0 {
            break;
        }
        let granted = reserve_log_bytes(budget, read);
        if granted == 0 {
            continue;
        }
        let bytes = &buffer[..granted];
        if bytes
            .windows(4)
            .any(|window| window == [0x1b, b'[', b'6', b'n'])
        {
            on_cursor_query();
        }
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) {
            let _ = file.write_all(bytes);
        }
        on_chunk(LogChunk {
            run_id: run_id.into(),
            stream: name.into(),
            text: String::from_utf8_lossy(bytes).into_owned(),
        });
    }
}

fn reserve_log_bytes(budget: &AtomicU64, requested: usize) -> usize {
    let requested = requested as u64;
    loop {
        let available = budget.load(Ordering::Relaxed);
        if available == 0 {
            return 0;
        }
        let granted = available.min(requested);
        if budget
            .compare_exchange(
                available,
                available - granted,
                Ordering::Relaxed,
                Ordering::Relaxed,
            )
            .is_ok()
        {
            return granted as usize;
        }
    }
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// Build the command used for a structured task on Windows.
///
/// Windows cannot execute a `.cmd`/`.bat` file through `CreateProcess` without
/// involving `cmd.exe`.  Keep that exception in one place and make the
/// `/C` payload deliberately boring: the script is addressed by a token that
/// survives the payload line unquoted, delayed expansion is disabled, and
/// values which `cmd.exe` cannot represent losslessly in a `/C` command
/// line are rejected.  Native executables never pass through this path and
/// continue to receive the original argv vector.
#[cfg(windows)]
fn pty_command(executable: &str, argv: &[String]) -> Result<CommandBuilder> {
    if !is_windows_script_launcher(executable) {
        let mut command = CommandBuilder::new(executable);
        command.args(argv);
        return Ok(command);
    }

    let token = cmd_script_token(executable)?;
    let mut command = CommandBuilder::new(windows_system_tool("cmd.exe"));
    command.arg("/D");
    command.arg("/V:OFF");
    command.arg("/S");
    command.arg("/C");
    // portable-pty only quotes values containing whitespace. The token is
    // whitespace-free, so it reaches the payload line verbatim, where /S
    // cannot strip a first quote that does not exist.
    command.arg(&token);
    for argument in argv {
        validate_cmd_value(argument)?;
        command.arg(argument);
    }
    Ok(command)
}

/// Pick the payload token that addresses a Windows script.
///
/// `cmd.exe /S /C` strips the first and last quote of the payload, so a
/// quoted script path breaks (and can be turned into a second command)
/// whenever quoted arguments follow. Scripts are therefore addressed by a
/// token that stays unquoted: the full path when it is payload-safe, or the
/// bare file name which `cmd.exe` resolves against the task working
/// directory and PATH.
#[cfg(windows)]
fn cmd_script_token(executable: &str) -> Result<String> {
    let path = Path::new(executable);
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .map(str::to_string);
    if is_cmd_payload_token(executable) {
        return Ok(executable.to_string());
    }
    if let Some(name) = file_name.as_deref() {
        if is_cmd_payload_token(name) {
            return Ok(name.to_string());
        }
    }
    Err(Error::msg(
        "the script path cannot be expressed safely in a cmd.exe command line; rename the script or move the project to a path without spaces or cmd.exe metacharacters",
    ))
}

/// True when the value can be placed on the `/C` payload line without
/// quoting: no whitespace (which would trigger portable-pty quoting), no
/// quotes, no percent expansion, no control characters, and none of the
/// characters cmd.exe treats as separators or grouping even outside quotes.
#[cfg(windows)]
fn is_cmd_payload_token(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            !character.is_whitespace()
                && !character.is_control()
                && !matches!(
                    character,
                    '"' | '%' | '&' | '|' | '<' | '>' | '^' | '(' | ')' | ',' | ';' | '='
                )
        })
}

#[cfg(not(windows))]
fn pty_command(executable: &str, argv: &[String]) -> Result<CommandBuilder> {
    let mut command = CommandBuilder::new(executable);
    command.args(argv);
    Ok(command)
}

#[cfg(windows)]
fn is_windows_script_launcher(executable: &str) -> bool {
    let path = Path::new(executable);
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());
    if matches!(extension.as_deref(), Some("cmd") | Some("bat")) {
        return true;
    }
    matches!(
        path.file_name()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .as_deref(),
        Some("npm") | Some("npx") | Some("pnpm") | Some("yarn") | Some("bun")
    )
}

/// Validate a value destined for a Windows script argument vector.
///
/// `cmd.exe` has no lossless general-purpose quoting form for a percent sign
/// in a command string: it performs percent expansion before it parses quoted
/// arguments.  Rejecting `%` and embedded quotes is therefore safer than
/// pretending that a caret or an extra quote protects them.  portable-pty
/// quotes values containing whitespace, and cmd.exe keeps everything except
/// `%` and quotes literal inside those quotes.  Values without whitespace
/// reach the `/C` payload verbatim, where separators, redirection, escapes,
/// and batch-parameter delimiters must not appear.  `!` stays literal
/// because the caller always enables `/V:OFF`, and parentheses are literal
/// in argument position.
#[cfg(any(windows, test))]
fn validate_cmd_value(value: &str) -> Result<()> {
    if value.chars().any(|character| character.is_control()) {
        return Err(Error::msg(
            "command values cannot contain control characters or line breaks",
        ));
    }
    if value.contains('%') {
        return Err(Error::msg(
            "command values containing '%' cannot be passed to a Windows script safely",
        ));
    }
    if value.contains('"') {
        return Err(Error::msg(
            "command values containing double quotes cannot be passed to a Windows script safely",
        ));
    }
    // portable-pty quotes values containing space, tab, newline, vertical
    // tab, or a double quote.  Quotes and control characters are rejected
    // above, so a plain space or tab is the only remaining quoting trigger,
    // and cmd.exe keeps every other character literal inside those quotes.
    let quoted = value.contains(' ') || value.contains('\t');
    if !quoted
        && value
            .chars()
            .any(|character| matches!(character, '&' | '|' | '<' | '>' | '^' | ',' | ';' | '='))
    {
        return Err(Error::msg(
            "command values without whitespace cannot contain the cmd.exe metacharacters & | < > ^ , ; =",
        ));
    }
    Ok(())
}

/// Mirror the runtime command construction for string-level tests: a
/// payload-safe script token followed by portable-pty style quoting for
/// values with whitespace.
#[cfg(test)]
fn encode_cmd_command_line(executable: &str, argv: &[String]) -> Result<String> {
    let mut values = vec![cmd_script_token(executable)?];
    for argument in argv {
        validate_cmd_value(argument)?;
        values.push(
            if argument.is_empty() || argument.chars().any(|character| character.is_whitespace()) {
                format!("\"{argument}\"")
            } else {
                argument.to_string()
            },
        );
    }
    Ok(values.join(" "))
}

#[cfg(test)]
mod tests {
    use super::{
        cmd_script_token, display_command, encode_cmd_command_line, resolve_task_command,
        start_error, validate_cmd_value,
    };
    use crate::models::TaskSpec;
    use std::path::Path;

    #[test]
    fn start_errors_include_command_cwd_reason_and_recovery() {
        let spec = TaskSpec {
            project_id: "project".into(),
            task_id: Some("test".into()),
            kind: "test".into(),
            executable: "pnpm".into(),
            argv: vec!["run".into(), "hello world".into()],
            cwd: Some("missing".into()),
            shell_mode: false,
        };
        let error = start_error(&spec, Path::new("missing"), "not found").to_string();
        assert!(error.contains("Command: pnpm run \"hello world\""));
        assert!(error.contains("Working directory: missing"));
        assert!(error.contains("Reason: not found"));
        assert!(error.contains("available on PATH"));
        assert_eq!(display_command("cargo", &["test".into()]), "cargo test");
    }

    #[test]
    fn resolves_project_local_tools_before_global_path() {
        let temp = tempfile::tempdir().unwrap();
        let bin = temp.path().join("node_modules").join(".bin");
        std::fs::create_dir_all(&bin).unwrap();
        #[cfg(windows)]
        let tool = bin.join("repoatlas-local.cmd");
        #[cfg(not(windows))]
        let tool = bin.join("repoatlas-local");
        std::fs::write(&tool, "local tool").unwrap();

        let (resolved, argv, search_path) =
            resolve_task_command("repoatlas-local", &["hello world".into()], temp.path()).unwrap();
        #[cfg(windows)]
        assert_eq!(
            resolved.to_ascii_lowercase(),
            tool.to_string_lossy().to_ascii_lowercase()
        );
        #[cfg(not(windows))]
        assert_eq!(std::path::PathBuf::from(resolved), tool);
        assert_eq!(argv, vec!["hello world"]);
        // The working directory itself comes first, then the project tool
        // directories, before the inherited PATH.
        let directories = std::env::split_paths(&search_path).collect::<Vec<_>>();
        assert_eq!(directories.first(), Some(&temp.path().to_path_buf()));
        assert_eq!(directories.get(1), Some(&bin));
    }

    #[test]
    fn missing_tools_return_a_specific_preflight_reason() {
        let temp = tempfile::tempdir().unwrap();
        let error =
            resolve_task_command("repoatlas-definitely-missing", &[], temp.path()).unwrap_err();
        assert!(error.contains("was not found"));
        assert!(error.contains("project tool directories or PATH"));
    }

    #[test]
    fn windows_script_tokens_prefer_full_paths_and_fall_back_to_basenames() {
        assert_eq!(
            cmd_script_token(r"C:\tools\npm.cmd").unwrap(),
            r"C:\tools\npm.cmd"
        );
        // Whitespace or a parenthesis in the command token breaks or splits
        // the /C payload, so cmd.exe gets the bare file name, which it
        // resolves against the task working directory and PATH.
        assert_eq!(cmd_script_token(r"C:\My Tools\npm.cmd").unwrap(), "npm.cmd");
        assert_eq!(
            cmd_script_token(r"C:\tools(x86)\npm.cmd").unwrap(),
            "npm.cmd"
        );
        // Neither form is payload-safe: fail loudly instead of launching
        // something unintended.
        assert!(cmd_script_token(r"C:\My Tools\my npm.cmd").is_err());
    }

    #[test]
    fn encodes_windows_script_commands_like_the_pty_payload() {
        let command = encode_cmd_command_line(
            r"C:\tools\build.cmd",
            &["--label=nightly | smoke".into(), "(preview) draft".into()],
        )
        .unwrap();

        assert_eq!(
            command,
            r#"C:\tools\build.cmd "--label=nightly | smoke" "(preview) draft""#
        );
        assert_eq!(
            encode_cmd_command_line(r"C:\tools\build.cmd", &["".into()]).unwrap(),
            r#"C:\tools\build.cmd """#
        );
    }

    #[test]
    fn windows_script_args_allow_quoted_metacharacters_and_reject_raw_ones() {
        // Values with whitespace are quoted by portable-pty; cmd.exe keeps
        // everything but % and double quotes literal inside those quotes.
        for value in ["--label=nightly | smoke", "(preview) <draft> !^"] {
            assert!(
                validate_cmd_value(value).is_ok(),
                "value should be accepted: {value:?}"
            );
        }
        // Without whitespace the value reaches the /C payload verbatim.
        // '!' and parentheses are literal under /V:OFF and stay allowed;
        // separators, redirection, escapes, and parameter delimiters are not.
        for value in ["hi!bye", "(install)", "hello"] {
            assert!(
                validate_cmd_value(value).is_ok(),
                "value should be accepted: {value:?}"
            );
        }
        {
            let value = "a=b,c;d";
            assert!(
                validate_cmd_value(value).is_err(),
                "value should be rejected: {value:?}"
            );
        }
        for value in [
            "a&b",
            "a|b",
            "a<b",
            "a>b",
            "a^b",
            "a,b",
            "a;b",
            "a=b",
            "100%",
            "quote\"value",
            "line\nfeed",
            "line\rfeed",
            "nul\0byte",
            "a\tb",
        ] {
            assert!(
                validate_cmd_value(value).is_err(),
                "value should be rejected: {value:?}"
            );
        }
    }
}
