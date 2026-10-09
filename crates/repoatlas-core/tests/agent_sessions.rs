use repoatlas_core::{
    agent_sessions::{
        adapters::{fingerprint, LocalAdapter, SessionAdapter},
        *,
    },
    Core, ProjectPatch,
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
fn codex_large_image_records_preserve_text_usage_and_search() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("sessions");
    let path = root.join("rollout-images.jsonl");
    let image = "a".repeat(9 * 1024 * 1024);
    let records = [
        json!({"type":"session_meta","payload":{"id":"large-images","cwd":dir.path()}}),
        json!({"type":"event_msg","payload":{"type":"user_message","message":"Find the image bug","images":[image]}}),
        json!({"type":"response_item","payload":{"type":"message","role":"user","content":[
            {"type":"input_text","text":"Find the image bug"},
            {"type":"input_image","image_url":format!("data:image/png;base64,{image}")},
            {"type":"input_text","text":"Keep text after the image too"}
        ]}}),
        json!({"type":"response_item","payload":{"type":"reasoning","encrypted_content":image}}),
        json!({"type":"response_item","payload":{"type":"function_call","arguments":image}}),
        json!({"type":"response_item","payload":{"type":"function_call_output","output":image}}),
        json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Image handled"}]}}),
        json!({"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"output_tokens":20,"total_tokens":120}}}}),
    ];
    write(
        &path,
        &records
            .iter()
            .map(|v| v.to_string() + "\n")
            .collect::<String>(),
    );
    let original = fingerprint(&path).unwrap();
    let (session, messages) = parse("codex", &path, &root);
    assert_eq!(session.external_id, "large-images");
    assert_eq!(session.usage.as_ref().unwrap().total_tokens, 120);
    assert_eq!(messages.len(), 2);
    assert_eq!(
        messages[0].content,
        "Find the image bug\nKeep text after the image too"
    );
    assert_eq!(messages[1].content, "Image handled");
    let core = Core::open(dir.path().join("app.sqlite")).unwrap();
    let source = core
        .set_session_source("codex", root.to_str().unwrap(), true)
        .unwrap();
    core.ingest_session(&source, session, &messages, &original)
        .unwrap();
    assert_eq!(
        core.search_agent_sessions(SessionQuery {
            query: "image bug".into(),
            ..Default::default()
        })
        .unwrap()
        .total,
        1
    );
    assert_eq!(fingerprint(&path).unwrap(), original);
}

#[test]
fn recorded_usage_survives_indexing_without_reopening_transcripts() {
    let dir = tempfile::tempdir().unwrap();
    let core = Core::open(dir.path().join("app.sqlite")).unwrap();
    for (folder, total) in [("one", 220), ("two", 330)] {
        let root = dir.path().join(folder);
        fs::create_dir(&root).unwrap();
        let source = core
            .set_session_source("codex", root.to_str().unwrap(), true)
            .unwrap();
        let path = root.join("rollout-usage.jsonl");
        let records = [
            json!({"type":"session_meta","payload":{"id":"same-session","cwd":dir.path()}}),
            json!({"type":"turn_context","payload":{"model":"gpt-5.4"}}),
            json!({"type":"event_msg","payload":{"type":"user_message","message":"Token fixture"}}),
            json!({"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":total-20,"output_tokens":20,"total_tokens":total}}}}),
        ];
        write(
            &path,
            &records
                .iter()
                .map(|v| v.to_string() + "\n")
                .collect::<String>(),
        );
        let (mut session, messages) = parse("codex", &path, &root);
        assert_eq!(session.usage.as_ref().unwrap().total_tokens, total);
        session.updated_at = if folder == "one" {
            "2026-09-29T00:00:00Z"
        } else {
            "2026-09-30T00:00:00Z"
        }
        .into();
        core.ingest_session(&source, session, &messages, &fingerprint(&path).unwrap())
            .unwrap();
        // Cache-only summary must not reopen the file or invoke an Agent CLI.
        fs::remove_file(path).unwrap();
    }
    let summary = core.session_usage_summary().unwrap();
    assert_eq!(summary.len(), 1);
    assert_eq!(summary[0].sessions, 1);
    assert_eq!(summary[0].total_tokens, Some(330));
    assert_eq!(summary[0].priced_tokens, 330);
    assert!((summary[0].estimated_usd.unwrap() - 0.001075).abs() < 1e-9);
    core.set_session_source("codex", dir.path().join("two").to_str().unwrap(), false)
        .unwrap();
    assert_eq!(
        core.session_usage_summary().unwrap()[0].total_tokens,
        Some(220)
    );
    core.set_session_source("codex", dir.path().join("one").to_str().unwrap(), false)
        .unwrap();
    assert!(core.session_usage_summary().unwrap().is_empty());
}

