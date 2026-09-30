//! Desktop window transport only. Search, authorization and Agent resume stay in Core.
//! Events are wake-up hints; requests remain queued until the recipient acknowledges them.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow, WebviewWindowBuilder, WindowEvent};

const MAIN: &str = "main";
const QUICK_SEARCH: &str = "quick-search";
const PENDING_EVENT: &str = "quick-search://pending";
const STATUS_EVENT: &str = "quick-search://status";
const MAX_PENDING: usize = 32;
const SHUTDOWN: u64 = u64::MAX;

#[derive(Clone, Copy)]
struct Generation(u64);

#[derive(Default)]
struct Lifecycle(AtomicU64);

impl Lifecycle {
    // Called at request ingress, never inside the scheduled worker. Shutdown and
    // generation share one atomic so a concurrent request cannot undo shutdown.
    fn request(&self) -> Result<Generation, String> {
        self.0
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |generation| {
                (generation < SHUTDOWN - 1).then(|| generation + 1)
            })
            .map(|previous| Generation(previous + 1))
            .map_err(|_| "RepoAtlas is shutting down".into())
    }

    fn current(&self, generation: Generation) -> bool {
        self.0.load(Ordering::SeqCst) == generation.0
    }

    fn shutdown(&self) {
        self.0.store(SHUTDOWN, Ordering::SeqCst);
    }

    fn is_shutdown(&self) -> bool {
        self.0.load(Ordering::SeqCst) == SHUTDOWN
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QuickSearchStatus {
    shortcut: &'static str,
    registered: bool,
    error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum QuickSearchTarget {
    Project {
        #[serde(rename = "projectId")]
        project_id: String,
    },
    Sessions {
        #[serde(rename = "sessionId", skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        sources: Option<bool>,
        #[serde(rename = "messageIndex", skip_serializing_if = "Option::is_none")]
        message_index: Option<u32>,
    },
}

impl QuickSearchTarget {
    fn validate(&self) -> Result<(), String> {
        if matches!(
            self,
            Self::Sessions {
                session_id: None,
                message_index: Some(_),
                ..
            }
        ) {
            return Err("A message index requires a session ID".into());
        }
        let id = match self {
            Self::Project { project_id } => Some(project_id),
            Self::Sessions { session_id, .. } => session_id.as_ref(),
        };
        if id.is_some_and(|id| {
            id.trim().is_empty() || id.len() > 256 || id.chars().any(char::is_control)
        }) {
            return Err("Invalid quick-search navigation ID".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "action", rename_all = "kebab-case")]
pub enum QuickSearchRequest {
    Focus {
        #[serde(rename = "requestId")]
        request_id: String,
    },
    OpenMain {
        #[serde(rename = "requestId")]
        request_id: String,
        target: QuickSearchTarget,
    },
}

impl QuickSearchRequest {
    fn id(&self) -> &str {
        match self {
            Self::Focus { request_id } | Self::OpenMain { request_id, .. } => request_id,
        }
    }
}

#[derive(Default)]
struct Mailboxes {
    focus: Option<QuickSearchRequest>,
    navigation: VecDeque<QuickSearchRequest>,
}

impl Mailboxes {
    fn focus(&mut self) {
        // Latest-only slot, but every new show has its own identity. An in-flight
        // handler can acknowledge its old ID without consuming a later invocation.
        self.focus = Some(QuickSearchRequest::Focus {
            request_id: uuid::Uuid::new_v4().to_string(),
        });
    }

    fn navigate(&mut self, target: QuickSearchTarget) -> Result<String, String> {
        target.validate()?;
        if self.navigation.len() >= MAX_PENDING {
            return Err("Quick-search navigation queue is full; wait for the main window".into());
        }
        let request_id = uuid::Uuid::new_v4().to_string();
        self.navigation.push_back(QuickSearchRequest::OpenMain {
            request_id: request_id.clone(),
            target,
        });
        Ok(request_id)
    }

    fn ready(
        &mut self,
        label: &str,
        acknowledged: Option<&str>,
    ) -> Result<Option<QuickSearchRequest>, String> {
        match label {
            MAIN => {
                if self
                    .navigation
                    .front()
                    .is_some_and(|item| Some(item.id()) == acknowledged)
                {
                    self.navigation.pop_front();
                }
                Ok(self.navigation.front().cloned())
            }
            QUICK_SEARCH => {
                if self
                    .focus
                    .as_ref()
                    .is_some_and(|item| Some(item.id()) == acknowledged)
                {
                    self.focus = None;
                }
                Ok(self.focus.clone())
            }
            _ => Err("Quick search is only available to RepoAtlas desktop windows".into()),
        }
    }
}

#[derive(Default)]
struct Registration {
    registered: bool,
    error: Option<String>,
}

impl Registration {
    fn record(&mut self, result: Result<(), String>) {
        self.registered = result.is_ok();
        self.error = result.err();
    }
}

#[derive(Default)]
pub struct QuickSearchState {
    registration: Mutex<Registration>,
    operation_error: Mutex<Option<String>>,
    mailboxes: Mutex<Mailboxes>,
    // Only worker threads take this lock; never block the native event loop on window creation.
    windows: Mutex<()>,
    lifecycle: Lifecycle,
}

fn shortcut() -> &'static str {
    if cfg!(target_os = "macos") {
        "Cmd+Shift+K"
    } else {
        "Ctrl+Shift+K"
    }
}

fn status(app: &AppHandle) -> QuickSearchStatus {
    let state = app.state::<QuickSearchState>();
    let registration = state.registration.lock().unwrap_or_else(|e| e.into_inner());
    QuickSearchStatus {
        shortcut: shortcut(),
        registered: registration.registered,
        error: registration.error.clone().or_else(|| {
            state
                .operation_error
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
        }),
    }
}

fn publish_status(app: &AppHandle) {
    let status = status(app);
    for label in [MAIN, QUICK_SEARCH] {
        let _ = app.emit_to(label, STATUS_EVENT, &status);
    }
}

fn record_operation(app: &AppHandle, generation: Generation, result: &Result<(), String>) {
    {
        let state = app.state::<QuickSearchState>();
        let mut error = state
            .operation_error
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if !state.lifecycle.current(generation) {
            return;
        }
        *error = result.as_ref().err().cloned();
    }
    publish_status(app);
}

fn check_caller(window: &WebviewWindow) -> Result<(), String> {
    match window.label() {
        MAIN | QUICK_SEARCH => Ok(()),
        _ => Err("Quick search is only available to RepoAtlas desktop windows".into()),
    }
}

// Keep lifecycle orchestration independent of the OS calls so tests can pause native
// creation/operations and run the same production paths with a deterministic schedule.
trait DesktopWindow {
    fn show(&self) -> Result<(), String>;
    fn restore(&self) -> Result<(), String>;
    fn focus(&self) -> Result<(), String>;
    fn hide(&self) -> Result<(), String>;
    fn destroy(&self) -> Result<(), String>;
}

trait Desktop {
    type Window: DesktopWindow;
    fn window(&self, label: &str) -> Option<Self::Window>;
    fn create_search(&self) -> Result<Self::Window, String>;
    fn pending(&self, label: &str);
}

struct NativeWindow(WebviewWindow);

impl DesktopWindow for NativeWindow {
    fn show(&self) -> Result<(), String> {
        self.0
            .show()
            .map_err(|e| format!("Could not show window: {e}"))
    }
    fn restore(&self) -> Result<(), String> {
        self.0
            .unminimize()
            .map_err(|e| format!("Could not restore window: {e}"))
    }
    fn focus(&self) -> Result<(), String> {
        self.0
            .set_focus()
            .map_err(|e| format!("Could not focus window: {e}"))
    }
    fn hide(&self) -> Result<(), String> {
        self.0
            .hide()
            .map_err(|e| format!("Could not hide window: {e}"))
    }
    fn destroy(&self) -> Result<(), String> {
        self.0.destroy().map_err(|e| e.to_string())
    }
}

struct NativeDesktop<'a>(&'a AppHandle);

impl Desktop for NativeDesktop<'_> {
    type Window = NativeWindow;

    fn window(&self, label: &str) -> Option<Self::Window> {
        self.0.get_webview_window(label).map(NativeWindow)
    }

    fn create_search(&self) -> Result<Self::Window, String> {
        let window = WebviewWindowBuilder::new(
            self.0,
            QUICK_SEARCH,
            tauri::WebviewUrl::App("index.html?quick-search=1".into()),
        )
        .title("RepoAtlas")
        .inner_size(760.0, 560.0)
        .min_inner_size(520.0, 360.0)
        .center()
        .decorations(false)
        .visible(false)
        .focused(false)
        .skip_taskbar(true)
        .build()
        .map_err(|e| format!("Could not create quick-search window: {e}"))?;
        let handle = self.0.clone();
        window.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let Ok(generation) = handle.state::<QuickSearchState>().lifecycle.request() else {
                    return;
                };
                api.prevent_close();
                let handle = handle.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    let result = hide(
                        &handle.state::<QuickSearchState>(),
                        &NativeDesktop(&handle),
                        generation,
                    );
                    record_operation(&handle, generation, &result);
                });
            }
            // No blur handler: Agent launch and nested dialogs must remain usable.
        });
        Ok(NativeWindow(window))
    }

    fn pending(&self, label: &str) {
        let _ = self.0.emit_to(label, PENDING_EVENT, ());
    }
}

