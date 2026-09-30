use super::*;
use repoatlas_core::subscriptions::{self, SubscriptionUsage};
use std::sync::LazyLock;

// At most one request per provider across desktop windows. Work runs outside the
// shared Core lock; a slow subscription endpoint cannot stall project navigation.
static FLIGHTS: LazyLock<HashMap<&'static str, Mutex<()>>> = LazyLock::new(|| {
    subscriptions::PROVIDERS
        .iter()
        .map(|provider| (*provider, Mutex::new(())))
        .collect()
});

#[tauri::command]
pub async fn set_usage_navigation(
    state: State<'_, Arc<AppState>>,
    hidden: Vec<String>,
) -> Result<AppSettings, String> {
    let core = state.core.clone();
    tauri::async_runtime::spawn_blocking(move || {
        core.lock()
            .map_err(err_to_string)?
            .set_usage_navigation(hidden)
            .map_err(err_to_string)
    })
    .await
    .map_err(err_to_string)?
}

#[tauri::command]
pub async fn subscription_usage(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<SubscriptionUsage>, String> {
    let core = state.core.clone();
    tauri::async_runtime::spawn_blocking(move || {
        core.lock()
            .map_err(err_to_string)?
            .subscription_usage()
            .map_err(err_to_string)
    })
    .await
    .map_err(err_to_string)?
}
#[tauri::command]
pub async fn set_subscription_enabled(
    state: State<'_, Arc<AppState>>,
    provider: String,
    enabled: bool,
) -> Result<(), String> {
    let core = state.core.clone();
    tauri::async_runtime::spawn_blocking(move || {
        core.lock()
            .map_err(err_to_string)?
            .set_subscription_enabled(&provider, enabled)
            .map_err(err_to_string)
    })
    .await
    .map_err(err_to_string)?
}
#[tauri::command]
pub async fn refresh_subscription_usage(
    state: State<'_, Arc<AppState>>,
    provider: String,
    force: bool,
) -> Result<Vec<SubscriptionUsage>, String> {
    subscriptions::validate_provider(&provider).map_err(err_to_string)?;
    let core = state.core.clone();
    tauri::async_runtime::spawn_blocking(move || {
        // Concurrent callers wait for the same provider's publication, then use
        // its fresh cache instead of returning an obsolete snapshot immediately.
        let _flight = FLIGHTS
            .get(provider.as_str())
            .ok_or("usage_unsupported_provider")?
            .lock()
            .map_err(err_to_string)?;
        let generation = core
            .lock()
            .map_err(err_to_string)?
            .prepare_subscription_refresh(&provider, force)
            .map_err(err_to_string)?;
        if let Some(generation) = generation {
            let snapshot = subscriptions::fetch(&provider);
            core.lock()
                .map_err(err_to_string)?
                .publish_subscription_usage(&generation, snapshot)
                .map_err(err_to_string)?;
        }
        core.lock()
            .map_err(err_to_string)?
            .subscription_usage()
            .map_err(err_to_string)
    })
    .await
    .map_err(err_to_string)?
}
#[tauri::command]
pub async fn session_usage_summary(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<repoatlas_core::AgentUsageSummary>, String> {
    let core = state.core.clone();
    tauri::async_runtime::spawn_blocking(move || {
        core.lock()
            .map_err(err_to_string)?
            .session_usage_summary()
            .map_err(err_to_string)
    })
    .await
    .map_err(err_to_string)?
}