#[test]
fn token_adapters_preserve_source_semantics_and_legacy_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let cases = [
        (
            "claude",
            "claude/session.jsonl",
            json!({"type":"assistant","message":{"id":"m","role":"assistant","content":"visible","usage":{"input_tokens":20,"cache_read_input_tokens":100,"cache_creation_input_tokens":30,"output_tokens":10}}}),
            160,
        ),
        (
            "gemini",
            "gemini/session-test.jsonl",
            json!({"id":"m","type":"gemini","content":"visible","tokens":{"input":100,"output":20,"cached":30,"thoughts":10,"total":130}}),
            130,
        ),
        (
            "copilot",
            "copilot/test/events.jsonl",
            json!({"id":"m","type":"assistant.usage","data":{"inputTokens":100,"outputTokens":20}}),
            120,
        ),
        (
            "qwen",
            "qwen/chats/test.jsonl",
            json!({"type":"assistant","message":{"id":"m","role":"assistant","content":"visible","usage":{"input_tokens":100,"output_tokens":20}}}),
            120,
        ),
        (
            "kimi",
            "kimi/sessions/test/wire/main/wire.jsonl",
            json!({"message":{"type":"StatusUpdate","payload":{"token_usage":{"input_other":20,"input_cache_read":100,"input_cache_creation":30,"output":10}}}}),
            160,
        ),
    ];
    for (adapter, file, record, total) in cases {
        let path = dir.path().join(file);
        write(
            &path,
            &(json!({"sessionId":"usage-fixture"}).to_string() + "\n" + &record.to_string() + "\n"),
        );
        let (session, _) = parse(adapter, &path, dir.path());
        assert_eq!(
            session.usage.as_ref().unwrap().total_tokens,
            total,
            "{adapter}"
        );
        let mut legacy = serde_json::to_value(session).unwrap();
        legacy.as_object_mut().unwrap().remove("usage");
        assert!(serde_json::from_value::<AgentSession>(legacy)
            .unwrap()
            .usage
            .is_none());
    }
    let path = dir.path().join("opencode.db");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("CREATE TABLE session(id TEXT,directory TEXT,title TEXT,time_created INTEGER,time_updated INTEGER);CREATE TABLE message(id TEXT,session_id TEXT,time_created INTEGER,data TEXT);CREATE TABLE part(id TEXT,message_id TEXT,data TEXT);INSERT INTO session VALUES('s','/workspace','Title',1,2);INSERT INTO message VALUES('m','s',1,'{\"role\":\"assistant\",\"tokens\":{\"input\":20,\"output\":10,\"cache\":{\"read\":100,\"write\":30}}}');INSERT INTO part VALUES('p','m','{\"type\":\"text\",\"text\":\"first\"}');INSERT INTO part VALUES('q','m','{\"type\":\"text\",\"text\":\"second\"}');INSERT INTO message VALUES('tool-only','s',2,'{\"role\":\"assistant\",\"tokens\":{\"input\":10,\"output\":10}}');").unwrap();
    drop(db);
    assert_eq!(
        parse("opencode", &path, dir.path())
            .0
            .usage
            .unwrap()
            .total_tokens,
        180
    );
}

fn index_codex_session(core: &Core, source: &SessionSource, cwd: &Path) -> AgentSession {
    let path = Path::new(&source.path).join("rollout-selected.jsonl");
    write(
        &path,
        &format!(
            "{}\n{}\n",
            json!({"type":"session_meta","payload":{"id":"selected","cwd":cwd}}),
            json!({"type":"event_msg","payload":{"type":"user_message","message":"Initial request"}}),
        ),
    );
    let (session, messages) = parse("codex", &path, Path::new(&source.path));
    core.ingest_session(source, session, &messages, &fingerprint(&path).unwrap())
        .unwrap();
    core.search_agent_sessions(Default::default())
        .unwrap()
        .items
        .remove(0)
        .session
}