fn wake(
    lifecycle: &Lifecycle,
    generation: Generation,
    window: &impl DesktopWindow,
) -> Result<bool, String> {
    if !lifecycle.current(generation) {
        return Ok(false);
    }
    window.show()?;
    if !lifecycle.current(generation) {
        return Ok(false);
    }
    window.restore()?;
    if !lifecycle.current(generation) {
        return Ok(false);
    }
    window.focus()?;
    Ok(lifecycle.current(generation))
}

fn show(
    state: &QuickSearchState,
    desktop: &impl Desktop,
    generation: Generation,
) -> Result<(), String> {
    let _windows = state.windows.lock().map_err(|e| e.to_string())?;
    if !state.lifecycle.current(generation) {
        return Ok(());
    }
    let window = match desktop.window(QUICK_SEARCH) {
        Some(window) => window,
        None => desktop.create_search()?,
    };
    // Recheck after build: native creation may finish after a hide, another show, or
    // main destruction. Creation is hidden/unfocused and cannot itself steal focus.
    let result = (|| {
        if wake(&state.lifecycle, generation, &window)? {
            state.mailboxes.lock().map_err(|e| e.to_string())?.focus();
            desktop.pending(QUICK_SEARCH);
        }
        Ok(())
    })();
    if state.lifecycle.is_shutdown() {
        // Main's callback can miss a window not yet attached to Tauri's registry.
        // Once attached, either this check or the callback sees it and destroys it.
        let _ = window.destroy();
    }
    result
}

