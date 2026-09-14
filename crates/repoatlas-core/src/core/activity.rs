//! Bounded, cache-only activity queries. Git collection is prepared here and executed outside Core.
use super::Core;
use crate::{
    ActivityHistoryDaySummary, ActivityHistoryItem, ActivityHistoryResponse, CachedGitCommit,
    Error, Result,
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRefreshRequest {
    pub project_id: Option<String>,
    pub start_at: i64,
    pub end_at: i64,
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRefreshStatus {
    pub id: String,
    pub state: String,
    pub completed: usize,
    pub total: usize,
    pub commits: usize,
    pub failures: Vec<String>,
}

struct RefreshJob {
    request: HistoryRefreshRequest,
    cancel: Arc<AtomicBool>,
    status: Arc<Mutex<HistoryRefreshStatus>>,
}
#[derive(Default)]
pub struct HistoryRefreshManager {
    job: Mutex<Option<RefreshJob>>,
}

impl HistoryRefreshManager {
    pub fn status(&self) -> HistoryRefreshStatus {
        self.job
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|j| j.status.lock().unwrap_or_else(|e| e.into_inner()).clone())
            .unwrap_or_else(|| HistoryRefreshStatus {
                state: "idle".into(),
                ..Default::default()
            })
    }
    pub fn cancel(&self, id: &str) {
        if let Some(j) = self.job.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            if j.status.lock().unwrap_or_else(|e| e.into_inner()).id == id {
                j.cancel.store(true, Ordering::Relaxed);
            }
        }
    }
    pub fn start(
        &self,
        core: Arc<Mutex<Core>>,
        request: HistoryRefreshRequest,
    ) -> Result<HistoryRefreshStatus> {
        let mut slot = self
            .job
            .lock()
            .map_err(|_| Error::msg("history scheduler unavailable"))?;
        if let Some(job) = slot.as_ref() {
            let status = job.status.lock().unwrap_or_else(|e| e.into_inner()).clone();
            if status.state == "running" {
                if job.request.project_id == request.project_id
                    && job.request.start_at == request.start_at
                    && job.request.end_at == request.end_at
                {
                    return Ok(status);
                }
                return Err(Error::msg(
                    "A history refresh is already running; cancel it before changing the range",
                ));
            }
        }
        let targets = core
            .lock()
            .map_err(|_| Error::msg("Core unavailable"))?
            .prepare_history(
                request.project_id.as_deref(),
                request.start_at,
                request.end_at,
                request.force,
            )?;
        let initial = HistoryRefreshStatus {
            id: uuid::Uuid::new_v4().to_string(),
            state: if targets.is_empty() {
                "completed"
            } else {
                "running"
            }
            .into(),
            total: targets.len(),
            ..Default::default()
        };
        let status = Arc::new(Mutex::new(initial.clone()));
        let cancel = Arc::new(AtomicBool::new(false));
        *slot = Some(RefreshJob {
            request: request.clone(),
            status: status.clone(),
            cancel: cancel.clone(),
        });
        if targets.is_empty() {
            return Ok(initial);
        }
        std::thread::spawn(move || {
            let next = AtomicUsize::new(0);
            std::thread::scope(|scope| {
                for _ in 0..2 {
                    let targets = &targets;
                    let next = &next;
                    let core = &core;
                    let cancel = &cancel;
                    let status = &status;
                    let request = &request;
                    scope.spawn(move || loop {
                        if cancel.load(Ordering::Relaxed) {
                            break;
                        }
                        let Some(target) = targets.get(next.fetch_add(1, Ordering::Relaxed)) else {
                            break;
                        };
                        let mut head = None;
                        let result = (|| -> Result<()> {
                            head = crate::git::history_head(&target.path, cancel)?;
                            if !(target.covered && target.previous_head == head) {
                                if let Some(sha) = head.as_deref() {
                                    let mut skip = 0;
                                    loop {
                                        if cancel.load(Ordering::Relaxed) {
                                            return Err(Error::msg("canceled"));
                                        }
                                        let batch = crate::git::history_batch(
                                            &target.path,
                                            sha,
                                            request.start_at,
                                            request.end_at,
                                            skip,
                                            cancel,
                                        )?;
                                        let n = batch.len();
                                        core.lock()
                                            .map_err(|_| Error::msg("Core unavailable"))?
                                            .persist_history_batch(target, &batch)?;
                                        status.lock().unwrap_or_else(|e| e.into_inner()).commits +=
                                            n;
                                        skip += n;
                                        if n < 500 {
                                            break;
                                        }
                                    }
                                }
                            }
                            Ok(())
                        })();
                        if cancel.load(Ordering::Relaxed) {
                            break;
                        }
                        // Persist only generic errors: command output and project contents never enter logs.
                        let error = result
                            .as_ref()
                            .err()
                            .map(|_| "Local Git history update failed");
                        let saved = core
                            .lock()
                            .map_err(|_| Error::msg("Core unavailable"))
                            .and_then(|c| {
                                c.finish_history(
                                    target,
                                    request.start_at,
                                    request.end_at,
                                    head.as_deref(),
                                    error,
                                )
                            });
                        let mut s = status.lock().unwrap_or_else(|e| e.into_inner());
                        s.completed += 1;
                        if result.is_err() || saved.is_err() {
                            s.failures.push(target.project_id.clone());
                        }
                    });
                }
            });
            let mut s = status.lock().unwrap_or_else(|e| e.into_inner());
            s.state = if cancel.load(Ordering::Relaxed) {
                "canceled"
            } else if s.failures.is_empty() {
                "completed"
            } else {
                "partial"
            }
            .into();
        });
        Ok(initial)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ActivityQuery {
    pub start_at: i64,
    pub end_at: i64,
    pub project_id: Option<String>,
    pub category: Option<String>,
    pub search: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityDayRange {
    pub date: String,
    pub start_at: i64,
    pub end_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySummary {
    pub days: Vec<ActivityHistoryDaySummary>,
    pub open_counts: Vec<u64>,
    pub project_counts: Vec<u64>,
    pub latest_at: Option<i64>,
    pub coverage: Vec<HistoryCoverage>,
    pub projects: Vec<ActivityProject>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityProject {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryCoverage {
    pub project_id: String,
    pub start_at: i64,
    pub end_at: i64,
    pub checked_at: i64,
    pub head: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct HistoryTarget {
    pub project_id: String,
    pub path: std::path::PathBuf,
    pub previous_head: Option<String>,
    pub covered: bool,
}

// All arms expose the same columns, with numeric timestamps matching expression indexes.
const SOURCES: [&str; 3] = [
    "SELECT 'git:'||project_id||':'||sha id, project_id, 'git' category, 'commit' kind, subject title, message detail, commit_date occurred_at, unixepoch(commit_date) ts, 'git_history' source, NULL run_id, sha commit_sha, short_sha commit_short_sha, author_name, author_date, NULL task_status, NULL task_duration_ms FROM project_git_commits",
    "SELECT 'run:'||id id, project_id, 'task' category, 'task_run' kind, executable title, NULL detail, started_at occurred_at, unixepoch(started_at) ts, 'repoatlas' source, id run_id, NULL commit_sha, NULL commit_short_sha, NULL author_name, NULL author_date, status task_status, MAX(0,(unixepoch(finished_at)-unixepoch(started_at))*1000) task_duration_ms FROM task_runs",
    "SELECT 'event:'||id id, project_id, CASE kind WHEN 'git' THEN 'git' WHEN 'task' THEN 'task' WHEN 'ide' THEN 'tool' WHEN 'agent' THEN 'tool' WHEN 'terminal' THEN 'tool' WHEN 'explorer' THEN 'tool' WHEN 'open' THEN 'open' ELSE 'maintenance' END category, kind, title, detail, created_at occurred_at, unixepoch(created_at) ts, 'repoatlas' source, NULL run_id, NULL commit_sha, NULL commit_short_sha, NULL author_name, NULL author_date, NULL task_status, NULL task_duration_ms FROM project_events",
];

fn map_item(r: &rusqlite::Row<'_>) -> rusqlite::Result<ActivityHistoryItem> {
    Ok(ActivityHistoryItem {
        id: r.get("id")?,
        project_id: r.get("project_id")?,
        project_name: r.get("project_name")?,
        canonical_path: r.get("canonical_path")?,
        category: r.get("category")?,
        kind: r.get("kind")?,
        title: r.get("title")?,
        detail: r.get("detail")?,
        occurred_at: r.get("occurred_at")?,
        source: r.get("source")?,
        run_id: r.get("run_id")?,
        commit_sha: r.get("commit_sha")?,
        commit_short_sha: r.get("commit_short_sha")?,
        author_name: r.get("author_name")?,
        author_date: r.get("author_date")?,
        task_status: r.get("task_status")?,
        task_duration_ms: r.get("task_duration_ms")?,
    })
}

impl ActivityQuery {
    fn validate(&self) -> Result<()> {
        if self.start_at >= self.end_at || self.end_at.saturating_sub(self.start_at) > 32 * 86400 {
            return Err(Error::msg(
                "activity range must be between one second and 32 days",
            ));
        }
        if self.search.as_ref().is_some_and(|s| s.len() > 512) {
            return Err(Error::msg("activity search is too long"));
        }
        if self
            .category
            .as_deref()
            .is_some_and(|s| !["git", "task", "tool", "open", "maintenance"].contains(&s))
        {
            return Err(Error::msg("invalid activity category"));
        }
        Ok(())
    }
    fn key(&self) -> String {
        serde_json::json!([
            self.start_at,
            self.end_at,
            self.project_id,
            self.category,
            self.search.as_deref().unwrap_or("").trim()
        ])
        .to_string()
    }
}

impl Core {
    pub fn activity_page(&self, q: &ActivityQuery) -> Result<ActivityHistoryResponse> {
        q.validate()?;
        let key = q.key();
        let (cursor_time, cursor_id) = if let Some(c) = &q.cursor {
            let (k, t, id): (String, i64, String) =
                serde_json::from_str(c).map_err(|_| Error::msg("invalid activity cursor"))?;
            if k != key {
                return Err(Error::msg("activity cursor belongs to another query"));
            }
            (t, id)
        } else {
            (i64::MAX, String::new())
        };
        let limit = q.limit.unwrap_or(50).clamp(1, 100);
        let project_filter = if q.project_id.is_some() {
            "AND a.project_id=?3"
        } else {
            ""
        };
        let category_filter = if q.category.is_some() {
            "AND a.category=?4"
        } else {
            ""
        };
        let search = q.search.as_deref().unwrap_or("").trim().to_lowercase();
        let search_filter = if search.is_empty() {
            ""
        } else {
            "AND (instr(lower(a.title),?5)>0 OR instr(lower(coalesce(a.detail,'')),?5)>0 OR instr(lower(coalesce(a.commit_sha,'')),?5)>0 OR instr(lower(p.display_name),?5)>0 OR instr(lower(p.canonical_path),?5)>0)"
        };
        // Branch-local LIMIT bounds the final merge to at most 303 rows.
        let branches = SOURCES.iter().zip(["git", "run", "event"]).map(|(source, index)| { let suffix = if q.project_id.is_some() { "project" } else { "time" }; let source = format!("{source} INDEXED BY idx_activity_{index}_{suffix}"); format!("SELECT * FROM (SELECT a.*, p.display_name project_name,p.canonical_path FROM ({source}) a CROSS JOIN projects p ON p.id=a.project_id WHERE a.ts>=?1 AND a.ts<?2 {project_filter} {category_filter} {search_filter} AND (a.ts<?6 OR (a.ts=?6 AND a.id<?7)) ORDER BY a.ts DESC,a.id DESC LIMIT ?8)") }).collect::<Vec<_>>().join(" UNION ALL ");
        let sql = format!("SELECT id,project_id,category,kind,title,NULL detail,occurred_at,source,run_id,commit_sha,commit_short_sha,author_name,author_date,task_status,task_duration_ms,project_name,canonical_path,ts FROM ({branches}) ORDER BY ts DESC,id DESC LIMIT ?8");
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let mut rows = stmt.query(params![
            q.start_at,
            q.end_at,
            q.project_id,
            q.category,
            search,
            cursor_time,
            cursor_id,
            (limit + 1) as i64
        ])?;
        let mut items = Vec::new();
        let mut times = Vec::new();
        while let Some(r) = rows.next()? {
            items.push(map_item(r)?);
            times.push(r.get::<_, i64>("ts")?);
        }
        let next_cursor = if items.len() > limit {
            items.pop();
            times.pop();
            Some(
                serde_json::json!([key, *times.last().unwrap(), &items.last().unwrap().id])
                    .to_string(),
            )
        } else {
            None
        };
        Ok(ActivityHistoryResponse {
            days: vec![],
            items,
            next_cursor,
            selected_date: None,
            total_days: 0,
        })
    }

    pub fn activity_detail(&self, id: &str) -> Result<ActivityHistoryItem> {
        let (source, filter, value, project) = if let Some(rest) = id.strip_prefix("git:") {
            let (pid, sha) = rest
                .rsplit_once(':')
                .ok_or_else(|| Error::msg("invalid activity id"))?;
            (SOURCES[0], "sha=?1 AND project_id=?2", sha, Some(pid))
        } else if let Some(value) = id.strip_prefix("run:") {
            (SOURCES[1], "id=?1 AND ?2 IS NULL", value, None)
        } else if let Some(value) = id.strip_prefix("event:") {
            (SOURCES[2], "id=?1 AND ?2 IS NULL", value, None)
        } else {
            return Err(Error::msg("invalid activity id"));
        };
        let sql=format!("SELECT a.*,p.display_name project_name,p.canonical_path FROM ({source} WHERE {filter}) a JOIN projects p ON p.id=a.project_id");
        self.conn
            .query_row(&sql, params![value, project], map_item)
            .map_err(Into::into)
    }

    pub fn activity_summary(
        &self,
        days: &[ActivityDayRange],
        project_id: Option<&str>,
    ) -> Result<ActivitySummary> {
        if days.is_empty() || days.len() > 31 {
            return Err(Error::msg("expected 1 to 31 calendar days"));
        }
        for (i, d) in days.iter().enumerate() {
            if d.start_at >= d.end_at
                || d.end_at - d.start_at > 90000
                || (i > 0 && days[i - 1].end_at != d.start_at)
            {
                return Err(Error::msg("invalid calendar boundaries"));
            }
        }
        let mut summaries = Vec::new();
        let mut open_counts = Vec::new();
        let mut project_counts = Vec::new();
        let project_filter = if project_id.is_some() {
            "AND project_id=?3"
        } else {
            "AND ?3 IS NULL"
        };
        // Whole hours use incrementally maintained counts; unusual timezone edges use bounded raw reads.
        let edges=format!("SELECT project_id,'git' category,COUNT(*) n FROM project_git_commits WHERE unixepoch(commit_date)>=?1 AND unixepoch(commit_date)<?2 {project_filter} GROUP BY project_id UNION ALL SELECT project_id,'task' category,COUNT(*) n FROM task_runs WHERE unixepoch(started_at)>=?1 AND unixepoch(started_at)<?2 {project_filter} GROUP BY project_id UNION ALL SELECT project_id,CASE kind WHEN 'git' THEN 'git' WHEN 'task' THEN 'task' WHEN 'ide' THEN 'tool' WHEN 'agent' THEN 'tool' WHEN 'terminal' THEN 'tool' WHEN 'explorer' THEN 'tool' WHEN 'open' THEN 'open' ELSE 'maintenance' END category,COUNT(*) n FROM project_events WHERE unixepoch(created_at)>=?1 AND unixepoch(created_at)<?2 {project_filter} GROUP BY project_id,category");
        let mut edge_stmt = self.conn.prepare_cached(&edges)?;
        for d in days {
            let mut day = ActivityHistoryDaySummary {
                date: d.date.clone(),
                total_count: 0,
                git_count: 0,
                task_count: 0,
                tool_count: 0,
                maintenance_count: 0,
            };
            let mut opens = 0;
            let mut projects = std::collections::HashSet::new();
            let mut add = |pid: String, cat: String, n: u64| {
                projects.insert(pid);
                day.total_count += n;
                match cat.as_str() {
                    "git" => day.git_count += n,
                    "task" => day.task_count += n,
                    "tool" => day.tool_count += n,
                    "open" => opens += n,
                    _ => day.maintenance_count += n,
                };
            };
            let mut ranges = vec![(d.start_at, d.end_at)];
            for (span, table) in [
                (86400, "activity_day_counts"),
                (21600, "activity_sixhour_counts"),
                (3600, "activity_hour_counts"),
            ] {
                let mut remainder = vec![];
                let index = if project_id.is_some() {
                    format!("{table}_project")
                } else {
                    format!("sqlite_autoindex_{table}_1")
                };
                let mut stmt=self.conn.prepare_cached(&format!("SELECT project_id,category,SUM(n) FROM {table} INDEXED BY {index} WHERE bucket_at>=?1 AND bucket_at<?2 {project_filter} GROUP BY project_id,category"))?;
                for (start, end) in ranges {
                    let ceil = start.div_euclid(span) * span
                        + if start.rem_euclid(span) == 0 { 0 } else { span };
                    let floor = end.div_euclid(span) * span;
                    if ceil < floor {
                        let rows = stmt.query_map(params![ceil, floor, project_id], |r| {
                            Ok((
                                r.get::<_, String>(0)?,
                                r.get::<_, String>(1)?,
                                r.get::<_, u64>(2)?,
                            ))
                        })?;
                        for row in rows {
                            let (pid, cat, n) = row?;
                            add(pid, cat, n);
                        }
                        if start < ceil {
                            remainder.push((start, ceil));
                        }
                        if floor < end {
                            remainder.push((floor, end));
                        }
                    } else {
                        remainder.push((start, end));
                    }
                }
                ranges = remainder;
            }
            for (start, end) in ranges {
                if start >= end {
                    continue;
                }
                let rows = edge_stmt.query_map(params![start, end, project_id], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, u64>(2)?,
                    ))
                })?;
                for row in rows {
                    let (pid, cat, n) = row?;
                    add(pid, cat, n);
                }
            }
            summaries.push(day);
            open_counts.push(opens);
            project_counts.push(projects.len() as u64);
        }
        let mut latest_at: Option<i64> = None;
        for (table, time) in [
            ("project_git_commits", "commit_date"),
            ("task_runs", "started_at"),
            ("project_events", "created_at"),
        ] {
            let filter = if project_id.is_some() {
                "WHERE project_id=?1"
            } else {
                "WHERE ?1 IS NULL"
            };
            let value: Option<i64> = self.conn.query_row(
                &format!("SELECT MAX(unixepoch({time})) FROM {table} {filter}"),
                params![project_id],
                |r| r.get(0),
            )?;
            latest_at = latest_at.max(value);
        }
        let coverage=self.conn.prepare("SELECT project_id,start_at,end_at,checked_at,head,error FROM git_history_coverage WHERE (?1 IS NULL OR project_id=?1)")?.query_map(params![project_id],|r|Ok(HistoryCoverage{project_id:r.get(0)?,start_at:r.get(1)?,end_at:r.get(2)?,checked_at:r.get(3)?,head:r.get(4)?,error:r.get(5)?}))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let projects = self
            .conn
            .prepare("SELECT id,display_name FROM projects ORDER BY display_name,id")?
            .query_map([], |r| {
                Ok(ActivityProject {
                    id: r.get(0)?,
                    name: r.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(ActivitySummary {
            days: summaries,
            open_counts,
            project_counts,
            latest_at,
            coverage,
            projects,
        })
    }

    /// Legacy Rust entry point remains cache-only. Desktop uses explicit local-day boundaries.
    pub fn get_activity_history(
        &self,
        date: Option<&str>,
        project_id: Option<&str>,
        category: Option<&str>,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<ActivityHistoryResponse> {
        let date = date
            .map(str::to_owned)
            .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
        let start = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d")
            .map_err(|_| Error::msg("invalid date"))?
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp();
        let mut page = self.activity_page(&ActivityQuery {
            start_at: start,
            end_at: start + 86400,
            project_id: project_id.map(str::to_owned),
            category: category.map(str::to_owned),
            cursor: cursor.map(str::to_owned),
            limit: Some(limit),
            search: None,
        })?;
        page.days = self
            .activity_summary(
                &[ActivityDayRange {
                    date: date.clone(),
                    start_at: start,
                    end_at: start + 86400,
                }],
                project_id,
            )?
            .days;
        page.total_days = 1;
        page.selected_date = Some(date);
        Ok(page)
    }

    pub fn prepare_history(
        &self,
        project_id: Option<&str>,
        start: i64,
        end: i64,
        force: bool,
    ) -> Result<Vec<HistoryTarget>> {
        if start >= end || end - start > 366 * 86400 {
            return Err(Error::msg("history range must be at most 366 days"));
        }
        let now = chrono::Utc::now().timestamp();
        let mut stmt=self.conn.prepare("SELECT id,canonical_path FROM projects WHERE vcs_kind='git' AND availability<>'unavailable' AND (?1 IS NULL OR id=?1)")?;
        let rows = stmt.query_map(params![project_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        let mut targets = vec![];
        for row in rows {
            let (id, path) = row?;
            let previous:Option<(i64,Option<String>,Option<String>)>=self.conn.query_row("SELECT checked_at,head,error FROM git_history_coverage WHERE project_id=?1 AND start_at<=?2 AND end_at>=?3 ORDER BY checked_at DESC LIMIT 1",params![id,start,end],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
            if !force && previous.as_ref().is_some_and(|p| now - p.0 < 300) {
                continue;
            }
            targets.push(HistoryTarget {
                project_id: id,
                path: path.into(),
                previous_head: previous.as_ref().and_then(|p| p.1.clone()),
                covered: previous.is_some_and(|p| p.2.is_none()),
            });
        }
        Ok(targets)
    }

    pub fn persist_history_batch(
        &self,
        target: &HistoryTarget,
        commits: &[CachedGitCommit],
    ) -> Result<()> {
        self.check_history_target(target)?;
        let tx = self.conn.unchecked_transaction()?;
        {
            let mut stmt=tx.prepare_cached("INSERT INTO project_git_commits(id,project_id,sha,short_sha,subject,author_name,author_date,commit_date,message,cached_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10) ON CONFLICT(project_id,sha) DO NOTHING")?;
            for c in commits {
                stmt.execute(params![
                    format!("{}:{}", target.project_id, c.sha),
                    target.project_id,
                    c.sha,
                    c.short_sha,
                    c.subject,
                    c.author_name,
                    c.author_date,
                    c.commit_date,
                    c.message,
                    c.cached_at
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
    fn check_history_target(&self, target: &HistoryTarget) -> Result<()> {
        let path: String = self.conn.query_row(
            "SELECT canonical_path FROM projects WHERE id=?1",
            params![target.project_id],
            |r| r.get(0),
        )?;
        if std::path::Path::new(&path) != target.path {
            return Err(Error::msg(
                "Project location changed during history collection",
            ));
        }
        Ok(())
    }
    pub fn finish_history(
        &self,
        target: &HistoryTarget,
        start: i64,
        end: i64,
        head: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        self.check_history_target(target)?;
        self.conn.execute("INSERT INTO git_history_coverage(project_id,start_at,end_at,checked_at,head,error) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(project_id,start_at,end_at) DO UPDATE SET checked_at=excluded.checked_at,head=excluded.head,error=excluded.error",params![target.project_id,start,end,chrono::Utc::now().timestamp(),head,error])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(projects: usize, events: usize) -> Core {
        seed_fixture(Core::open_in_memory().unwrap(), projects, events)
    }
    fn seed_fixture(core: Core, projects: usize, events: usize) -> Core {
        let tx = core.conn.unchecked_transaction().unwrap();
        for i in 0..projects {
            tx.execute("INSERT INTO projects(id,canonical_path,display_name,vcs_kind,availability,origin,created_at,updated_at) VALUES(?1,?1,?1,'git','available','test','2026-09-01T00:00:00Z','2026-09-01T00:00:00Z')",params![format!("p{i}")]).unwrap();
        }
        {
            let mut stmt=tx.prepare("INSERT INTO project_events(id,project_id,kind,title,detail,created_at) VALUES(?1,?2,'open',?3,'searchable body',datetime(1788220800+?4,'unixepoch'))").unwrap();
            for i in 0..events {
                stmt.execute(params![
                    format!("e{i:08}"),
                    format!("p{}", i % projects),
                    format!("record {i}"),
                    (i % (30 * 86400)) as i64
                ])
                .unwrap();
            }
        }
        tx.commit().unwrap();
        core
    }
    #[test]
    fn activity_pagination_search_and_details_are_complete() {
        let c = fixture(2, 205);
        let mut q = ActivityQuery {
            start_at: 1788220800,
            end_at: 1788307200,
            limit: Some(50),
            ..Default::default()
        };
        let first = c.activity_page(&q).unwrap();
        assert_eq!(first.items.len(), 50);
        assert!(first.items[0].detail.is_none());
        let mut ids = std::collections::HashSet::new();
        loop {
            let page = c.activity_page(&q).unwrap();
            for i in page.items {
                assert!(ids.insert(i.id));
            }
            q.cursor = page.next_cursor;
            if q.cursor.is_none() {
                break;
            }
        }
        assert_eq!(ids.len(), 205);
        q.search = Some("record 0".into());
        let page = c.activity_page(&q).unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(
            c.activity_detail(&page.items[0].id)
                .unwrap()
                .detail
                .as_deref(),
            Some("searchable body")
        );
        q.cursor = first.next_cursor;
        assert!(c.activity_page(&q).is_err());
        q.cursor = None;
        q.search = Some("' OR 1=1 --".into());
        assert!(c.activity_page(&q).unwrap().items.is_empty());
    }
    #[test]
    fn activity_same_sha_different_projects_and_numeric_time() {
        let c = fixture(2, 0);
        for (p, date) in [
            ("p0", "2026-09-01T01:00:00+08:00"),
            ("p1", "2026-08-31T17:00:00Z"),
        ] {
            c.conn.execute("INSERT INTO project_git_commits VALUES(?1,?2,'abc','abc','same commit','author',?3,?3,'body',?3)",params![p,p,date]).unwrap();
        }
        let q = ActivityQuery {
            start_at: 1788192000,
            end_at: 1788278400,
            ..Default::default()
        };
        let p = c.activity_page(&q).unwrap();
        assert_eq!(p.items.len(), 2);
        assert_ne!(p.items[0].id, p.items[1].id);
        assert_eq!(
            c.activity_detail(&p.items[0].id).unwrap().detail.as_deref(),
            Some("body")
        );
    }
    #[test]
    fn activity_summary_counts_open_records_and_zero_days() {
        let c = fixture(2, 205);
        let summary = c
            .activity_summary(
                &[
                    ActivityDayRange {
                        date: "2026-09-01".into(),
                        start_at: 1788220800,
                        end_at: 1788307200,
                    },
                    ActivityDayRange {
                        date: "2026-09-02".into(),
                        start_at: 1788307200,
                        end_at: 1788393600,
                    },
                ],
                None,
            )
            .unwrap();
        assert_eq!(summary.days[0].total_count, 205);
        assert_eq!(summary.open_counts, [205, 0]);
        assert_eq!(summary.project_counts, [2, 0]);
        assert_eq!(summary.days[1].total_count, 0);
    }
    #[test]
    fn activity_coverage_empty_repo_and_path_race() {
        let c = fixture(1, 0);
        let target = c.prepare_history(None, 1, 2, false).unwrap().remove(0);
        c.finish_history(&target, 1, 2, None, None).unwrap();
        assert!(c.prepare_history(None, 1, 2, false).unwrap().is_empty());
        c.conn
            .execute("UPDATE projects SET canonical_path='moved'", [])
            .unwrap();
        assert!(c.persist_history_batch(&target, &[]).is_err());
    }
    #[test]
    fn activity_counts_follow_changes_and_partial_hour_boundaries() {
        let c = fixture(2, 205);
        let day = ActivityDayRange {
            date: "2026-09-01".into(),
            start_at: 1788220800,
            end_at: 1788307200,
        };
        c.conn
            .execute(
                "UPDATE project_events SET kind='ide' WHERE id='e00000000'",
                [],
            )
            .unwrap();
        c.conn
            .execute("DELETE FROM project_events WHERE id='e00000001'", [])
            .unwrap();
        let summary = c.activity_summary(&[day.clone()], None).unwrap();
        assert_eq!(summary.days[0].total_count, 204);
        assert_eq!(summary.days[0].tool_count, 1);
        assert_eq!(summary.open_counts[0], 203);
        let edge = ActivityDayRange {
            date: day.date,
            start_at: 1788220900,
            end_at: 1788305500,
        };
        assert_eq!(
            c.activity_summary(&[edge], None).unwrap().days[0].total_count,
            105
        );
        c.conn
            .execute("DELETE FROM projects WHERE id='p0'", [])
            .unwrap();
        assert_eq!(
            c.conn
                .query_row(
                    "SELECT COUNT(*) FROM activity_hour_counts WHERE project_id='p0'",
                    [],
                    |r| r.get::<_, u64>(0)
                )
                .unwrap(),
            0
        );
    }
    #[test]
    fn activity_scheduler_reports_failure_without_blocking_core() {
        let core = Arc::new(Mutex::new(fixture(3, 0)));
        let scheduler = HistoryRefreshManager::default();
        let request = HistoryRefreshRequest {
            project_id: None,
            start_at: 1,
            end_at: 2,
            force: true,
        };
        let started = scheduler.start(core.clone(), request).unwrap();
        assert_eq!(started.total, 3);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while scheduler.status().state == "running" {
            assert!(std::time::Instant::now() < deadline);
            // A bad path must not hold the lock while trying to execute a child.
            assert!(core
                .lock()
                .unwrap()
                .activity_page(&ActivityQuery {
                    start_at: 1,
                    end_at: 2,
                    ..Default::default()
                })
                .is_ok());
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let status = scheduler.status();
        assert_eq!(status.state, "partial");
        assert_eq!(status.failures.len(), 3);
        assert!(core
            .lock()
            .unwrap()
            .prepare_history(None, 1, 2, false)
            .unwrap()
            .is_empty());
    }
    #[test]
    fn activity_migrates_existing_history_once() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("legacy.sqlite");
        let c = seed_fixture(Core::open(&path).unwrap(), 2, 205);
        // Reconstruct the pre-count schema without changing source records.
        for label in ["hour", "sixhour", "day"] {
            for table in ["project_git_commits", "task_runs", "project_events"] {
                for operation in ["insert", "update", "delete"] {
                    c.conn
                        .execute_batch(&format!(
                            "DROP TRIGGER activity_{label}_{table}_{operation};"
                        ))
                        .unwrap();
                }
            }
            c.conn
                .execute_batch(&format!("DROP TABLE activity_{label}_counts;"))
                .unwrap();
        }
        c.conn
            .execute(
                "DELETE FROM schema_migrations WHERE version BETWEEN 18 AND 20",
                [],
            )
            .unwrap();
        drop(c);
        for _ in 0..2 {
            let reopened = Core::open(&path).unwrap();
            let days = [ActivityDayRange {
                date: "2026-09-01".into(),
                start_at: 1788220800,
                end_at: 1788307200,
            }];
            assert_eq!(
                reopened.activity_summary(&days, None).unwrap().days[0].total_count,
                205
            );
            assert_eq!(
                reopened
                    .conn
                    .query_row("SELECT COUNT(*) FROM project_events", [], |row| row
                        .get::<_, i64>(0))
                    .unwrap(),
                205
            );
        }
    }
    #[test]
    #[ignore = "release performance fixture; run explicitly"]
    fn activity_release_benchmark() {
        for (projects, events) in [(100, 100_000), (1000, 1_000_000)] {
            let directory = tempfile::tempdir().unwrap();
            let database = std::env::var_os("REPOATLAS_ACTIVITY_BENCH_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| directory.path().to_path_buf())
                .join(format!("history-{projects}-{events}.sqlite"));
            let existing = database.exists();
            let c = Core::open(&database).unwrap();
            let c = if existing {
                c
            } else {
                seed_fixture(c, projects, events)
            };
            // Move two thirds into the other source tables, retaining the total fixture size.
            if !existing {
                let transaction = c.conn.unchecked_transaction().unwrap();
                c.conn.execute_batch("INSERT INTO project_git_commits SELECT 'git-'||id,project_id,id,id,title,'QA',created_at,created_at,detail,created_at FROM project_events WHERE CAST(substr(id,2) AS INTEGER)%3=0; INSERT INTO task_runs(id,project_id,kind,executable,argv_json,cwd,status,log_path,started_at,finished_at) SELECT 'run-'||id,project_id,'test','cargo','[]','fixture','succeeded','fixture.log',created_at,created_at FROM project_events WHERE CAST(substr(id,2) AS INTEGER)%3=1; DELETE FROM project_events WHERE CAST(substr(id,2) AS INTEGER)%3 IN (0,1);").unwrap();
                transaction.commit().unwrap();
            }
            let actual: i64 = c.conn.query_row("SELECT (SELECT COUNT(*) FROM project_events)+(SELECT COUNT(*) FROM project_git_commits)+(SELECT COUNT(*) FROM task_runs)", [], |r| r.get(0)).unwrap();
            assert_eq!(actual, events as i64, "reusable fixture size changed");
            println!("fixture_ready projects={projects} events={events}");
            let mut page_times = vec![];
            let mut search_times = vec![];
            let mut summary_times = vec![];
            let days = (0..30)
                .map(|i| ActivityDayRange {
                    date: format!("day-{i}"),
                    start_at: 1788192000 + i * 86400,
                    end_at: 1788192000 + (i + 1) * 86400,
                })
                .collect::<Vec<_>>();
            for _ in 0..20 {
                let mut q = ActivityQuery {
                    start_at: 1788220800,
                    end_at: 1788307200,
                    ..Default::default()
                };
                let t = std::time::Instant::now();
                c.activity_page(&q).unwrap();
                page_times.push(t.elapsed().as_secs_f64() * 1000.);
                q.search = Some("record 1234".into());
                let t = std::time::Instant::now();
                c.activity_page(&q).unwrap();
                search_times.push(t.elapsed().as_secs_f64() * 1000.);
                let t = std::time::Instant::now();
                c.activity_summary(&days, None).unwrap();
                summary_times.push(t.elapsed().as_secs_f64() * 1000.);
            }
            page_times.sort_by(f64::total_cmp);
            search_times.sort_by(f64::total_cmp);
            summary_times.sort_by(f64::total_cmp);
            println!("projects={projects} events={events} page_p95_ms={:.2} search_p95_ms={:.2} summary_p95_ms={:.2}",page_times[18],search_times[18],summary_times[18]);
            assert!(page_times[18] < 100., "page budget");
            assert!(search_times[18] < 300., "search budget");
            assert!(summary_times[18] < 200., "summary budget");
        }
    }
}