#[test]
fn session_search_finds_current_project_name_and_paths_without_fabricating_messages() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("sessions");
    fs::create_dir(&root).unwrap();
    let project_dir = dir.path().join("checkout-locator");
    write(&project_dir.join("package.json"), r#"{"name":"example"}"#);
    let core = Core::open(dir.path().join("atlas.sqlite")).unwrap();
    let project = core
        .register_project_with_origin(&project_dir, "test")
        .unwrap();
    core.update_project(
        &project.id,
        ProjectPatch {
            display_name: Some("Atlas 中文 Project".into()),
            ..Default::default()
        },
    )
    .unwrap();
    let source = core
        .set_session_source("codex", root.to_str().unwrap(), true)
        .unwrap();
    let session = index_codex_session(&core, &source, &project_dir);
    let replacement = dir.path().join("explicit-replacement");
    fs::create_dir(&replacement).unwrap();
    core.link_agent_session(&session.id, Some(&project.id), replacement.to_str())
        .unwrap();

    for query in [
        "ATLAS",
        "中文",
        "checkout-locator",
        "explicit-replacement",
        project_dir.to_str().unwrap(),
    ] {
        let result = core
            .search_agent_sessions(SessionQuery {
                query: query.into(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(result.total, 1, "{query}");
        assert_eq!(result.items[0].session.id, session.id);
        assert!(
            result.items[0].snippets.is_empty(),
            "metadata is not a transcript message"
        );
    }
    assert_eq!(
        core.search_agent_sessions(SessionQuery {
            query: "Atlas".into(),
            project_id: Some("another-project".into()),
            ..Default::default()
        })
        .unwrap()
        .total,
        0
    );
    assert_eq!(
        core.search_agent_sessions(SessionQuery {
            query: "Atlas".into(),
            adapter: Some("claude".into()),
            ..Default::default()
        })
        .unwrap()
        .total,
        0
    );
    core.update_project(
        &project.id,
        ProjectPatch {
            display_name: Some("Renamed Project".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        core.search_agent_sessions(SessionQuery {
            query: "Atlas".into(),
            ..Default::default()
        })
        .unwrap()
        .total,
        0
    );
    core.set_session_source("codex", &source.path, false)
        .unwrap();
    assert_eq!(
        core.search_agent_sessions(SessionQuery {
            query: "checkout-locator".into(),
            ..Default::default()
        })
        .unwrap()
        .total,
        0
    );
}

#[test]
fn session_search_snippets_locate_original_text_and_large_message_context() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("sessions");
    fs::create_dir(&root).unwrap();
    let core = Core::open(dir.path().join("atlas.sqlite")).unwrap();
    let source = core
        .set_session_source("codex", root.to_str().unwrap(), true)
        .unwrap();
    let session = index_codex_session(&core, &source, dir.path());
    let messages = vec![
        SessionMessage {
            index: 100_004,
            role: "user".into(),
            timestamp: "2026-09-30T00:00:00Z".into(),
            content: "Previous message".into(),
        },
        SessionMessage {
            index: 100_005,
            role: "assistant".into(),
            timestamp: "2026-09-30T00:00:01Z".into(),
            content: format!("{} precise DPI-1047 中文 message", "İ".repeat(600)),
        },
        SessionMessage {
            index: 100_006,
            role: "user".into(),
            timestamp: "2026-09-30T00:00:02Z".into(),
            content: format!("{} café final context", "padding ".repeat(200)),
        },
    ];
    core.ingest_session(&source, session.clone(), &messages, "snippets")
        .unwrap();
    for (query, expected_index, expected_text) in [
        ("DPI-1047", 100_005, "DPI-1047"),
        ("中文", 100_005, "中文"),
        ("cafe", 100_006, "café"),
    ] {
        let result = core
            .search_agent_sessions(SessionQuery {
                query: query.into(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(result.total, 1, "{query}");
        assert_eq!(
            result.items[0].snippets.len(),
            1,
            "duplicate FTS hits must merge"
        );
        let snippet = &result.items[0].snippets[0];
        assert_eq!(snippet.index, expected_index);
        assert!(
            snippet.content.contains(expected_text),
            "{query}: {}",
            snippet.content
        );
        assert!(snippet.content.chars().count() <= 320);
        let context = core
            .agent_session_messages(&session.id, snippet.index, 1)
            .unwrap();
        assert_eq!(context.items.len(), 1);
        let message = &context.items[0];
        assert_eq!(message.index, snippet.index);
        assert_eq!(message.role, snippet.role);
        assert_eq!(message.timestamp, snippet.timestamp);
        assert!(message.content.contains(snippet.content.trim_matches('…')));
    }
}

#[test]
fn session_resume_rejects_invalid_cwds_without_falling_back_to_linked_project() {
    for kind in [
        "empty",
        "relative",
        "missing-relative",
        "missing-absolute",
        "file",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("sessions");
        fs::create_dir(&root).unwrap();
        let project_dir = dir.path().join("project");
        write(&project_dir.join("package.json"), r#"{"name":"example"}"#);
        let core = Core::open(dir.path().join("atlas.sqlite")).unwrap();
        let project = core
            .register_project_with_origin(&project_dir, "test")
            .unwrap();
        let source = core
            .set_session_source("codex", root.to_str().unwrap(), true)
            .unwrap();
        let session = index_codex_session(&core, &source, &project_dir);
        let (mut parsed, messages) = parse("codex", Path::new(&session.source_locator), &root);
        let cwd = match kind {
            "empty" => String::new(),
            "relative" => ".".into(),
            "missing-relative" => "missing-relative-directory".into(),
            "missing-absolute" => dir
                .path()
                .join("missing-directory")
                .to_string_lossy()
                .into_owned(),
            _ => session.source_locator.clone(),
        };
        parsed.cwd = cwd.clone();
        core.ingest_session(&source, parsed.clone(), &messages, "cwd")
            .unwrap();
        // A valid Project link must never silently replace the recorded cwd.
        core.link_agent_session(&session.id, Some(&project.id), None)
            .unwrap();
        assert_eq!(
            core.agent_session_resume_spec(&session.id)
                .unwrap_err()
                .to_string(),
            "session_cwd_missing",
            "{cwd}"
        );
        assert_eq!(
            core.resume_agent_session(&session.id)
                .unwrap_err()
                .to_string(),
            "session_cwd_missing",
            "{cwd}"
        );
    }
}

#[test]
fn session_resume_rejects_stale_identity_missing_files_and_revoked_sources_before_launch() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("sessions");
    fs::create_dir(&root).unwrap();
    let core = Core::open(dir.path().join("atlas.sqlite")).unwrap();
    let source = core
        .set_session_source("codex", root.to_str().unwrap(), true)
        .unwrap();
    let session = index_codex_session(&core, &source, dir.path());
    let file = Path::new(&session.source_locator);
    let original = fs::read_to_string(file).unwrap();
    write(file, &original.replace("selected", "replacement"));
    assert_eq!(
        core.resume_agent_session(&session.id)
            .unwrap_err()
            .to_string(),
        "session_source_missing"
    );
    fs::remove_file(file).unwrap();
    for remove_root in [false, true] {
        if remove_root {
            fs::remove_dir(&root).unwrap();
        }
        let cached = core.get_agent_session(&session.id).unwrap();
        assert!(cached.source_missing);
        assert!(!cached.capabilities.direct_resume);
        assert_eq!(
            core.agent_session_resume_spec(&session.id)
                .unwrap_err()
                .to_string(),
            "session_source_missing"
        );
        assert_eq!(
            core.resume_agent_session(&session.id)
                .unwrap_err()
                .to_string(),
            "session_source_missing"
        );
        assert_eq!(
            core.search_agent_sessions(SessionQuery {
                query: "Initial request".into(),
                ..Default::default()
            })
            .unwrap()
            .total,
            1,
            "missing originals retain authorized cached search"
        );
    }
    core.set_session_source("codex", &source.path, false)
        .unwrap();
    assert_eq!(
        core.resume_agent_session(&session.id)
            .unwrap_err()
            .to_string(),
        "session_not_authorized_or_missing"
    );
    assert_eq!(
        core.agent_session_resume_spec("unknown-session")
            .unwrap_err()
            .to_string(),
        "session_not_authorized_or_missing"
    );
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
