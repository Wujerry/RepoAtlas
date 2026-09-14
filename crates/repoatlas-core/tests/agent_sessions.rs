use repoatlas_core::{
    agent_sessions::{
        adapters::{fingerprint, LocalAdapter, SessionAdapter},
        *,
    },
    Core,
};
use serde_json::json;
use std::{fs, path::Path, sync::atomic::AtomicBool};
fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}
fn parse(adapter: &str, path: &Path, root: &Path) -> (AgentSession, Vec<SessionMessage>) {
    LocalAdapter(adapter.into())
        .read(root, path, &AtomicBool::new(false))
        .unwrap()
        .remove(0)
}

#[test]
fn json_adapters_extract_only_visible_messages_and_stable_identity() {
    let dir = tempfile::tempdir().unwrap();
    let cases = [
        (
            "claude",
            "claude/session.jsonl",
            vec![
                json!({"type":"user","sessionId":"test-1","cwd":"/workspace","message":{"role":"user","content":[{"type":"text","text":"中文 DPI-1047"},{"type":"tool_result","content":"SECRET"}]}}),
                json!({"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"SECRET"},{"type":"text","text":"visible"}]}}),
            ],
        ),
        (
            "codex",
            "codex/rollout-test.jsonl",
            vec![
                json!({"type":"session_meta","payload":{"id":"test-1","cwd":"/workspace"}}),
                json!({"type":"turn_context","payload":{"cwd":"/workspace","summary":"auto"}}),
                json!({"type":"event_msg","payload":{"type":"user_message","message":"中文 DPI-1047"}}),
                json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"中文 DPI-1047"}]}}),
                json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"visible"}]}}),
            ],
        ),
        (
            "qwen",
            "qwen/chats/session.jsonl",
            vec![
                json!({"type":"user","sessionId":"test-1","cwd":"/workspace","message":{"role":"user","parts":[{"text":"中文 DPI-1047"}]}}),
                json!({"type":"assistant","message":{"role":"model","parts":[{"thought":true,"text":"SECRET"},{"text":"visible"}]}}),
            ],
        ),
        (
            "copilot",
            "copilot/test-1/events.jsonl",
            vec![
                json!({"type":"session.start","data":{"sessionId":"test-1","context":{"cwd":"/workspace"}}}),
                json!({"type":"user.message","data":{"content":"中文 DPI-1047"}}),
                json!({"type":"assistant.message","data":{"content":"visible"}}),
            ],
        ),
        (
            "kimi",
            "kimi/sessions/work/test-1/context.jsonl",
            vec![
                json!({"role":"user","content":"中文 DPI-1047"}),
                json!({"role":"assistant","content":[{"type":"think","text":"SECRET"},{"type":"text","text":"visible"}]}),
            ],
        ),
    ];
    for (adapter, file, records) in cases {
        let path = dir.path().join(file);
        write(
            &path,
            &(records
                .into_iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join("\n")
                + "\n{\"partial\":"),
        );
        let (session, messages) = parse(adapter, &path, dir.path());
        assert_eq!(session.external_id, "test-1", "{adapter}");
        if adapter == "codex" {
            assert_eq!(session.title, "中文 DPI-1047");
        }
        assert_eq!(messages.len(), 2, "{adapter}");
        assert_eq!(messages[0].content, "中文 DPI-1047");
        assert_eq!(messages[1].content, "visible");
    }
    let path = dir.path().join("gemini/chats/session-test.jsonl");
    write(&path,&[json!({"sessionId":"test-1","projectHash":"hash","startTime":"2026-09-14T00:00:00Z"}),json!({"id":"a","type":"user","content":"中文 DPI-1047"}),json!({"id":"b","type":"gemini","content":[{"text":"wrong"}]}),json!({"id":"b","type":"gemini","content":[{"thought":true,"text":"SECRET"},{"text":"visible"}]})].iter().map(|v|v.to_string()+"\n").collect::<String>());
    let (_, messages) = parse("gemini", &path, dir.path());
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1].content, "visible");
}

