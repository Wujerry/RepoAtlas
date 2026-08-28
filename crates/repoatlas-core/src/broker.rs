use crate::error::{Error, Result};
use crate::models::{LogChunk, TaskRun, TaskSpec};
use crate::paths;
use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection};
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use uuid::Uuid;

const DEFAULT_CONCURRENCY: usize = 4;
const MAX_LOG_BYTES: u64 = 10 * 1024 * 1024;

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
    child: Child,
    stdin: Option<std::process::ChildStdin>,
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
        #[cfg(windows)]
        let mut command = windows_command(&resolved_executable, &resolved_argv)?;
        #[cfg(not(windows))]
        let mut command = {
            let mut command = Command::new(&resolved_executable);
            command.args(&resolved_argv);
            command
        };

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

        command
            .current_dir(&cwd)
            .env("PATH", task_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn().map_err(|err| {
            let _ = crate::broker::finish_run(conn, &id, "failed", None);
            start_error(&spec, &cwd, err)
        })?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let stdin = child.stdin.take();
        {
            let mut inner = self.inner.lock().map_err(|_| Error::msg("broker lock"))?;
            inner.runs.push(LiveRun {
                id: id.clone(),
                project_id: spec.project_id.clone(),
                kind: spec.kind.clone(),
                child,
                stdin,
            });
        }

        let broker = self.clone();
        let log_path_thread = log_path.clone();
        let run_id = id.clone();
        let log_budget = Arc::new(AtomicU64::new(MAX_LOG_BYTES));
        thread::spawn(move || {
            let on_chunk = Arc::new(on_chunk);
            let on_exit = on_exit;
            let mut handles = Vec::new();
            if let Some(stdout) = stdout {
                let log_path = log_path_thread.clone();
                let run_id = run_id.clone();
                let on_chunk = on_chunk.clone();
                let log_budget = log_budget.clone();
                handles.push(thread::spawn(move || {
                    pump(
                        stdout,
                        "stdout",
                        &run_id,
                        &log_path,
                        on_chunk.as_ref(),
                        &log_budget,
                    );
                }));
            }
            if let Some(stderr) = stderr {
                let log_path = log_path_thread.clone();
                let run_id = run_id.clone();
                let on_chunk = on_chunk.clone();
                let log_budget = log_budget.clone();
                handles.push(thread::spawn(move || {
                    pump(
                        stderr,
                        "stderr",
                        &run_id,
                        &log_path,
                        on_chunk.as_ref(),
                        &log_budget,
                    );
                }));
            }
            for handle in handles {
                let _ = handle.join();
            }
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
        let Some(stdin) = run.stdin.as_mut() else {
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
                    run.stdin.take();
                    run.child.id()
                })
        };
        if let Some(pid) = child_pid {
            // Do not run the Windows process manager while holding the Broker
            // mutex: a hung taskkill must not block log callbacks or other
            // task controls. Fall back to the direct handle only after the
            // bounded tree-termination attempt returns.
            let tree_killed = terminate_process_tree(pid);
            let mut inner = self.inner.lock().map_err(|_| Error::msg("broker lock"))?;
            if let Some(run) = inner.runs.iter_mut().find(|run| run.id == run_id) {
                if !tree_killed {
                    terminate_child(&mut run.child);
                }
            }
        }
        self.reap(run_id)?;
        get_run(conn, run_id)
    }

    pub fn active(&self) -> Result<Vec<String>> {
        let inner = self.inner.lock().map_err(|_| Error::msg("broker lock"))?;
        Ok(inner.runs.iter().map(|run| run.id.clone()).collect())
    }

    fn reap(&self, run_id: &str) -> Result<Option<i32>> {
        let mut inner = self.inner.lock().map_err(|_| Error::msg("broker lock"))?;
        if let Some(index) = inner.runs.iter().position(|run| run.id == run_id) {
            let mut run = inner.runs.remove(index);
            let code = run.child.wait().ok().and_then(|status| status.code());
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
    let mut entries = Vec::new();
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

fn terminate_child(child: &mut Child) {
    #[cfg(windows)]
    {
        let _ = child.kill();
    }
    #[cfg(not(windows))]
    {
        let _ = child.kill();
    }
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
        let _ = pid;
        false
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
) {
    // Bound the amount of data retained for each captured stream. Without a
    // `take` wrapper a command which never emits a newline could make
    // `BufRead::lines` allocate until the process exits, even though the log
    // file itself is capped below.
    let mut reader = BufReader::new(stream).take(MAX_LOG_BYTES);
    let mut line = Vec::new();
    loop {
        line.clear();
        let read = match reader.read_until(b'\n', &mut line) {
            Ok(read) => read,
            Err(_) => break,
        };
        if read == 0 {
            break;
        }
        let granted = reserve_log_bytes(budget, read);
        if granted == 0 {
            continue;
        }
        let bytes = &line[..granted];
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
/// involving `cmd.exe`.  Keep that exception in one place and make the command
/// string deliberately boring: every token is quoted, delayed expansion is
/// disabled, and values which `cmd.exe` cannot represent losslessly in a `/C`
/// command line are rejected.  Native executables never pass through this
/// path and continue to receive the original argv vector.
#[cfg(windows)]
fn windows_command(executable: &str, argv: &[String]) -> Result<Command> {
    if !is_windows_script_launcher(executable) {
        let mut command = Command::new(executable);
        command.args(argv);
        return Ok(command);
    }

    let command_line = encode_cmd_command_line(executable, argv)?;
    let mut command = Command::new(windows_system_tool("cmd.exe"));
    use std::os::windows::process::CommandExt;

    // `raw_arg` is intentional here.  It lets us pass the already validated
    // `/C` string without Rust adding another layer of Windows quoting.
    command.raw_arg(format!("/D /V:OFF /S /C {command_line}"));
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

/// Encode a command line which will be consumed by `cmd.exe /C`.
///
/// `cmd.exe` has no lossless general-purpose quoting form for a percent sign
/// in a command string: it performs percent expansion before it parses quoted
/// arguments.  Rejecting `%` and embedded quotes is therefore safer than
/// pretending that a caret or an extra quote protects them.  The remaining
/// command metacharacters are enclosed in quotes, and `!` is safe because the
/// caller always enables `/V:OFF`.
fn encode_cmd_command_line(executable: &str, argv: &[String]) -> Result<String> {
    let mut values = Vec::with_capacity(argv.len() + 1);
    values.push(executable);
    values.extend(argv.iter().map(String::as_str));

    let mut encoded = values
        .into_iter()
        .map(encode_cmd_value)
        .collect::<Result<Vec<_>>>()?
        .join(" ");
    // `/S /C` strips the first and last quote from the command string.  The
    // extra pair preserves the quotes around the executable and each argv
    // value, including when the executable path contains spaces.
    encoded.insert(0, '"');
    encoded.push('"');
    Ok(encoded)
}

fn encode_cmd_value(value: &str) -> Result<String> {
    if value
        .chars()
        .any(|character| matches!(character, '\0' | '\r' | '\n'))
    {
        return Err(Error::msg(
            "command values cannot contain NUL or line breaks",
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
    Ok(format!("\"{value}\""))
}

#[cfg(test)]
mod tests {
    use super::{
        display_command, encode_cmd_command_line, encode_cmd_value, resolve_task_command,
        start_error,
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
        assert_eq!(std::env::split_paths(&search_path).next(), Some(bin));
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
    fn quotes_windows_script_tokens_with_shell_metacharacters() {
        let command = encode_cmd_command_line(
            r"tools\build&release.cmd",
            &[
                "--label=nightly | smoke".into(),
                "(preview)<draft>!^".into(),
            ],
        )
        .unwrap();

        assert_eq!(
            command,
            r#"""tools\build&release.cmd" "--label=nightly | smoke" "(preview)<draft>!^"""#
        );
    }

    #[test]
    fn rejects_control_percent_and_quote_values() {
        for value in [
            "line\nfeed",
            "line\rfeed",
            "nul\0byte",
            "100%",
            "quote\"value",
        ] {
            assert!(
                encode_cmd_value(value).is_err(),
                "value should be rejected: {value:?}"
            );
        }
    }
}