fn hide_current(
    state: &QuickSearchState,
    desktop: &impl Desktop,
    generation: Generation,
) -> Result<(), String> {
    if !state.lifecycle.current(generation) {
        return Ok(());
    }
    if let Some(window) = desktop.window(QUICK_SEARCH) {
        if !state.lifecycle.current(generation) {
            return Ok(());
        }
        window.hide()?;
    }
    if state.lifecycle.current(generation) {
        state.mailboxes.lock().map_err(|e| e.to_string())?.focus = None;
    }
    Ok(())
}

fn hide(
    state: &QuickSearchState,
    desktop: &impl Desktop,
    generation: Generation,
) -> Result<(), String> {
    let _windows = state.windows.lock().map_err(|e| e.to_string())?;
    hide_current(state, desktop, generation)
}

struct NavigationOutcome {
    request_id: String,
    window_result: Result<(), String>,
}

fn open_main(
    state: &QuickSearchState,
    desktop: &impl Desktop,
    generation: Generation,
    target: QuickSearchTarget,
) -> Result<NavigationOutcome, String> {
    let _windows = state.windows.lock().map_err(|e| e.to_string())?;
    if state.lifecycle.is_shutdown() {
        return Err("RepoAtlas is shutting down".into());
    }
    let main = desktop.window(MAIN).ok_or("Main window is unavailable")?;
    // Superseding a window operation must not discard an already accepted navigation.
    // Queue first; a later OS failure is a status error, not a reason to resend navigation.
    let request_id = state
        .mailboxes
        .lock()
        .map_err(|e| e.to_string())?
        .navigate(target)?;
    desktop.pending(MAIN);
    let window_result = wake(&state.lifecycle, generation, &main)
        .and_then(|_| hide_current(state, desktop, generation));
    Ok(NavigationOutcome {
        request_id,
        window_result,
    })
}

