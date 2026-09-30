use super::*;
use crate::subscriptions::{self, SubscriptionUsage};
use serde::Serialize;

pub(crate) fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS subscription_usage(provider TEXT PRIMARY KEY,enabled INTEGER NOT NULL DEFAULT 0,generation TEXT NOT NULL,snapshot TEXT);")?;
    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations VALUES(23,datetime('now'))",
        [],
    )?;
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentUsageSummary {
    pub adapter: String,
    pub sessions: u64,
    pub measured_sessions: u64,
    pub total_tokens: Option<u64>,
    pub estimated_usd: Option<f64>,
    pub priced_tokens: u64,
}
impl Core {
    /// A display preference, separate from account authorization. Update only this
    /// key so a simultaneous theme/font change cannot be overwritten.
    pub fn set_usage_navigation(&self, hidden: Vec<String>) -> Result<AppSettings> {
        let hidden: Vec<_> = subscriptions::PROVIDERS
            .iter()
            .filter(|provider| hidden.iter().any(|item| item == **provider))
            .map(|provider| (*provider).to_owned())
            .collect();
        self.put_setting(
            "usageNavHiddenProviders",
            &serde_json::to_string(&hidden).map_err(|_| Error::msg("usage_encode_failed"))?,
        )?;
        self.settings()
    }
    /// One metadata-only query: no CLI detection, transcript decode or file I/O.
    pub fn session_usage_summary(&self) -> Result<Vec<AgentUsageSummary>> {
        let mut stmt = self.conn.prepare("WITH ranked AS (SELECT a.data,row_number() OVER (PARTITION BY json_extract(a.data,'$.adapter'),a.external_id ORDER BY json_extract(a.data,'$.updatedAt') DESC,a.id) n FROM agent_sessions a JOIN agent_session_sources s ON s.id=a.source_id AND s.enabled=1 WHERE a.revision<>'' AND a.missing=0) SELECT json_extract(data,'$.adapter'),count(*),count(json_extract(data,'$.usage.totalTokens')),sum(json_extract(data,'$.usage.totalTokens')),sum(CASE WHEN json_extract(data,'$.usage.cost.pricedTokens')>0 THEN json_extract(data,'$.usage.cost.usd') END),coalesce(sum(json_extract(data,'$.usage.cost.pricedTokens')),0) FROM ranked WHERE n=1 GROUP BY json_extract(data,'$.adapter')")?;
        let rows = stmt.query_map([], |r| {
            Ok(AgentUsageSummary {
                adapter: r.get(0)?,
                sessions: r.get(1)?,
                measured_sessions: r.get(2)?,
                total_tokens: r.get(3)?,
                estimated_usd: r.get(4)?,
                priced_tokens: r.get(5)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }
    pub fn subscription_usage(&self) -> Result<Vec<SubscriptionUsage>> {
        subscriptions::PROVIDERS
            .iter()
            .map(|provider| {
                let row: Option<(bool, Option<String>)> = self
                    .conn
                    .query_row(
                        "SELECT enabled,snapshot FROM subscription_usage WHERE provider=?1",
                        [provider],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()?;
                let Some((true, snapshot)) = row else {
                    return Ok(SubscriptionUsage::empty(provider, false));
                };
                snapshot
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .map(Ok)
                    .unwrap_or_else(|| Ok(SubscriptionUsage::empty(provider, true)))
            })
            .collect()
    }
    pub fn set_subscription_enabled(&self, provider: &str, enabled: bool) -> Result<()> {
        subscriptions::validate_provider(provider)?;
        self.conn.execute("INSERT INTO subscription_usage(provider,enabled,generation) VALUES(?1,?2,?3) ON CONFLICT(provider) DO UPDATE SET enabled=excluded.enabled,generation=excluded.generation,snapshot=NULL",params![provider,enabled,Uuid::new_v4().to_string()])?;
        self.record_audit_event(
            "desktop",
            "subscription_usage_authorization",
            "provider",
            Some(provider),
            Some(if enabled { "enabled" } else { "disabled" }),
            "success",
        )?;
        Ok(())
    }
    pub fn prepare_subscription_refresh(
        &self,
        provider: &str,
        force: bool,
    ) -> Result<Option<String>> {
        subscriptions::validate_provider(provider)?;
        let row: Option<(String,Option<String>)> = self.conn.query_row("SELECT generation,snapshot FROM subscription_usage WHERE provider=?1 AND enabled=1",[provider],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        let Some((generation, snapshot)) = row else {
            return Ok(None);
        };
        if snapshot
            .and_then(|s| serde_json::from_str::<SubscriptionUsage>(&s).ok())
            .is_some_and(|s| s.fresh(force))
        {
            return Ok(None);
        }
        Ok(Some(generation))
    }
    pub fn publish_subscription_usage(
        &self,
        generation: &str,
        mut snapshot: SubscriptionUsage,
    ) -> Result<()> {
        // A revoked/re-enabled connection cannot be repopulated by an old request.
        if snapshot.status != "ready" {
            let prior: Option<String> = self.conn.query_row("SELECT snapshot FROM subscription_usage WHERE provider=?1 AND generation=?2 AND enabled=1",params![snapshot.provider,generation],|r|r.get(0)).optional()?.flatten();
            if let Some(prior) =
                prior.and_then(|s| serde_json::from_str::<SubscriptionUsage>(&s).ok())
            {
                snapshot.windows = prior.windows;
                snapshot.fetched_at = prior.fetched_at;
                snapshot.plan = prior.plan;
            }
        }
        let data =
            serde_json::to_string(&snapshot).map_err(|_| Error::msg("usage_encode_failed"))?;
        self.conn.execute("UPDATE subscription_usage SET snapshot=?1 WHERE provider=?2 AND generation=?3 AND enabled=1",params![data,snapshot.provider,generation])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_preferences_survive_reopen_without_authorizing_accounts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.sqlite");
        {
            let core = Core::open(&path).unwrap();
            assert!(core
                .settings()
                .unwrap()
                .usage_nav_hidden_providers
                .is_empty());
            let mut settings = core.settings().unwrap();
            settings.theme = "dark".into();
            core.update_settings(settings).unwrap();
            let saved = core
                .set_usage_navigation(vec!["codex".into(), "codex".into(), "unknown".into()])
                .unwrap();
            assert_eq!(saved.theme, "dark");
            assert_eq!(saved.usage_nav_hidden_providers, vec!["codex"]);
            // An appearance form opened before the toggle carries an old copy.
            let stale_form = AppSettings {
                theme: "light".into(),
                ..Default::default()
            };
            assert_eq!(
                core.update_settings(stale_form)
                    .unwrap()
                    .usage_nav_hidden_providers,
                vec!["codex"]
            );
        }
        let core = Core::open(&path).unwrap();
        assert_eq!(
            core.settings().unwrap().usage_nav_hidden_providers,
            vec!["codex"]
        );
        assert!(core
            .subscription_usage()
            .unwrap()
            .iter()
            .all(|s| !s.enabled));
        let exported = core.export_json().unwrap();
        let restored = Core::open_in_memory().unwrap();
        restored.import_json(&exported).unwrap();
        assert_eq!(
            restored.settings().unwrap().usage_nav_hidden_providers,
            vec!["codex"]
        );
        assert!(restored
            .subscription_usage()
            .unwrap()
            .iter()
            .all(|s| !s.enabled));
        let legacy: AppSettings =
            serde_json::from_str(r#"{"theme":"system","locale":"en"}"#).unwrap();
        assert!(legacy.usage_nav_hidden_providers.is_empty());
    }
    #[test]
    fn subscription_access_is_opt_in_and_revocation_rejects_late_results() {
        let core = Core::open_in_memory().unwrap();
        assert!(core
            .prepare_subscription_refresh("codex", false)
            .unwrap()
            .is_none());
        core.set_subscription_enabled("codex", true).unwrap();
        let generation = core
            .prepare_subscription_refresh("codex", false)
            .unwrap()
            .unwrap();
        core.set_subscription_enabled("codex", false).unwrap();
        core.set_subscription_enabled("codex", true).unwrap();
        let mut snapshot = SubscriptionUsage::empty("codex", true);
        snapshot.plan = Some("Old account".into());
        core.publish_subscription_usage(&generation, snapshot)
            .unwrap();
        assert!(core.subscription_usage().unwrap()[0].plan.is_none());
        assert!(core.set_subscription_enabled("unknown", true).is_err());
    }
    #[test]
    fn refresh_failure_keeps_timestamped_cache_and_backup_drops_authorization() {
        let core = Core::open_in_memory().unwrap();
        core.set_subscription_enabled("claude", true).unwrap();
        let generation = core
            .prepare_subscription_refresh("claude", false)
            .unwrap()
            .unwrap();
        let mut snapshot = subscriptions::parse(
            "claude",
            &serde_json::json!({"five_hour":{"utilization":42}}),
        )
        .unwrap();
        snapshot.fetched_at = Some("2026-09-30T00:00:00Z".into());
        core.publish_subscription_usage(&generation, snapshot)
            .unwrap();
        let mut error = SubscriptionUsage::empty("claude", true);
        error.status = "network_error".into();
        core.publish_subscription_usage(&generation, error).unwrap();
        let value = core.subscription_usage().unwrap().remove(1);
        assert_eq!(value.status, "network_error");
        assert_eq!(value.windows[0].used_percent, Some(42.));
        assert_eq!(value.fetched_at.as_deref(), Some("2026-09-30T00:00:00Z"));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("backup.sqlite");
        core.backup_db(&path).unwrap();
        let copy = Core::open_without_recovery(path).unwrap();
        assert!(copy
            .subscription_usage()
            .unwrap()
            .iter()
            .all(|s| !s.enabled && s.windows.is_empty()));
        assert!(core.subscription_usage().unwrap()[1].enabled);
    }
}