#[test]
fn search_association_revision_revoke_and_backup_are_consistent() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("sources");
    fs::create_dir(&root).unwrap();
    let project_dir = dir.path().join("project");
    write(&project_dir.join("package.json"), r#"{"name":"test"}"#);
    let core = Core::open(dir.path().join("atlas.sqlite")).unwrap();
    let project = core
        .register_project_with_origin(&project_dir, "test")
        .unwrap();
    let source = core
        .set_session_source("claude", root.to_str().unwrap(), true)
        .unwrap();
    let path = root.join("s.jsonl");
    write(&path,&json!({"type":"user","sessionId":"test-1","cwd":project_dir,"message":{"role":"user","content":"中文 DPI-1047 release certificate"}}).to_string());
    let (session, messages) = parse("claude", &path, &root);
    let fp = fingerprint(&path).unwrap();
    let worker = core.session_worker().unwrap();
    assert!(worker
        .ingest_session_cancelable(
            &source,
            session.clone(),
            &messages,
            &fp,
            &AtomicBool::new(true)
        )
        .is_err());
    assert_eq!(
        core.search_agent_sessions(Default::default())
            .unwrap()
            .total,
        0
    );
    worker
        .ingest_session(&source, session, &messages, &fp)
        .unwrap();
    for query in [
        "中文",
        "DPI-1047",
        "certificate",
        "release certificate",
        "DPI",
        "中文 DPI",
    ] {
        let result = core
            .search_agent_sessions(SessionQuery {
                query: query.into(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(result.total, 1, "{query}");
        assert_eq!(
            result.items[0].session.project_id.as_deref(),
            Some(project.id.as_str())
        );
    }
    assert!(
        core.session_file_changed(&source.id, path.to_str().unwrap(), &fp)
            .unwrap(),
        "partial file ingestion must be retried"
    );
    core.finish_session_file(
        &source.id,
        path.to_str().unwrap(),
        &fp,
        &["test-1".to_owned()].into_iter().collect(),
    )
    .unwrap();
    assert!(!core
        .session_file_changed(&source.id, path.to_str().unwrap(), &fp)
        .unwrap());
    let id = core
        .search_agent_sessions(Default::default())
        .unwrap()
        .items[0]
        .session
        .id
        .clone();
    let mut session = core.get_agent_session(&id).unwrap();
    session.title = "changed".into();
    let new = [SessionMessage {
        index: 0,
        role: "assistant".into(),
        content: "replacement".into(),
        timestamp: String::new(),
    }];
    core.ingest_session(&source, session, &new, "new").unwrap();
    assert_eq!(
        core.search_agent_sessions(SessionQuery {
            query: "certificate".into(),
            ..Default::default()
        })
        .unwrap()
        .total,
        0
    );
    assert_eq!(
        core.agent_session_messages(&id, 0, 1000)
            .unwrap()
            .items
            .len(),
        1
    );
    let backup = dir.path().join("backup.sqlite");
    core.backup_db(&backup).unwrap();
    let copied = Core::open_without_recovery(backup).unwrap();
    assert!(copied.session_sources().unwrap().iter().all(|s| !s.enabled));
    drop(copied);
    core.set_session_source("claude", &source.path, false)
        .unwrap();
    assert_eq!(
        core.search_agent_sessions(Default::default())
            .unwrap()
            .total,
        0
    );
    assert!(core.agent_session_messages(&id, 0, 40).is_err());
    assert!(path.exists());
}

#[test]
fn resume_parameters_never_accept_shell_syntax() {
    for adapter in [
        "claude",
        "codex",
        "opencode",
        "cursor-cli",
        "gemini",
        "copilot",
        "kimi",
        "qwen",
    ] {
        assert!(!resume_args(adapter, "session-123_abc").unwrap().is_empty());
        for id in [
            "",
            "--help",
            "hello;whoami",
            "$(whoami)",
            "a b",
            "a\nb",
            "a%PATH%",
        ] {
            assert!(resume_args(adapter, id).is_err(), "{adapter}: {id}");
        }
    }
}

#[test]
fn sqlite_adapters_read_ordered_visible_content() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("opencode.db");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("CREATE TABLE session(id TEXT,directory TEXT,title TEXT,time_created INTEGER,time_updated INTEGER);CREATE TABLE message(id TEXT,session_id TEXT,time_created INTEGER,data TEXT);CREATE TABLE part(id TEXT,message_id TEXT,data TEXT);INSERT INTO session VALUES('ses_1','/workspace','Title',1,2);INSERT INTO message VALUES('m','ses_1',1,'{\"role\":\"user\"}');INSERT INTO part VALUES('p','m','{\"type\":\"text\",\"text\":\"visible\"}');INSERT INTO part VALUES('q','m','{\"type\":\"reasoning\",\"text\":\"SECRET\"}');").unwrap();
    drop(db);
    let (session, messages) = parse("opencode", &path, dir.path());
    assert_eq!(session.external_id, "ses_1");
    assert_eq!(messages.len(), 1);
    fn field(n: u8, data: &[u8]) -> Vec<u8> {
        assert!(data.len() < 128);
        let mut out = vec![n * 8 + 2, data.len() as u8];
        out.extend(data);
        out
    }
    let path = dir.path().join("cursor/work/test-1/store.db");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch(
        "CREATE TABLE meta(key TEXT,value TEXT);CREATE TABLE blobs(id TEXT PRIMARY KEY,data BLOB);",
    )
    .unwrap();
    let id = |n: u8| format!("{n:02x}").repeat(32);
    let nodes = [
        (1, field(8, &[2; 32])),
        (
            2,
            field(1, &[field(1, &[3; 32]), field(2, &[4; 32])].concat()),
        ),
        (3, field(1, b"user visible")),
        (4, field(1, &field(1, b"assistant visible"))),
    ];
    for (n, bytes) in nodes {
        db.execute(
            "INSERT INTO blobs VALUES(?1,?2)",
            rusqlite::params![id(n), bytes],
        )
        .unwrap();
    }
    db.execute(
        "INSERT INTO meta VALUES('0',?1)",
        [json!({"agentId":"test-1","latestRootBlobId":id(1),"name":"Cursor"}).to_string()],
    )
    .unwrap();
    drop(db);
    let (_, messages) = parse("cursor-cli", &path, dir.path());
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].content, "user visible");
    assert_eq!(messages[1].content, "assistant visible");
}

