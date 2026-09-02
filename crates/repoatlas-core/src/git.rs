use crate::error::{Error, Result};
use crate::models::{
    GitCommandResult, GitDiff, GitFileStatus, GitLogEntry, GitOp, GitSnapshot, GitStatus,
};
use crate::paths;
use chrono::{SecondsFormat, Utc};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const GIT_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_GIT_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

pub fn snapshot(path: &Path) -> Option<GitSnapshot> {
    status(path).ok().map(|value| value.snapshot)
}

/// Return the checkout root for a path inside a Git worktree.
///
/// A RepoAtlas Project can intentionally point at a package or another
/// directory below the checkout root. Git commands accept that working
/// directory, but checking only `path/.git` incorrectly treats such a
/// Project as non-Git. Walk upwards without spawning Git for every directory
/// visited by the scanner; both regular `.git` directories and linked
/// worktree `.git` files are valid markers.
pub fn repository_root(path: &Path) -> Option<std::path::PathBuf> {
    let mut candidate = paths::canonicalize(path).ok()?;
    if !candidate.is_dir() {
        candidate = candidate.parent()?.to_path_buf();
    }

    loop {
        if candidate.join(".git").exists() {
            return paths::canonicalize(&candidate).ok();
        }
        let parent = candidate.parent()?;
        if parent == candidate {
            return None;
        }
        candidate = parent.to_path_buf();
    }
}

pub fn status(path: &Path) -> Result<GitStatus> {
    ensure_git(path)?;
    let branch = git_optional(path, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();
    let porcelain = git_status_porcelain(path)?;
    let files = parse_porcelain(&porcelain);
    let last = git_optional(path, &["log", "-1", "--format=%H%x1f%s%x1f%cI"]);
    let (last_commit_sha, last_commit_subject, last_commit_at) = match last {
        Some(last) => {
            let parts: Vec<&str> = last.split('\u{1f}').collect();
            (
                parts.first().map(|value| value.to_string()),
                parts.get(1).map(|value| value.to_string()),
                parts.get(2).map(|value| value.to_string()),
            )
        }
        None => (None, None, None),
    };
    let (ahead, behind) = ahead_behind(path);
    let branches = git_text(
        path,
        &["for-each-ref", "--format=%(refname:short)", "refs/heads"],
    )
    .unwrap_or_default()
    .lines()
    .filter(|line| !line.is_empty())
    .map(str::to_string)
    .collect();
    let log = log(path, 20).unwrap_or_default();
    Ok(GitStatus {
        snapshot: GitSnapshot {
            branch: empty_to_none(branch),
            dirty: Some(!files.is_empty()),
            ahead,
            behind,
            last_commit_sha,
            last_commit_subject,
            last_commit_at,
            observed_at: now(),
        },
        files,
        branches,
        log,
    })
}

pub fn diff(path: &Path, file: Option<&str>, staged: bool) -> Result<GitDiff> {
    ensure_git(path)?;
    let mut args = vec!["diff", "--no-color"];
    if staged {
        args.push("--cached");
    }
    if let Some(file) = file {
        // Keep read-only pathspecs inside the checkout as well. The `--`
        // separator prevents option injection, but Git still accepts path
        // traversal pathspecs unless we apply the same relative-path policy
        // used by write operations.
        validate_rel(file)?;
        args.push("--");
        args.push(file);
    }
    Ok(GitDiff {
        path: file.map(str::to_string),
        staged,
        patch: git_text(path, &args)?,
    })
}

pub fn log(path: &Path, limit: u32) -> Result<Vec<GitLogEntry>> {
    ensure_git(path)?;
    let limit = limit.clamp(1, 100).to_string();
    let raw = git_text(
        path,
        &["log", "-n", &limit, "--format=%H%x1f%s%x1f%an%x1f%cI"],
    )?;
    Ok(raw
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let parts: Vec<&str> = line.split('\u{1f}').collect();
            GitLogEntry {
                sha: parts.first().unwrap_or(&"").to_string(),
                subject: parts.get(1).unwrap_or(&"").to_string(),
                author: parts.get(2).unwrap_or(&"").to_string(),
                committed_at: parts.get(3).unwrap_or(&"").to_string(),
            }
        })
        .collect())
}

pub fn execute(path: &Path, op: GitOp) -> Result<GitCommandResult> {
    ensure_git(path)?;
    let args = typed_args(&op)?;
    git_run(path, &args)
}