fn shutdown(state: &QuickSearchState, desktop: &impl Desktop) {
    // Runs in the native event callback: never wait for the worker's windows lock.
    state.lifecycle.shutdown();
    if let Some(window) = desktop.window(QUICK_SEARCH) {
        let _ = window.destroy();
    }
}

fn show_and_report(app: &AppHandle, generation: Generation) -> Result<(), String> {
    let state = app.state::<QuickSearchState>();
    let desktop = NativeDesktop(app);
    let result = show(&state, &desktop, generation);
    record_operation(app, generation, &result);
    if result.is_err() {
        let _windows = state.windows.lock().map_err(|e| e.to_string())?;
        if let Some(main) = desktop.window(MAIN) {
            let _ = wake(&state.lifecycle, generation, &main);
        }
    }
    result
}

#[cfg(desktop)]
fn register(app: &AppHandle) {
    use tauri_plugin_global_shortcut::{GlobalShortcut, GlobalShortcutExt, ShortcutState};
    let state = app.state::<QuickSearchState>();
    let mut registration = state.registration.lock().unwrap_or_else(|e| e.into_inner());
    if registration.registered {
        return;
    }
    // A plugin initialization failure is also non-fatal. Registration conflicts can retry
    // without restarting; initialization failures require restarting the desktop runtime.
    if app.try_state::<GlobalShortcut<tauri::Wry>>().is_none() {
        return;
    }
    registration.record(
        app.global_shortcut()
            .on_shortcut("CommandOrControl+Shift+K", |app, _, event| {
                if event.state == ShortcutState::Pressed {
                    let Ok(generation) = app.state::<QuickSearchState>().lifecycle.request() else {
                        return;
                    };
                    let app = app.clone();
                    // WebView2 creation from a synchronous native event callback can deadlock.
                    tauri::async_runtime::spawn_blocking(move || {
                        let _ = show_and_report(&app, generation);
                    });
                }
            })
            .map_err(|e| format!("Could not register {}: {e}", shortcut())),
    );
}

pub fn setup(app: &AppHandle) {
    app.manage(QuickSearchState::default());
    #[cfg(desktop)]
    {
        // Do not configure with_shortcut(): its setup error would abort app startup.
        match app.plugin(tauri_plugin_global_shortcut::Builder::new().build()) {
            Ok(()) => register(app),
            Err(error) => app
                .state::<QuickSearchState>()
                .registration
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .record(Err(format!(
                    "Could not initialize global shortcuts: {error}"
                ))),
        }
    }
    #[cfg(not(desktop))]
    app.state::<QuickSearchState>()
        .registration
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .record(Err("Global quick search requires the desktop app".into()));

    // Preserve normal app shutdown: a hidden auxiliary window must not keep the app alive.
    if let Some(main) = app.get_webview_window(MAIN) {
        let handle = app.clone();
        main.on_window_event(move |event| {
            if matches!(event, WindowEvent::Destroyed) {
                shutdown(&handle.state::<QuickSearchState>(), &NativeDesktop(&handle));
            }
        });
    }
}

#[tauri::command]
pub async fn quick_search_status(
    app: AppHandle,
    window: WebviewWindow,
) -> Result<QuickSearchStatus, String> {
    check_caller(&window)?;
    Ok(status(&app))
}