#[test]
fn codex_titles_use_user_intent_without_altering_transcripts() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rollout-intent.jsonl");
    let lines = [
        json!({"type":"session_meta","payload":{"id":"intent-1","cwd":"/workspace"}}),
        json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<recommended_plugins>setup</recommended_plugins>"}]}}),
        json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"# Files mentioned by the user:\nimage.png\n## My request:\nFix navigation"}]}}),
    ];
    write(
        &path,
        &lines.iter().map(|v| format!("{v}\n")).collect::<String>(),
    );
    let (session, messages) = parse("codex", &path, dir.path());
    assert_eq!(session.title, "Fix navigation");
    assert_eq!(session.last_user_excerpt, "Fix navigation");
    assert_eq!(messages.len(), 2);
    assert!(messages[0].content.contains("recommended_plugins"));
}

#[test]
fn resume_rechecks_source_identity_instead_of_trusting_cached_rows() {
    use repoatlas_core::agent_sessions::adapters::validate_resume_source;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("rollout.jsonl");
    write(
        &file,
        "{\"type\":\"session_meta\",\"payload\":{\"id\":\"selected\",\"cwd\":\"/project\"}}\n",
    );
    let source = SessionSource {
        id: "source".into(),
        adapter: "codex".into(),
        path: dir.path().to_string_lossy().into_owned(),
        enabled: true,
        last_scanned_at: None,
        last_error: None,
    };
    let (mut session, _) = parse("codex", &file, dir.path());
    session.source_id = source.id.clone();
    validate_resume_source(&source, &session).unwrap();
    write(
        &file,
        "{\"type\":\"session_meta\",\"payload\":{\"id\":\"replacement\",\"cwd\":\"/project\"}}\n",
    );
    assert!(validate_resume_source(&source, &session)
        .unwrap_err()
        .to_string()
        .contains("session_source_missing"));
    let mut revoked = source.clone();
    revoked.enabled = false;
    assert!(validate_resume_source(&revoked, &session)
        .unwrap_err()
        .to_string()
        .contains("source_disabled"));
    fs::remove_file(file).unwrap();
    assert!(validate_resume_source(&source, &session).is_err());
}

#[test]
fn all_provider_resume_arguments_target_the_selected_session() {
    for (adapter, expected) in [
        ("codex", vec!["resume", "session-123"]),
        ("claude", vec!["--resume", "session-123"]),
        ("gemini", vec!["--resume", "session-123"]),
        ("qwen", vec!["--resume", "session-123"]),
        ("opencode", vec!["--session", "session-123"]),
        ("kimi", vec!["--session", "session-123"]),
        ("cursor-cli", vec!["--resume=session-123"]),
        ("copilot", vec!["--resume=session-123"]),
    ] {
        assert_eq!(
            resume_args(adapter, "session-123").unwrap(),
            expected,
            "{adapter}"
        );
    }
}
