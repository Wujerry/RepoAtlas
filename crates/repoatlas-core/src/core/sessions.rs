use super::*;
use crate::agent_sessions::{adapters, *};

pub(crate) fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS agent_session_sources(id TEXT PRIMARY KEY,adapter TEXT NOT NULL,path TEXT NOT NULL,enabled INTEGER NOT NULL DEFAULT 0,last_scanned_at TEXT,last_error TEXT,UNIQUE(adapter,path));
        CREATE TABLE IF NOT EXISTS agent_sessions(id TEXT PRIMARY KEY,source_id TEXT NOT NULL REFERENCES agent_session_sources(id) ON DELETE CASCADE,external_id TEXT NOT NULL,project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,manual INTEGER NOT NULL DEFAULT 0,revision TEXT NOT NULL,fingerprint TEXT NOT NULL,locator TEXT NOT NULL,data TEXT NOT NULL,missing INTEGER NOT NULL DEFAULT 0,UNIQUE(source_id,external_id));
        CREATE TABLE IF NOT EXISTS agent_session_files(source_id TEXT NOT NULL REFERENCES agent_session_sources(id) ON DELETE CASCADE,locator TEXT NOT NULL,fingerprint TEXT NOT NULL,PRIMARY KEY(source_id,locator));
        CREATE INDEX IF NOT EXISTS agent_sessions_project ON agent_sessions(project_id);
        CREATE INDEX IF NOT EXISTS agent_sessions_locator ON agent_sessions(source_id,locator);
        INSERT OR IGNORE INTO schema_migrations VALUES(22,datetime('now'));")?;
    Ok(())
}
impl Core {
    /// A separate connection for background history work; the desktop retains runtime ownership.
    pub fn session_worker(&self) -> Result<Self> {
        let path = self
            .conn
            .path()
            .filter(|p| !p.is_empty())
            .ok_or_else(|| Error::msg("session_index_requires_local_database"))?;
        Ok(Self {
            conn: db::open(Path::new(path))?,
            log_dir: self.log_dir.clone(),
            runtime_lock: None,
        })
    }
    fn session_index(&self) -> Result<Connection> {
        let path = self
            .conn
            .path()
            .filter(|p| !p.is_empty())
            .ok_or_else(|| Error::msg("session_index_requires_local_database"))?;
        let db = Connection::open(Path::new(path).with_file_name("session-index.sqlite"))?;
        db.busy_timeout(std::time::Duration::from_secs(2))?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA secure_delete=ON;
            CREATE TABLE IF NOT EXISTS documents(id INTEGER PRIMARY KEY,session_id TEXT NOT NULL,source_id TEXT NOT NULL,revision TEXT NOT NULL,message_index INTEGER NOT NULL,role TEXT NOT NULL,timestamp TEXT NOT NULL,content TEXT NOT NULL);
            CREATE INDEX IF NOT EXISTS session_documents ON documents(session_id,revision,message_index);
            CREATE INDEX IF NOT EXISTS source_documents ON documents(source_id);
            CREATE VIRTUAL TABLE IF NOT EXISTS session_fts USING fts5(content,content='documents',content_rowid='id',tokenize='unicode61');
            CREATE VIRTUAL TABLE IF NOT EXISTS session_trigram USING fts5(content,content='documents',content_rowid='id',tokenize='trigram');
            CREATE TRIGGER IF NOT EXISTS session_doc_insert AFTER INSERT ON documents BEGIN INSERT INTO session_fts(rowid,content) VALUES(new.id,new.content); INSERT INTO session_trigram(rowid,content) VALUES(new.id,new.content); END;
            CREATE TRIGGER IF NOT EXISTS session_doc_delete AFTER DELETE ON documents BEGIN INSERT INTO session_fts(session_fts,rowid,content) VALUES('delete',old.id,old.content); INSERT INTO session_trigram(session_trigram,rowid,content) VALUES('delete',old.id,old.content); END;")?;
        Ok(db)
    }
    pub fn session_sources(&self) -> Result<Vec<SessionSource>> {
        let mut stmt=self.conn.prepare("SELECT id,adapter,path,enabled,last_scanned_at,last_error FROM agent_session_sources ORDER BY adapter,path")?;
        let rows = stmt.query_map([], |r| {
            Ok(SessionSource {
                id: r.get(0)?,
                adapter: r.get(1)?,
                path: r.get(2)?,
                enabled: r.get(3)?,
                last_scanned_at: r.get(4)?,
                last_error: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }
    pub fn session_source_candidates(&self) -> Result<Vec<SessionSource>> {
        let mut sources = self.session_sources()?;
        for candidate in adapters::candidate_sources() {
            if !sources.iter().any(|s| {
                s.adapter == candidate.adapter
                    && paths::normalize(Path::new(&s.path))
                        == paths::normalize(Path::new(&candidate.path))
            }) {
                sources.push(candidate);
            }
        }
        Ok(sources)
    }
    pub fn set_session_source(
        &self,
        adapter: &str,
        path: &str,
        enabled: bool,
    ) -> Result<SessionSource> {
        if !adapters::ADAPTERS.contains(&adapter) {
            return Err(Error::msg("unsupported_adapter"));
        }
        if !Path::new(path).is_absolute() {
            return Err(Error::msg("source_requires_absolute_path"));
        }
        let stored = self
            .session_sources()?
            .into_iter()
            .find(|s| s.adapter == adapter && s.path == path);
        let canonical = if enabled {
            paths::canonicalize(Path::new(path))?
        } else {
            PathBuf::from(path)
        };
        if enabled && !canonical.is_dir() {
            return Err(Error::msg("source_requires_directory"));
        }
        let path = paths::path_to_string(&canonical);
        let id = stored
            .map(|s| s.id)
            .or(self
                .conn
                .query_row(
                    "SELECT id FROM agent_session_sources WHERE adapter=?1 AND path=?2",
                    params![adapter, path],
                    |r| r.get(0),
                )
                .optional()?)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("INSERT INTO agent_session_sources(id,adapter,path,enabled) VALUES(?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET enabled=excluded.enabled,last_scanned_at=NULL,last_error=NULL",params![id,adapter,path,enabled])?;
        if !enabled {
            tx.execute("DELETE FROM agent_sessions WHERE source_id=?1", [&id])?;
            tx.execute("DELETE FROM agent_session_files WHERE source_id=?1", [&id])?;
        }
        tx.commit()?;
        // Authorization is revoked first, so an index cleanup failure cannot expose data.
        if !enabled {
            let index = self.session_index()?;
            index.execute("DELETE FROM documents WHERE source_id=?1", [&id])?;
            index.execute_batch("INSERT INTO session_fts(session_fts) VALUES('optimize'); INSERT INTO session_trigram(session_trigram) VALUES('optimize'); PRAGMA wal_checkpoint(TRUNCATE);")?;
        }
        self.record_audit_event(
            "desktop",
            "set_session_source",
            "session_source",
            Some(&id),
            None,
            "success",
        )?;
        self.session_sources()?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| Error::msg("source_not_found"))
    }
    pub fn session_file_changed(
        &self,
        source_id: &str,
        path: &str,
        fingerprint: &str,
    ) -> Result<bool> {
        let count:i64=self.conn.query_row("SELECT count(*) FROM agent_session_files WHERE source_id=?1 AND locator=?2 AND fingerprint=?3",params![source_id,path,fingerprint],|r|r.get(0))?;
        Ok(count == 0)
    }
    pub fn finish_session_file(
        &self,
        source_id: &str,
        locator: &str,
        fingerprint: &str,
        ids: &HashSet<String>,
    ) -> Result<()> {
        let mut stmt = self.conn.prepare(
            "SELECT id,external_id FROM agent_sessions WHERE source_id=?1 AND locator=?2",
        )?;
        let rows = stmt
            .query_map(params![source_id, locator], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let tx = self.conn.unchecked_transaction()?;
        for (id, external) in rows {
            tx.execute(
                "UPDATE agent_sessions SET missing=?2 WHERE id=?1",
                params![id, !ids.contains(&external)],
            )?;
        }
        tx.execute("INSERT INTO agent_session_files SELECT ?1,?2,?3 WHERE EXISTS(SELECT 1 FROM agent_session_sources WHERE id=?1 AND enabled=1) ON CONFLICT(source_id,locator) DO UPDATE SET fingerprint=excluded.fingerprint",params![source_id,locator,fingerprint])?;
        tx.commit()?;
        Ok(())
    }
    pub fn ingest_session(
        &self,
        source: &SessionSource,
        session: AgentSession,
        messages: &[SessionMessage],
        fingerprint: &str,
    ) -> Result<()> {
        self.ingest_session_cancelable(
            source,
            session,
            messages,
            fingerprint,
            &std::sync::atomic::AtomicBool::new(false),
        )
    }
    pub fn ingest_session_cancelable(
        &self,
        source: &SessionSource,
        mut session: AgentSession,
        messages: &[SessionMessage],
        fingerprint: &str,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<()> {
        let enabled: bool = self.conn.query_row(
            "SELECT enabled FROM agent_session_sources WHERE id=?1",
            [&source.id],
            |r| r.get(0),
        )?;
        if !enabled {
            return Err(Error::msg("source_disabled"));
        }
        let existing:Option<(String,Option<String>,bool,String)>=self.conn.query_row("SELECT id,project_id,manual,data FROM agent_sessions WHERE source_id=?1 AND external_id=?2",params![source.id,session.external_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
        session.id = existing
            .as_ref()
            .map(|e| e.0.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        session.source_id = source.id.clone();
        session.adapter = source.adapter.clone();
        session.revision = Uuid::new_v4().to_string();
        let manual = existing.as_ref().is_some_and(|e| e.2);
        if manual {
            if let Some(old) = existing {
                session.project_id = old.1;
                if let Ok(old) = serde_json::from_str::<AgentSession>(&old.3) {
                    session.cwd = old.cwd;
                }
            }
            session.match_kind = "manual".into();
        } else {
            let projects = self.list_projects(ProjectQuery {
                include_archived: Some(true),
                limit: Some(5000),
                ..Default::default()
            })?;
            if session.cwd.is_empty()
                && ["cursor-cli", "gemini"].contains(&session.adapter.as_str())
            {
                use sha2::Digest;
                let bucket = Path::new(&session.source_locator)
                    .parent()
                    .and_then(Path::parent)
                    .and_then(Path::file_name)
                    .unwrap_or_default()
                    .to_string_lossy();
                for p in &projects {
                    let hash = if session.adapter == "cursor-cli" {
                        format!("{:x}", md5::compute(p.canonical_path.as_bytes()))
                    } else {
                        format!("{:x}", sha2::Sha256::digest(p.canonical_path.as_bytes()))
                    };
                    if hash == bucket {
                        session.cwd = p.canonical_path.clone();
                        break;
                    }
                }
            }
            let cwd = paths::canonicalize(Path::new(&session.cwd))
                .unwrap_or_else(|_| paths::normalize(Path::new(&session.cwd)));
            let matched = projects
                .iter()
                .filter(|p| {
                    !session.cwd.is_empty() && paths::is_within(&cwd, Path::new(&p.canonical_path))
                })
                .max_by_key(|p| Path::new(&p.canonical_path).components().count());
            if let Some(p) = matched {
                session.project_id = Some(p.id.clone());
                session.match_kind = if paths::is_within(Path::new(&p.canonical_path), &cwd) {
                    "exact"
                } else {
                    "child"
                }
                .into();
            } else {
                session.project_id = None;
                session.match_kind = "unlinked".into();
            }
        }
        let index = self.session_index()?;
        let tx = index.unchecked_transaction()?;
        for message in messages {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(Error::msg("canceled"));
            }
            tx.execute("INSERT INTO documents(session_id,source_id,revision,message_index,role,timestamp,content) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![session.id,source.id,session.revision,message.index,message.role,message.timestamp,message.content])?;
        }
        tx.execute("INSERT INTO documents(session_id,source_id,revision,message_index,role,timestamp,content) VALUES(?1,?2,?3,-1,'title','',?4)",params![session.id,source.id,session.revision,session.title])?;
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(Error::msg("canceled"));
        }
        tx.commit()?;
        // Publish only after its complete index generation is durable.
        let publish = rusqlite::Transaction::new_unchecked(
            &self.conn,
            rusqlite::TransactionBehavior::Immediate,
        )?;
        // A manual association made while indexing must win over the earlier snapshot.
        let latest: Option<(Option<String>, String)> = publish
            .query_row(
                "SELECT project_id,data FROM agent_sessions WHERE id=?1 AND manual=1",
                [&session.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((project_id, data)) = latest {
            session.project_id = project_id;
            session.cwd = serde_json::from_str::<AgentSession>(&data)
                .map_err(|e| Error::msg(e.to_string()))?
                .cwd;
            session.match_kind = "manual".into();
        }
        let published = publish.execute("INSERT INTO agent_sessions(id,source_id,external_id,project_id,manual,revision,fingerprint,locator,data) SELECT ?1,?2,?3,?4,?5,?6,?7,?8,?9 WHERE EXISTS(SELECT 1 FROM agent_session_sources WHERE id=?2 AND enabled=1) ON CONFLICT(id) DO UPDATE SET project_id=excluded.project_id,revision=excluded.revision,fingerprint=excluded.fingerprint,locator=excluded.locator,data=excluded.data,missing=0",params![session.id,source.id,session.external_id,session.project_id,manual,session.revision,fingerprint,session.source_locator,serde_json::to_string(&session).map_err(|e|Error::msg(e.to_string()))?])?;
        publish.commit()?;
        index.execute(
            "DELETE FROM documents WHERE session_id=?1 AND (revision<>?2 OR ?3=0)",
            params![session.id, session.revision, published],
        )?;
        Ok(())
    }
    pub fn finish_session_source(
        &self,
        id: &str,
        seen: &HashSet<String>,
        error: Option<&str>,
    ) -> Result<()> {
        if error.is_none() {
            let mut stmt = self
                .conn
                .prepare("SELECT id,locator FROM agent_sessions WHERE source_id=?1")?;
            let records = stmt
                .query_map([id], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            for (session, locator) in records {
                if !seen.contains(&locator) {
                    self.conn.execute(
                        "UPDATE agent_sessions SET missing=1 WHERE id=?1",
                        [&session],
                    )?;
                    self.conn.execute(
                        "DELETE FROM agent_session_files WHERE source_id=?1 AND locator=?2",
                        params![id, locator],
                    )?;
                }
            }
        }
        self.conn.execute(
            "UPDATE agent_session_sources SET last_scanned_at=?2,last_error=?3 WHERE id=?1",
            params![id, Utc::now().to_rfc3339(), error],
        )?;
        Ok(())
    }
    fn read_sessions(&self, id: Option<&str>) -> Result<Vec<AgentSession>> {
        let mut stmt=self.conn.prepare("SELECT a.data,a.project_id,p.display_name,a.missing FROM agent_sessions a JOIN agent_session_sources s ON s.id=a.source_id AND s.enabled=1 LEFT JOIN projects p ON p.id=a.project_id WHERE (?1 IS NULL OR a.id=?1) AND a.revision<>''")?;
        let rows = stmt.query_map([id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, bool>(3)?,
            ))
        })?;
        let installed = crate::launch::list_agents();
        let mut sessions = Vec::new();
        let sources = self.session_sources()?;
        for row in rows {
            let (data, project_id, project_name, missing) = row?;
            let mut session: AgentSession =
                serde_json::from_str(&data).map_err(|_| Error::msg("invalid_session_metadata"))?;
            session.project_id = project_id;
            session.project_name = project_name;
            session.source_missing = missing;
            session.resume_reason = if missing {
                Some("session_source_missing")
            } else if resume_args(&session.adapter, &session.external_id).is_err() {
                Some("invalid_session_id")
            } else if session.cwd.is_empty() || !Path::new(&session.cwd).is_dir() {
                Some("session_cwd_missing")
            } else if !installed.iter().any(|a| {
                a.id == if session.adapter == "opencode" {
                    "opencode-cli"
                } else {
                    &session.adapter
                }
            }) {
                Some("session_agent_missing")
            } else {
                None
            }
            .map(str::to_owned);
            if session.resume_reason.is_none()
                && sources
                    .iter()
                    .find(|s| s.id == session.source_id)
                    .is_none_or(|s| adapters::resume_environment(s).is_err())
            {
                session.resume_reason = Some("session_source_layout_unknown".into());
            }
            session.capabilities.direct_resume = session.resume_reason.is_none();
            sessions.push(session);
        }
        Ok(sessions)
    }
    pub fn get_agent_session(&self, id: &str) -> Result<AgentSession> {
        self.read_sessions(Some(id))?
            .into_iter()
            .next()
            .ok_or_else(|| Error::msg("session_not_authorized_or_missing"))
    }
    pub fn continue_agent_sessions(&self, project_id: Option<&str>) -> Result<Vec<AgentSession>> {
        let projects = self.list_projects(ProjectQuery {
            limit: Some(5000),
            ..Default::default()
        })?;
        let allowed: HashSet<_> = projects
            .iter()
            .filter(|p| !p.archived && p.availability == "ready")
            .map(|p| p.id.as_str())
            .collect();
        let mut sessions = self.read_sessions(None)?;
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(a.id.cmp(&b.id)));
        let mut seen = HashSet::new();
        Ok(sessions
            .into_iter()
            .filter(|s| {
                s.capabilities.direct_resume
                    && !s.archived
                    && s.project_id.as_deref().is_some_and(|p| {
                        allowed.contains(p)
                            && project_id.is_none_or(|id| id == p)
                            && seen.insert(p.to_owned())
                    })
            })
            .take(if project_id.is_some() { 1 } else { 6 })
            .collect())
    }
    pub fn search_agent_sessions(&self, q: SessionQuery) -> Result<SessionSearchResult> {
        if q.query.chars().count() > 512 || q.offset > 100_000 {
            return Err(Error::msg("session_query_limit"));
        }
        let limit = if q.limit == 0 { 50 } else { q.limit.min(100) };
        let parse_date = |v: &Option<String>| {
            v.as_ref()
                .map(|s| {
                    chrono::DateTime::parse_from_rfc3339(s)
                        .map(|d| d.to_utc())
                        .map_err(|_| Error::msg("invalid_session_date"))
                })
                .transpose()
        };
        let after = parse_date(&q.after)?;
        let before = parse_date(&q.before)?;
        if after.zip(before).is_some_and(|(a, b)| a >= b) {
            return Err(Error::msg("invalid_session_date_range"));
        }
        let eligible: Vec<_> = self
            .read_sessions(None)?
            .into_iter()
            .filter(|session| {
                let updated = chrono::DateTime::parse_from_rfc3339(&session.updated_at)
                    .ok()
                    .map(|d| d.to_utc());
                !((!q.archived && session.archived)
                    || q.project_id
                        .as_ref()
                        .is_some_and(|p| session.project_id.as_ref() != Some(p))
                    || q.adapter.as_ref().is_some_and(|a| a != &session.adapter)
                    || after.is_some_and(|a| updated.is_none_or(|t| t < a))
                    || before.is_some_and(|b| updated.is_none_or(|t| t >= b))
                    || (q.resumable_only && !session.capabilities.direct_resume))
            })
            .collect();
        let needle = q.query.trim().to_lowercase();
        let mut matches: HashMap<(String, String), Vec<SessionMessage>> = HashMap::new();
        let mut ranks: HashMap<(String, String), f64> = HashMap::new();
        if !needle.is_empty() {
            let index = self.session_index()?;
            index.execute_batch("CREATE TEMP TABLE scope(session_id TEXT,revision TEXT,PRIMARY KEY(session_id,revision)) WITHOUT ROWID;")?;
            let tx = index.unchecked_transaction()?;
            for session in &eligible {
                tx.execute(
                    "INSERT INTO scope VALUES(?1,?2)",
                    params![session.id, session.revision],
                )?;
            }
            tx.commit()?;
            let expression = needle
                .split_whitespace()
                .map(|s| format!("\"{}\"", s.replace('"', "\"\"")))
                .collect::<Vec<_>>()
                .join(" AND ");
            let terms: Vec<_> = needle.split_whitespace().collect();
            let short_terms =
                serde_json::to_string(&terms).map_err(|e| Error::msg(e.to_string()))?;
            for table in ["session_fts", "session_trigram"] {
                let short = terms.iter().any(|term| term.chars().count() < 3);
                if short && table == "session_fts" {
                    continue;
                }
                let select = if short {
                    "SELECT d.id,d.session_id,d.revision,0.0 AS score FROM documents d JOIN scope c ON c.session_id=d.session_id AND c.revision=d.revision WHERE NOT EXISTS(SELECT 1 FROM json_each(?1) term WHERE instr(lower(d.content),term.value)=0)".to_owned()
                } else {
                    format!("SELECT d.id,d.session_id,d.revision,bm25({table}) AS score FROM {table} JOIN documents d ON d.id={table}.rowid JOIN scope c ON c.session_id=d.session_id AND c.revision=d.revision WHERE {table} MATCH ?1")
                };
                // Bound returned text per session, not the global hit set. This keeps
                // Project/date filters and result counts correct for large histories.
                let sql=format!("WITH hits AS MATERIALIZED ({select}), ranked AS (SELECT *,row_number() OVER(PARTITION BY session_id,revision ORDER BY score,id) AS n FROM hits) SELECT r.session_id,r.revision,d.message_index,d.role,d.timestamp,d.content,r.score FROM ranked r JOIN documents d ON d.id=r.id WHERE n<=4 ORDER BY r.score,r.id");
                let mut stmt = index.prepare(&sql)?;
                let rows =
                    stmt.query_map([if short { &short_terms } else { &expression }], |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, i64>(2)?,
                            r.get::<_, String>(3)?,
                            r.get::<_, String>(4)?,
                            r.get::<_, String>(5)?,
                            r.get::<_, f64>(6)?,
                        ))
                    })?;
                for row in rows {
                    let (id, revision, n, role, timestamp, content, rank) = row?;
                    let key = (id, revision);
                    ranks
                        .entry(key.clone())
                        .and_modify(|v| *v = v.min(rank))
                        .or_insert(rank);
                    let items = matches.entry(key).or_default();
                    if n >= 0 && items.len() < 3 && !items.iter().any(|m| m.index == n as usize) {
                        let chars: Vec<char> = content.chars().collect();
                        let lower = content.to_lowercase();
                        let pos = lower
                            .find(needle.split_whitespace().next().unwrap_or(""))
                            .map(|b| lower[..b].chars().count())
                            .unwrap_or(0);
                        let start = pos.saturating_sub(80);
                        let end = (start + 320).min(chars.len());
                        items.push(SessionMessage {
                            index: n as usize,
                            role,
                            timestamp,
                            content: chars[start.min(end)..end].iter().collect(),
                        });
                    }
                }
            }
        }
        let mut items = Vec::new();
        for session in eligible {
            let key = (session.id.clone(), session.revision.clone());
            if !needle.is_empty() && !matches.contains_key(&key) {
                continue;
            }
            items.push(SessionSearchHit {
                snippets: matches.remove(&key).unwrap_or_default(),
                session,
            });
        }
        items.sort_by(|a, b| {
            let rank = |s: &AgentSession| {
                ranks
                    .get(&(s.id.clone(), s.revision.clone()))
                    .copied()
                    .unwrap_or(0.0)
            };
            rank(&a.session)
                .total_cmp(&rank(&b.session))
                .then(b.session.updated_at.cmp(&a.session.updated_at))
                .then(a.session.id.cmp(&b.session.id))
        });
        let total = items.len();
        Ok(SessionSearchResult {
            items: items.into_iter().skip(q.offset).take(limit).collect(),
            total,
        })
    }
    pub fn agent_session_messages(
        &self,
        id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<SessionMessagePage> {
        let session = self.get_agent_session(id)?;
        let db = self.session_index()?;
        let mut stmt=db.prepare("SELECT message_index,role,content,timestamp FROM documents WHERE session_id=?1 AND revision=?2 AND message_index>=?3 ORDER BY message_index LIMIT ?4")?;
        let items = stmt
            .query_map(
                params![
                    id,
                    session.revision,
                    offset.min(100_000),
                    limit.clamp(1, 100)
                ],
                |r| {
                    Ok(SessionMessage {
                        index: r.get(0)?,
                        role: r.get(1)?,
                        content: r.get(2)?,
                        timestamp: r.get(3)?,
                    })
                },
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(SessionMessagePage {
            items,
            total: session.message_count,
        })
    }
    pub fn link_agent_session(
        &self,
        id: &str,
        project_id: Option<&str>,
        cwd: Option<&str>,
    ) -> Result<()> {
        let tx = rusqlite::Transaction::new_unchecked(
            &self.conn,
            rusqlite::TransactionBehavior::Immediate,
        )?;
        let mut session = self.get_agent_session(id)?;
        if let Some(project) = project_id {
            self.get_project(project)?;
        }
        if let Some(cwd) = cwd {
            if !Path::new(cwd).is_absolute() {
                return Err(Error::msg("source_requires_absolute_path"));
            }
            let path = paths::canonicalize(Path::new(cwd))?;
            if !path.is_dir() {
                return Err(Error::msg("session_cwd_missing"));
            }
            session.cwd = paths::path_to_string(&path);
        }
        session.project_id = project_id.map(str::to_owned);
        session.match_kind = "manual".into();
        self.conn.execute(
            "UPDATE agent_sessions SET project_id=?2,manual=1,data=?3 WHERE id=?1",
            params![
                id,
                project_id,
                serde_json::to_string(&session).map_err(|e| Error::msg(e.to_string()))?
            ],
        )?;
        tx.commit()?;
        self.record_audit_event(
            "desktop",
            "link_agent_session",
            "agent_session",
            Some(id),
            None,
            "success",
        )?;
        Ok(())
    }
    pub fn agent_session_resume_spec(&self, id: &str) -> Result<SessionResumeSpec> {
        let session = self.get_agent_session(id)?;
        if let Some(reason) = session.resume_reason {
            return Err(Error::msg(reason));
        }
        let source = self
            .session_sources()?
            .into_iter()
            .find(|s| s.id == session.source_id && s.enabled)
            .ok_or_else(|| Error::msg("source_disabled"))?;
        let locator = paths::canonicalize(Path::new(&session.source_locator))
            .map_err(|_| Error::msg("session_source_missing"))?;
        if !paths::is_within(&locator, &paths::canonicalize(Path::new(&source.path))?) {
            return Err(Error::msg("source_path_escape"));
        }
        let args = resume_args(&session.adapter, &session.external_id)?;
        let env = adapters::resume_environment(&source)?;
        let program = crate::launch::session_agent_program(&session.adapter)?;
        let quote = |v: &str| {
            if cfg!(windows) {
                format!("'{}'", v.replace('\'', "''"))
            } else {
                format!("'{}'", v.replace('\'', "'\\''"))
            }
        };
        let command = if cfg!(windows) {
            format!(
                "$ErrorActionPreference = 'Stop'; Set-Location -LiteralPath {}; & {} {}",
                quote(&session.cwd),
                quote(&program),
                args.iter().map(|s| quote(s)).collect::<Vec<_>>().join(" ")
            )
        } else {
            format!(
                "cd {} && {} {}",
                quote(&session.cwd),
                quote(&program),
                args.iter().map(|s| quote(s)).collect::<Vec<_>>().join(" ")
            )
        };
        let prefix = env
            .iter()
            .map(|(k, v)| {
                if cfg!(windows) {
                    format!("$env:{k} = {}; ", quote(v))
                } else {
                    format!("export {k}={}; ", quote(v))
                }
            })
            .collect::<String>();
        let mut app = crate::launch::session_app_target(&session.adapter);
        if let Some(app) = &mut app {
            if session.adapter == "codex" {
                let default_home =
                    std::env::var_os("CODEX_HOME")
                        .map(PathBuf::from)
                        .or_else(|| {
                            std::env::var_os("USERPROFILE")
                                .or_else(|| std::env::var_os("HOME"))
                                .map(|p| PathBuf::from(p).join(".codex"))
                        });
                app.can_resume = default_home.is_some_and(|p| {
                    env.get("CODEX_HOME")
                        .is_some_and(|v| paths::normalize(Path::new(v)) == paths::normalize(&p))
                });
            }
        }
        Ok(SessionResumeSpec {
            app,
            agent: session.adapter,
            args,
            cwd: session.cwd,
            command: format!("{prefix}{command}"),
            env,
        })
    }
    pub fn resume_agent_session(&self, id: &str) -> Result<()> {
        self.resume_agent_session_target(id, "cli")
    }
    pub fn resume_agent_session_target(&self, id: &str, target: &str) -> Result<()> {
        let spec = self.agent_session_resume_spec(id)?;
        let session = self.get_agent_session(id)?;
        let source = self
            .session_sources()?
            .into_iter()
            .find(|source| source.id == session.source_id && source.enabled)
            .ok_or_else(|| Error::msg("source_disabled"))?;
        adapters::validate_resume_source(&source, &session)?;
        let result = if target == "app" {
            if !spec.app.as_ref().is_some_and(|a| a.can_resume) {
                return Err(Error::msg("session_app_resume_unsupported"));
            }
            let session = self.get_agent_session(id)?;
            crate::launch::resume_session_app(&spec.agent, &session.external_id, &spec.cwd)
        } else if target == "cli" {
            crate::launch::resume_session(&spec.cwd, &spec.agent, &spec.args, &spec.env)
        } else {
            return Err(Error::msg("invalid_session_target"));
        };
        self.record_audit_event(
            "desktop",
            "resume_agent_session",
            "agent_session",
            Some(id),
            None,
            if result.is_ok() { "success" } else { "failed" },
        )?;
        result
    }
    pub fn rebuild_session_index(&self) -> Result<()> {
        let path = Path::new(
            self.conn
                .path()
                .ok_or_else(|| Error::msg("session_index_requires_local_database"))?,
        )
        .with_file_name("session-index.sqlite");
        // This fixed application-owned cache is never an external source path.
        for p in [
            path.clone(),
            path.with_file_name("session-index.sqlite-wal"),
            path.with_file_name("session-index.sqlite-shm"),
        ] {
            if p.is_file() {
                fs::remove_file(p)?;
            }
        }
        self.session_index()?;
        self.conn
            .execute("UPDATE agent_sessions SET fingerprint='',revision=''", [])?;
        self.conn.execute("DELETE FROM agent_session_files", [])?;
        self.conn
            .execute("UPDATE agent_session_sources SET last_scanned_at=NULL", [])?;
        Ok(())
    }
}