#[tauri::command]
pub async fn quick_search_retry_registration(
    app: AppHandle,
    window: WebviewWindow,
) -> Result<QuickSearchStatus, String> {
    check_caller(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(desktop)]
        register(&app);
        publish_status(&app);
        status(&app)
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn quick_search_show(app: AppHandle, window: WebviewWindow) -> Result<(), String> {
    check_caller(&window)?;
    let generation = app.state::<QuickSearchState>().lifecycle.request()?;
    tauri::async_runtime::spawn_blocking(move || show_and_report(&app, generation))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn quick_search_hide(app: AppHandle, window: WebviewWindow) -> Result<(), String> {
    check_caller(&window)?;
    let generation = app.state::<QuickSearchState>().lifecycle.request()?;
    tauri::async_runtime::spawn_blocking(move || {
        let result = hide(
            &app.state::<QuickSearchState>(),
            &NativeDesktop(&app),
            generation,
        );
        record_operation(&app, generation, &result);
        result
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn quick_search_ready(
    app: AppHandle,
    window: WebviewWindow,
    acknowledged_request_id: Option<String>,
) -> Result<Option<QuickSearchRequest>, String> {
    check_caller(&window)?;
    app.state::<QuickSearchState>()
        .mailboxes
        .lock()
        .map_err(|e| e.to_string())?
        .ready(window.label(), acknowledged_request_id.as_deref())
}

#[tauri::command]
pub async fn quick_search_open_main(
    app: AppHandle,
    window: WebviewWindow,
    target: QuickSearchTarget,
) -> Result<String, String> {
    check_caller(&window)?;
    target.validate()?;
    let generation = app.state::<QuickSearchState>().lifecycle.request()?;
    tauri::async_runtime::spawn_blocking(move || {
        let outcome = open_main(
            &app.state::<QuickSearchState>(),
            &NativeDesktop(&app),
            generation,
            target,
        )?;
        record_operation(&app, generation, &outcome.window_result);
        Ok(outcome.request_id)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::{atomic::AtomicBool, mpsc, Arc};
    use std::time::Duration;

    #[derive(Default)]
    struct FakeWindows {
        attached: AtomicBool,
        live: AtomicBool,
        visible: AtomicBool,
        focused: Mutex<Option<&'static str>>,
        effects: Mutex<Vec<String>>,
    }

    type Hook = Arc<dyn Fn() + Send + Sync>;

    #[derive(Clone, Default)]
    struct FakeDesktop {
        windows: Arc<FakeWindows>,
        before_attach: Option<Hook>,
        after_show: Option<Hook>,
    }

    impl FakeDesktop {
        fn record(&self, effect: &str) {
            self.windows.effects.lock().unwrap().push(effect.into());
        }

        fn effects(&self) -> Vec<String> {
            self.windows.effects.lock().unwrap().clone()
        }
    }

    struct FakeWindow {
        desktop: FakeDesktop,
        label: &'static str,
    }

    impl DesktopWindow for FakeWindow {
        fn show(&self) -> Result<(), String> {
            self.desktop.record(&format!("show:{}", self.label));
            if self.label == QUICK_SEARCH {
                self.desktop.windows.visible.store(true, Ordering::SeqCst);
            }
            if let Some(hook) = &self.desktop.after_show {
                hook();
            }
            Ok(())
        }

        fn restore(&self) -> Result<(), String> {
            self.desktop.record(&format!("restore:{}", self.label));
            Ok(())
        }

        fn focus(&self) -> Result<(), String> {
            self.desktop.record(&format!("focus:{}", self.label));
            *self.desktop.windows.focused.lock().unwrap() = Some(self.label);
            Ok(())
        }

        fn hide(&self) -> Result<(), String> {
            self.desktop.record(&format!("hide:{}", self.label));
            self.desktop.windows.visible.store(false, Ordering::SeqCst);
            Ok(())
        }

        fn destroy(&self) -> Result<(), String> {
            self.desktop.record(&format!("destroy:{}", self.label));
            self.desktop.windows.attached.store(false, Ordering::SeqCst);
            self.desktop.windows.live.store(false, Ordering::SeqCst);
            self.desktop.windows.visible.store(false, Ordering::SeqCst);
            Ok(())
        }
    }

    impl Desktop for FakeDesktop {
        type Window = FakeWindow;

        fn window(&self, label: &str) -> Option<Self::Window> {
            let label = match label {
                MAIN => MAIN,
                QUICK_SEARCH if self.windows.attached.load(Ordering::SeqCst) => QUICK_SEARCH,
                _ => return None,
            };
            Some(FakeWindow {
                desktop: self.clone(),
                label,
            })
        }

        fn create_search(&self) -> Result<Self::Window, String> {
            // Model the native-live / Tauri-not-attached interval of build().
            self.record("create-native");
            self.windows.live.store(true, Ordering::SeqCst);
            if let Some(hook) = &self.before_attach {
                hook();
            }
            self.windows.attached.store(true, Ordering::SeqCst);
            self.record("attach");
            Ok(FakeWindow {
                desktop: self.clone(),
                label: QUICK_SEARCH,
            })
        }

        fn pending(&self, label: &str) {
            self.record(&format!("pending:{label}"));
        }
    }

    fn project(id: &str) -> QuickSearchTarget {
        QuickSearchTarget::Project {
            project_id: id.into(),
        }
    }

    #[test]
    fn navigation_survives_absent_listeners_and_reloads_until_acknowledged() {
        let mut queues = Mailboxes::default();
        let first = queues.navigate(project("one")).unwrap();
        let second = queues.navigate(project("two")).unwrap();
        for _ in 0..3 {
            assert_eq!(queues.ready(MAIN, None).unwrap().unwrap().id(), first);
        }
        assert_eq!(
            queues.ready(MAIN, Some(&first)).unwrap().unwrap().id(),
            second
        );
        // Retried/stale acknowledgements cannot remove a later request.
        assert_eq!(
            queues.ready(MAIN, Some(&first)).unwrap().unwrap().id(),
            second
        );
        assert!(queues.ready(MAIN, Some(&second)).unwrap().is_none());
    }

    #[test]
    fn acknowledgements_are_window_scoped() {
        let mut queues = Mailboxes::default();
        let navigation = queues.navigate(project("one")).unwrap();
        queues.focus();
        let focus = queues.ready(QUICK_SEARCH, None).unwrap().unwrap();
        assert_eq!(
            queues.ready(QUICK_SEARCH, Some(&navigation)).unwrap(),
            Some(focus.clone())
        );
        assert_eq!(
            queues.ready(MAIN, Some(focus.id())).unwrap().unwrap().id(),
            navigation
        );
        assert!(queues.ready("untrusted", Some(&navigation)).is_err());
        assert!(queues
            .ready(QUICK_SEARCH, Some(focus.id()))
            .unwrap()
            .is_none());
    }

    #[test]
    fn new_show_focus_survives_ack_from_an_older_in_flight_handler() {
        let mut queues = Mailboxes::default();
        queues.focus();
        let old = queues.ready(QUICK_SEARCH, None).unwrap().unwrap();
        // A new invocation arrives while the old handler awaits settings/recent IPC.
        queues.focus();
        let latest = queues.ready(QUICK_SEARCH, None).unwrap().unwrap();
        assert_ne!(old.id(), latest.id());
        for _ in 0..2 {
            assert_eq!(
                queues.ready(QUICK_SEARCH, Some(old.id())).unwrap(),
                Some(latest.clone())
            );
        }
        assert!(queues
            .ready(QUICK_SEARCH, Some(latest.id()))
            .unwrap()
            .is_none());
    }

    #[test]
    fn full_queue_rejects_new_requests_without_losing_existing_navigation() {
        let mut queues = Mailboxes::default();
        let ids: Vec<_> = (0..MAX_PENDING)
            .map(|_| queues.navigate(project("one")).unwrap())
            .collect();
        assert!(queues.navigate(project("overflow")).is_err());
        assert_eq!(queues.navigation.len(), MAX_PENDING);
        assert_eq!(queues.ready(MAIN, None).unwrap().unwrap().id(), ids[0]);
        for id in ids {
            queues.ready(MAIN, Some(&id)).unwrap();
        }
        assert!(queues.navigate(project("recovered")).is_ok());
    }

    #[test]
    fn typed_navigation_rejects_urls_commands_unknown_fields_and_invalid_ids() {
        for value in [
            json!({"kind":"url", "url":"https://example.com"}),
            json!({"kind":"project", "projectId":"one", "url":"file:///secret"}),
            json!({"kind":"sessions", "command":"resume"}),
            json!({"kind":"sessions", "sessionId":"one", "messageIndex":-1}),
            json!({"kind":"sessions", "sessionId":"one", "messageIndex":1.5}),
        ] {
            assert!(serde_json::from_value::<QuickSearchTarget>(value).is_err());
        }
        for id in ["", "  ", "x\n", &"x".repeat(257)] {
            assert!(Mailboxes::default().navigate(project(id)).is_err());
        }
        let missing_session = serde_json::from_value::<QuickSearchTarget>(
            json!({"kind":"sessions", "messageIndex":0}),
        )
        .unwrap();
        assert!(missing_session.validate().is_err());
        assert!(
            serde_json::from_value::<QuickSearchTarget>(json!({"kind":"sessions"}))
                .unwrap()
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn serialized_requests_match_the_typescript_contract() {
        let target: QuickSearchTarget = serde_json::from_value(json!({
            "kind":"sessions", "sessionId":"session-one", "sources":false, "messageIndex":0,
        }))
        .unwrap();
        assert_eq!(
            serde_json::to_value(QuickSearchRequest::OpenMain {
                request_id: "request-one".into(),
                target,
            })
            .unwrap(),
            json!({
                "action":"open-main", "requestId":"request-one",
                "target":{"kind":"sessions", "sessionId":"session-one", "sources":false, "messageIndex":0},
            })
        );
        assert_eq!(
            serde_json::to_value(QuickSearchRequest::Focus {
                request_id: "focus-one".into()
            })
            .unwrap(),
            json!({"action":"focus", "requestId":"focus-one"})
        );
    }

    #[test]
    fn registration_failure_is_reportable_and_successful_retry_clears_it() {
        let mut registration = Registration::default();
        registration.record(Err("shortcut already registered".into()));
        assert!(!registration.registered);
        assert!(registration.error.is_some());
        registration.record(Ok(()));
        assert!(registration.registered);
        assert_eq!(registration.error, None);
    }

    #[test]
    fn delayed_hide_cannot_hide_or_clear_focus_from_a_newer_show() {
        let state = QuickSearchState::default();
        let desktop = FakeDesktop::default();
        let old_hide = state.lifecycle.request().unwrap();
        let new_show = state.lifecycle.request().unwrap();
        // Force the later worker to acquire the windows lock first.
        show(&state, &desktop, new_show).unwrap();
        let effects = desktop.effects();
        let focus = state.mailboxes.lock().unwrap().focus.clone();
        hide(&state, &desktop, old_hide).unwrap();
        assert_eq!(desktop.effects(), effects);
        assert!(desktop.windows.visible.load(Ordering::SeqCst));
        assert_eq!(*desktop.windows.focused.lock().unwrap(), Some(QUICK_SEARCH));
        assert!(focus.is_some());
        assert_eq!(state.mailboxes.lock().unwrap().focus, focus);
    }

    #[test]
    fn delayed_show_cannot_reopen_after_a_newer_hide() {
        let state = QuickSearchState::default();
        let desktop = FakeDesktop::default();
        let old_show = state.lifecycle.request().unwrap();
        let new_hide = state.lifecycle.request().unwrap();
        hide(&state, &desktop, new_hide).unwrap();
        show(&state, &desktop, old_show).unwrap();
        assert!(desktop.effects().is_empty());
        assert!(!desktop.windows.live.load(Ordering::SeqCst));
        assert!(state.mailboxes.lock().unwrap().focus.is_none());
    }

    #[test]
    fn delayed_navigation_still_queues_but_cannot_steal_new_search_focus_or_hide_it() {
        let state = QuickSearchState::default();
        let desktop = FakeDesktop::default();
        let old_navigation = state.lifecycle.request().unwrap();
        let new_show = state.lifecycle.request().unwrap();
        show(&state, &desktop, new_show).unwrap();
        let focus = state.mailboxes.lock().unwrap().focus.clone();
        desktop.windows.effects.lock().unwrap().clear();
        let outcome = open_main(&state, &desktop, old_navigation, project("one")).unwrap();
        assert!(outcome.window_result.is_ok());
        assert_eq!(desktop.effects(), vec!["pending:main"]);
        assert!(desktop.windows.visible.load(Ordering::SeqCst));
        assert_eq!(*desktop.windows.focused.lock().unwrap(), Some(QUICK_SEARCH));
        let mut mailboxes = state.mailboxes.lock().unwrap();
        assert_eq!(mailboxes.focus, focus);
        assert_eq!(
            mailboxes.ready(MAIN, None).unwrap().unwrap().id(),
            outcome.request_id
        );
    }

    #[test]
    fn show_superseded_during_build_stays_hidden_until_the_new_worker_runs() {
        let state = Arc::new(QuickSearchState::default());
        let latest = Arc::new(Mutex::new(None));
        let desktop = FakeDesktop {
            before_attach: Some({
                let state = state.clone();
                let latest = latest.clone();
                Arc::new(move || {
                    *latest.lock().unwrap() = Some(state.lifecycle.request().unwrap());
                })
            }),
            ..Default::default()
        };
        let old_show = state.lifecycle.request().unwrap();
        show(&state, &desktop, old_show).unwrap();
        assert_eq!(desktop.effects(), vec!["create-native", "attach"]);
        assert!(!desktop.windows.visible.load(Ordering::SeqCst));
        assert!(state.mailboxes.lock().unwrap().focus.is_none());
        show(&state, &desktop, latest.lock().unwrap().unwrap()).unwrap();
        assert!(desktop.windows.visible.load(Ordering::SeqCst));
        assert_eq!(*desktop.windows.focused.lock().unwrap(), Some(QUICK_SEARCH));
        assert_eq!(
            desktop
                .effects()
                .iter()
                .filter(|effect| *effect == "create-native")
                .count(),
            1
        );
    }

    #[test]
    fn navigation_superseded_during_wake_stops_before_restore_focus_and_hide() {
        let state = Arc::new(QuickSearchState::default());
        let latest = Arc::new(Mutex::new(None));
        let desktop = FakeDesktop {
            after_show: Some({
                let state = state.clone();
                let latest = latest.clone();
                Arc::new(move || {
                    *latest.lock().unwrap() = Some(state.lifecycle.request().unwrap());
                })
            }),
            ..Default::default()
        };
        let old_navigation = state.lifecycle.request().unwrap();
        let outcome = open_main(&state, &desktop, old_navigation, project("one")).unwrap();
        assert!(outcome.window_result.is_ok());
        assert_eq!(desktop.effects(), vec!["pending:main", "show:main"]);
        assert_eq!(state.mailboxes.lock().unwrap().navigation.len(), 1);
        let desktop = FakeDesktop {
            after_show: None,
            ..desktop
        };
        show(&state, &desktop, latest.lock().unwrap().unwrap()).unwrap();
        assert_eq!(*desktop.windows.focused.lock().unwrap(), Some(QUICK_SEARCH));
        assert!(desktop.windows.visible.load(Ordering::SeqCst));
    }

    #[test]
    fn shutdown_never_waits_for_build_and_destroys_a_late_unattached_window() {
        let state = Arc::new(QuickSearchState::default());
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let release_rx = Mutex::new(release_rx);
        let desktop = FakeDesktop {
            before_attach: Some(Arc::new(move || {
                entered_tx.send(()).unwrap();
                release_rx
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(10))
                    .unwrap();
            })),
            ..Default::default()
        };
        let generation = state.lifecycle.request().unwrap();
        let worker = {
            let state = state.clone();
            let desktop = desktop.clone();
            std::thread::spawn(move || show(&state, &desktop, generation))
        };
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(desktop.windows.live.load(Ordering::SeqCst));
        assert!(desktop.window(QUICK_SEARCH).is_none());
        assert!(state.windows.try_lock().is_err());
        let (closed_tx, closed_rx) = mpsc::channel();
        let main_callback = {
            let state = state.clone();
            let desktop = desktop.clone();
            std::thread::spawn(move || {
                shutdown(&state, &desktop);
                closed_tx.send(()).unwrap();
            })
        };
        let returned_without_worker = closed_rx.recv_timeout(Duration::from_secs(2)).is_ok();
        // Always release before asserting: even a lock regression must fail without hanging.
        release_tx.send(()).unwrap();
        main_callback.join().unwrap();
        worker.join().unwrap().unwrap();
        assert!(
            returned_without_worker,
            "main callback waited for the worker's windows lock"
        );
        assert!(state.lifecycle.request().is_err());
        assert!(!desktop.windows.live.load(Ordering::SeqCst));
        assert!(!desktop.windows.visible.load(Ordering::SeqCst));
        assert!(desktop.window(QUICK_SEARCH).is_none());
        assert_eq!(
            desktop.effects(),
            vec!["create-native", "attach", "destroy:quick-search"]
        );
    }

    #[test]
    fn shutdown_destroys_existing_window_and_rejects_queued_or_new_operations() {
        let state = QuickSearchState::default();
        let desktop = FakeDesktop::default();
        let shown = state.lifecycle.request().unwrap();
        show(&state, &desktop, shown).unwrap();
        let queued_navigation = state.lifecycle.request().unwrap();
        let queued_show = state.lifecycle.request().unwrap();
        shutdown(&state, &desktop);
        let effects = desktop.effects();
        show(&state, &desktop, queued_show).unwrap();
        hide(&state, &desktop, queued_show).unwrap();
        assert!(open_main(&state, &desktop, queued_navigation, project("one")).is_err());
        assert!(state.lifecycle.request().is_err());
        assert_eq!(desktop.effects(), effects);
        assert!(!desktop.windows.live.load(Ordering::SeqCst));
        assert!(state.mailboxes.lock().unwrap().navigation.is_empty());
    }
}
