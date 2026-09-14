use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct Session(Child);

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn session() -> Session {
    Session(
        Command::new(env!("CARGO_BIN_EXE_repoatlas-mcp"))
            .env_remove("REPOATLAS_DB")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("start MCP"),
    )
}

fn await_exit(session: &mut Session) {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if let Some(status) = session.0.try_wait().expect("poll MCP") {
            assert!(status.success(), "{status}");
            return;
        }
        assert!(
            Instant::now() < deadline,
            "MCP did not exit after disconnect"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn stdin_eof_exits_without_requests() {
    let mut child = session();
    drop(child.0.stdin.take());
    await_exit(&mut child);
}

#[test]
fn broken_stdout_exits_even_while_stdin_is_open() {
    let mut child = session();
    writeln!(
        child.0.stdin.as_mut().unwrap(),
        r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{}}}}"#
    )
    .expect("initialize");
    let mut output = BufReader::new(child.0.stdout.take().unwrap());
    let mut line = String::new();
    output.read_line(&mut line).expect("initialized response");
    assert!(!line.is_empty());
    drop(output);
    writeln!(
        child.0.stdin.as_mut().unwrap(),
        r#"{{"jsonrpc":"2.0","id":2,"method":"ping"}}"#
    )
    .expect("ping");
    await_exit(&mut child);
}

#[test]
fn disconnect_does_not_stop_another_session() {
    let mut first = session();
    let mut second = session();
    drop(first.0.stdin.take());
    await_exit(&mut first);
    assert!(second.0.try_wait().unwrap().is_none());
    writeln!(
        second.0.stdin.as_mut().unwrap(),
        r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{}}}}"#
    )
    .unwrap();
    let output = second.0.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        BufReader::new(output).read_line(&mut line).unwrap();
        let _ = tx.send(line);
    });
    let line = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("live session response");
    let response: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(response["id"], 1);
    assert_eq!(response["result"]["serverInfo"]["name"], "repoatlas-mcp");
    drop(second.0.stdin.take());
    await_exit(&mut second);
}