fn typed_args(op: &GitOp) -> Result<Vec<String>> {
    match op {
        GitOp::Status => Ok(strings(&["status", "--porcelain=v1", "-uall", "--branch"])),
        GitOp::Diff { path, staged } => {
            let mut args = strings(&["diff", "--no-color"]);
            if *staged {
                args.push("--cached".into());
            }
            if let Some(path) = path {
                validate_rel(path)?;
                args.push("--".into());
                args.push(path.clone());
            }
            Ok(args)
        }
        GitOp::Log { limit } => {
            let n = limit.unwrap_or(20).clamp(1, 100).to_string();
            Ok(vec![
                "log".into(),
                "-n".into(),
                n,
                "--format=%H%x1f%s%x1f%an%x1f%cI".into(),
            ])
        }
        GitOp::Branch => Ok(strings(&[
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/heads",
        ])),
        GitOp::Checkout { branch, create } => {
            validate_ref(branch)?;
            if *create {
                Ok(vec!["switch".into(), "-c".into(), branch.clone()])
            } else {
                Ok(vec!["switch".into(), branch.clone()])
            }
        }
        GitOp::Stage { paths } => {
            let mut args = strings(&["add", "--"]);
            for path in paths {
                validate_rel(path)?;
                args.push(path.clone());
            }
            Ok(args)
        }
        GitOp::Unstage { paths } => {
            let mut args = strings(&["restore", "--staged", "--"]);
            for path in paths {
                validate_rel(path)?;
                args.push(path.clone());
            }
            Ok(args)
        }
        GitOp::Commit { message } => {
            if message.trim().is_empty() {
                return Err(Error::msg("commit message is required"));
            }
            Ok(vec!["commit".into(), "-m".into(), message.clone()])
        }
        GitOp::Pull => Ok(strings(&["pull", "--ff-only"])),
        GitOp::Push => Ok(strings(&["push"])),
        GitOp::StashPush { message } => {
            if let Some(message) = message {
                Ok(vec![
                    "stash".into(),
                    "push".into(),
                    "-m".into(),
                    message.clone(),
                ])
            } else {
                Ok(strings(&["stash", "push"]))
            }
        }
        GitOp::StashPop => Ok(strings(&["stash", "pop"])),
    }
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|value| (*value).to_string()).collect()
}

fn parse_porcelain(raw: &[u8]) -> Vec<GitFileStatus> {
    let records = raw.split(|byte| *byte == 0).collect::<Vec<_>>();
    let mut files = Vec::new();
    let mut index = 0;
    while index < records.len() {
        let record = records[index];
        index += 1;
        if record.len() < 4 || record.starts_with(b"##") {
            continue;
        }
        let staged_code = record[0] as char;
        let unstaged_code = record[1] as char;
        let path = record[3..].to_vec();
        // With -z, a rename/copy record stores the destination first and the
        // source in the following NUL-delimited record. Git operations use
        // the destination path, so intentionally discard the source here.
        if matches!(staged_code, 'R' | 'C') && index < records.len() {
            index += 1;
        }
        let staged = staged_code != ' ' && staged_code != '?';
        let code = if staged { staged_code } else { unstaged_code };
        files.push(GitFileStatus {
            path: String::from_utf8_lossy(&path).into_owned(),
            status: status_name(code),
            staged,
        });
    }
    files
}

fn status_name(code: char) -> String {
    match code {
        'M' => "modified",
        'A' => "added",
        'D' => "deleted",
        'R' => "renamed",
        'C' => "copied",
        'U' => "unmerged",
        '?' => "untracked",
        '!' => "ignored",
        _ => "changed",
    }
    .into()
}

fn ahead_behind(path: &Path) -> (Option<i64>, Option<i64>) {
    let Some(raw) = git_optional(
        path,
        &["rev-list", "--left-right", "--count", "@{upstream}...HEAD"],
    ) else {
        return (None, None);
    };
    let mut parts = raw.split_whitespace();
    let behind = parts.next().and_then(|value| value.parse().ok());
    let ahead = parts.next().and_then(|value| value.parse().ok());
    (ahead, behind)
}

fn ensure_git(path: &Path) -> Result<()> {
    if repository_root(path).is_some() {
        Ok(())
    } else {
        Err(Error::msg("not a git project"))
    }
}

fn git_text(path: &Path, args: &[&str]) -> Result<String> {
    let result = git_run(
        path,
        &args
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>(),
    )?;
    if result.ok {
        Ok(result.stdout)
    } else {
        Err(Error::msg(if result.stderr.is_empty() {
            result.stdout
        } else {
            result.stderr
        }))
    }
}

fn git_optional(path: &Path, args: &[&str]) -> Option<String> {
    git_text(path, args).ok().filter(|value| !value.is_empty())
}

fn git_run(path: &Path, args: &[String]) -> Result<GitCommandResult> {
    let result = git_run_raw(path, args)?;
    Ok(GitCommandResult {
        ok: result.ok,
        stdout: String::from_utf8_lossy(&result.stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&result.stderr).trim().to_string(),
    })
}

fn git_status_porcelain(path: &Path) -> Result<Vec<u8>> {
    let args = strings(&["status", "--porcelain=v1", "-z", "-uall"]);
    let result = git_run_raw(path, &args)?;
    if result.ok {
        Ok(result.stdout)
    } else {
        Err(Error::msg(if result.stderr.is_empty() {
            String::from_utf8_lossy(&result.stdout).trim().to_string()
        } else {
            String::from_utf8_lossy(&result.stderr).trim().to_string()
        }))
    }
}

struct RawGitCommandResult {
    ok: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn git_run_raw(path: &Path, args: &[String]) -> Result<RawGitCommandResult> {
    let mut command = Command::new("git");
    command
        .args(args)
        .current_dir(path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    crate::process::suppress_console_window(&mut command);
    let mut child = command
        .spawn()
        .map_err(|err| Error::msg(format!("git is unavailable: {err}")))?;
    let stdout = child.stdout.take().map(read_pipe_limited);
    let stderr = child.stderr.take().map(read_pipe_limited);
    let deadline = Instant::now() + GIT_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Error::msg("git command timed out"));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Error::msg(format!("git process failed: {error}")));
            }
        }
    };
    let stdout = receive_pipe(stdout, deadline)?;
    let stderr = receive_pipe(stderr, deadline)?;
    Ok(RawGitCommandResult {
        ok: status.success(),
        stdout,
        stderr,
    })
}

fn read_pipe_limited<T>(mut stream: T) -> mpsc::Receiver<Vec<u8>>
where
    T: Read + Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::with_capacity(MAX_GIT_OUTPUT_BYTES.min(64 * 1024));
        let mut buffer = [0u8; 8192];
        loop {
            match stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    if bytes.len() < MAX_GIT_OUTPUT_BYTES {
                        let remaining = MAX_GIT_OUTPUT_BYTES - bytes.len();
                        bytes.extend_from_slice(&buffer[..count.min(remaining)]);
                    }
                }
                Err(_) => break,
            }
        }
        let _ = sender.send(bytes);
    });
    receiver
}

fn receive_pipe(receiver: Option<mpsc::Receiver<Vec<u8>>>, deadline: Instant) -> Result<Vec<u8>> {
    let Some(receiver) = receiver else {
        return Ok(Vec::new());
    };
    receiver
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(|_| Error::msg("git command output timed out"))
}

fn validate_rel(path: &str) -> Result<()> {
    let windows_drive = path.as_bytes().get(1).copied() == Some(b':');
    if path.is_empty()
        || path.contains('\0')
        || path.contains('\r')
        || path.contains('\n')
        || path.starts_with('/')
        || path.starts_with('\\')
        || windows_drive
        || path.contains("..")
    {
        Err(Error::msg("invalid git path"))
    } else {
        Ok(())
    }
}

fn validate_ref(name: &str) -> Result<()> {
    if name.is_empty()
        || name.contains(' ')
        || name.contains("..")
        || name.contains('\\')
        || name.starts_with('-')
    {
        Err(Error::msg("invalid git ref"))
    } else {
        Ok(())
    }
}

pub fn origin_url(path: &Path) -> Option<String> {
    git_optional(path, &["remote", "get-url", "origin"])
        .or_else(|| git_optional(path, &["config", "--get", "remote.origin.url"]))
        .and_then(empty_to_none)
}

pub fn normalize_remote_url(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() {
        return None;
    }
    let value = value.trim_end_matches('/');
    let value = value.strip_suffix(".git").unwrap_or(value);
    let value = if let Some(rest) = value.strip_prefix("git@") {
        let rest = rest.replacen(':', "/", 1);
        format!("https://{rest}")
    } else if let Some(rest) = value.strip_prefix("ssh://git@") {
        format!("https://{rest}")
    } else if let Some(rest) = value.strip_prefix("ssh://") {
        format!("https://{rest}")
    } else {
        value.to_string()
    };
    let value = value.trim_end_matches('/');
    Some(value.to_ascii_lowercase())
}

fn empty_to_none(value: String) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::parse_porcelain;

    #[test]
    fn parses_nul_terminated_renames_using_the_destination_path() {
        let files = parse_porcelain(b"R  src/foo bar.rs\0old.txt\0");

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "src/foo bar.rs");
        assert_eq!(files[0].status, "renamed");
        assert!(files[0].staged);
    }

    #[test]
    fn preserves_spaces_quotes_and_non_ascii_in_paths() {
        let files = parse_porcelain(" M src/quoted \"名字\".rs\0".as_bytes());

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "src/quoted \"名字\".rs");
        assert_eq!(files[0].status, "modified");
        assert!(!files[0].staged);
    }

    #[test]
    fn parses_an_ordinary_modified_file() {
        let files = parse_porcelain(b" M src/main.rs\0");

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "src/main.rs");
        assert_eq!(files[0].status, "modified");
        assert!(!files[0].staged);
    }
}
