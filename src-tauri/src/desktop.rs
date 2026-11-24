use crate::{
    geometry,
    storage::{Config, ImportPreview, ScreenPosition, Store, DEFAULT_SHORTCUT},
};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        mpsc, Mutex,
    },
    time::Duration,
};
use tauri::{
    menu::MenuBuilder,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, EventTarget, Manager, State, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tokio::sync::oneshot;

const NOTE: &str = "main";
const SETTINGS: &str = "settings";

type Reply<T> = Result<T, String>;

fn message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[derive(Clone, Copy, PartialEq)]
enum Action {
    Hide,
    Exit,
    Restart,
}

struct PendingAction {
    id: String,
    action: Action,
    complete: Option<oneshot::Sender<bool>>,
}

struct Desktop {
    store: Store,
    config: Mutex<Config>,
    shortcut_id: AtomicU32,
    history_warning: Mutex<Option<String>>,
    ready: AtomicBool,
    exiting: AtomicBool,
    pending: Mutex<Option<PendingAction>>,
    sequence: AtomicU64,
    positions: mpsc::Sender<ScreenPosition>,
    note_lock: Mutex<()>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    text: String,
    config: Config,
    shortcut: String,
    default_shortcut: &'static str,
    shortcut_ok: bool,
    data_dir: String,
    warning: Option<String>,
    history_warning: Option<String>,
}

#[tauri::command]
fn load_state(state: State<'_, Desktop>) -> Reply<Snapshot> {
    let config = state.config.lock().map_err(message)?.clone();
    Ok(Snapshot {
        text: state.store.load_notes()?,
        shortcut: config.shortcut.clone(),
        default_shortcut: DEFAULT_SHORTCUT,
        config,
        shortcut_ok: state.shortcut_id.load(Ordering::Relaxed) != 0,
        data_dir: state.store.dir.display().to_string(),
        warning: state.store.startup_warning.clone(),
        history_warning: state.history_warning.lock().map_err(message)?.clone(),
    })
}

#[tauri::command]
async fn save_note(app: AppHandle, text: String) -> Reply<Option<String>> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Desktop>();
        let _note = state.note_lock.lock().map_err(message)?;
        state.store.save_notes(&text)?;
        let warning = state.store.start_history(&text).err();
        *state.history_warning.lock().map_err(message)? = warning.clone();
        Ok(warning)
    })
    .await
    .map_err(message)?
}

#[derive(Serialize, Deserialize, Clone)]
struct Appearance {
    width: Option<i32>,
    font_size: Option<i32>,
    color: Option<String>,
    opacity: Option<f64>,
    outline: Option<bool>,
}

#[tauri::command]
fn save_config(app: AppHandle, state: State<'_, Desktop>, config: Appearance) -> Reply<()> {
    let mut current = state.config.lock().map_err(message)?;
    let mut next = current.clone();
    if let Some(value) = config.width {
        next.width = value.clamp(240, 1200);
    }
    if let Some(value) = config.font_size {
        next.font_size = value.clamp(10, 64);
    }
    if let Some(color) = config.color {
        if color.len() != 7
            || !color.starts_with('#')
            || !color[1..].bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("文字颜色无效".into());
        }
        next.color = color;
    }
    if let Some(value) = config.opacity {
        next.opacity = value.clamp(0.2, 1.0);
    }
    if let Some(value) = config.outline {
        next.outline = value;
    }
    state.store.save_config(&next)?;
    let appearance = Appearance {
        width: Some(next.width),
        font_size: Some(next.font_size),
        color: Some(next.color.clone()),
        opacity: Some(next.opacity),
        outline: Some(next.outline),
    };
    *current = next;
    drop(current);
    emit(&app, NOTE, "config-changed", appearance)
}

#[tauri::command]
fn set_shortcut(app: AppHandle, state: State<'_, Desktop>, shortcut: String) -> Reply<String> {
    let key: Shortcut = shortcut.parse().map_err(message)?;
    if key.mods.is_empty() {
        return Err("快捷键需要包含修饰键".into());
    }
    let mut current = state.config.lock().map_err(message)?;
    let old = current
        .shortcut
        .parse::<Shortcut>()
        .ok()
        .filter(|old| old.id() == state.shortcut_id.load(Ordering::Relaxed));
    if old == Some(key) {
        return Ok(current.shortcut.clone());
    }

    let manager = app.global_shortcut();
    manager
        .register(key)
        .map_err(|error| format!("无法注册快捷键，可能已被占用：{error}"))?;
    let mut next = current.clone();
    next.shortcut.clone_from(&shortcut);
    if let Err(error) = state.store.save_config(&next) {
        let _ = manager.unregister(key);
        return Err(error);
    }
    state.shortcut_id.store(key.id(), Ordering::Relaxed);
    if let Some(old) = old {
        let _ = manager.unregister(old);
    }
    *current = next;
    drop(current);
    emit(&app, NOTE, "shortcut-changed", &shortcut)?;
    Ok(shortcut)
}

#[tauri::command]
async fn history(app: AppHandle, start: i64, end: i64) -> Reply<String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<Desktop>().store.history_increment(start, end)
    })
    .await
    .map_err(message)?
}

#[tauri::command]
async fn history_bounds(app: AppHandle) -> Reply<(Option<i64>, Option<i64>)> {
    tauri::async_runtime::spawn_blocking(move || {
        Ok(app
            .state::<Desktop>()
            .store
            .history_bounds()?
            .map_or((None, None), |(first, last)| (Some(first), Some(last))))
    })
    .await
    .map_err(message)?
}

#[tauri::command]
async fn pick_import_directory(window: WebviewWindow) -> Reply<Option<String>> {
    let (send, receive) = oneshot::channel();
    window
        .app_handle()
        .dialog()
        .file()
        .set_title("选择旧版 FloatNote 数据目录")
        .set_parent(&window)
        .pick_folder(move |folder| {
            let _ = send.send(folder);
        });
    let folder = receive.await.map_err(message)?;
    folder
        .map(|path| {
            path.into_path()
                .map(|path| path.display().to_string())
                .map_err(message)
        })
        .transpose()
}

#[tauri::command]
async fn inspect_import(app: AppHandle, source: PathBuf) -> Reply<ImportPreview> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<Desktop>().store.inspect_import(&source)
    })
    .await
    .map_err(message)?
}

#[tauri::command]
async fn import_data(app: AppHandle, source: PathBuf, allow_conflict: bool) -> Reply<String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Desktop>();
        let _note = state.note_lock.lock().map_err(message)?;
        state.store.import_from(&source, allow_conflict)
    })
    .await
    .map_err(message)?
}

fn emit<S: Serialize + Clone>(app: &AppHandle, label: &str, event: &str, data: S) -> Reply<()> {
    app.emit_to(EventTarget::labeled(label), event, data)
        .map_err(message)
}

fn note_window(app: &AppHandle) -> Reply<WebviewWindow> {
    app.get_webview_window(NOTE)
        .ok_or_else(|| "浮窗未就绪".into())
}

#[tauri::command]
fn fit_note(app: AppHandle, state: State<'_, Desktop>, height: u32) -> Reply<geometry::NoteSize> {
    let width = state.config.lock().map_err(message)?.width;
    geometry::fit(&note_window(&app)?, width, height).map_err(message)
}

#[tauri::command]
fn reset_position(app: AppHandle, state: State<'_, Desktop>) -> Reply<()> {
    let config = state.config.lock().map_err(message)?.clone();
    geometry::restore(&note_window(&app)?, &config, true).map_err(message)
}

#[tauri::command]
fn note_error(app: AppHandle) -> Reply<()> {
    note_window(&app)?.show().map_err(message)
}

#[tauri::command]
fn note_ready(app: AppHandle, state: State<'_, Desktop>) -> Reply<()> {
    let config = state.config.lock().map_err(message)?.clone();
    let window = note_window(&app)?;
    geometry::restore(&window, &config, false).map_err(message)?;
    window.set_focusable(false).map_err(message)?;
    window.show().map_err(message)?;
    state.ready.store(true, Ordering::Release);
    if std::env::args_os().any(|arg| arg == "--smoke-test") {
        finish_startup_check(&app, &window, &state)?;
    }
    Ok(())
}

// CI launches the bundled executable with a scratch data directory.
fn finish_startup_check(app: &AppHandle, window: &WebviewWindow, state: &Desktop) -> Reply<()> {
    let dir = std::env::var_os("FLOATNOTE_DATA_DIR").map(PathBuf::from);
    let report = std::env::var_os("FLOATNOTE_SMOKE_REPORT")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute() && path.parent() == dir.as_deref())
        .ok_or("启动验证需要在临时数据目录内指定报告文件")?;
    let visible = window.is_visible().map_err(message)?;
    if !visible {
        return Err("浮窗未显示".into());
    }
    let output = serde_json::json!({
        "visible": visible,
        "shortcutOk": state.shortcut_id.load(Ordering::Relaxed) != 0,
    });
    std::fs::write(report, output.to_string()).map_err(message)?;
    complete_action(app, Action::Exit)
}

fn complete_action(app: &AppHandle, action: Action) -> Reply<()> {
    let window = note_window(app)?;
    if action == Action::Hide {
        return window.hide().map_err(message);
    }
    let state = app.state::<Desktop>();
    if let Ok(Some(position)) = geometry::capture(&window) {
        // A pending move can otherwise be lost when restarting immediately after a drag.
        if state.config.lock().map_err(message)?.position.is_some() {
            persist_position(&state, position)?;
        }
    }
    state.exiting.store(true, Ordering::Release);
    if action == Action::Restart {
        app.request_restart();
    } else {
        app.exit(0);
    }
    Ok(())
}

fn show_note(app: &AppHandle, window: &WebviewWindow) -> Reply<()> {
    let config = app
        .state::<Desktop>()
        .config
        .lock()
        .map_err(message)?
        .clone();
    geometry::recover_if_offscreen(window, &config).map_err(message)?;
    window.set_focusable(false).map_err(message)?;
    window.show().map_err(message)
}

fn request_action(app: &AppHandle, action: Action) -> Reply<()> {
    let state = app.state::<Desktop>();
    let window = note_window(app)?;
    if !state.ready.load(Ordering::Acquire) || !window.is_visible().map_err(message)? {
        return complete_action(app, action);
    }

    let mut pending = state.pending.lock().map_err(message)?;
    if let Some(request) = pending.as_mut() {
        if action != Action::Hide {
            request.action = action;
        }
        return Ok(());
    }
    let (send, receive) = oneshot::channel();
    let id = state.sequence.fetch_add(1, Ordering::Relaxed).to_string();
    *pending = Some(PendingAction {
        id: id.clone(),
        action,
        complete: Some(send),
    });
    drop(pending);

    if let Err(error) = emit(app, NOTE, "note-flush", &id) {
        state.pending.lock().map_err(message)?.take();
        return Err(error);
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let acknowledged = tokio::time::timeout(Duration::from_secs(15), receive).await;
        let request = app
            .state::<Desktop>()
            .pending
            .lock()
            .ok()
            .and_then(|mut pending| pending.take());
        let Some(request) = request else { return };
        let result = match acknowledged {
            Ok(Ok(true)) => complete_action(&app, request.action),
            Ok(Ok(false)) => Err("便签保存失败，请回到编辑框重试。".into()),
            _ => Err("浮窗未完成保存，请稍后重试。".into()),
        };
        if let Err(error) = result {
            report_action_error(&app, &error);
        }
    });
    Ok(())
}

fn report_action_error(app: &AppHandle, error: &str) {
    let _ = emit(app, NOTE, "action-failed", error);
    let _ = emit(app, SETTINGS, "action-failed", error);
}

#[tauri::command]
fn flush_complete(
    window: WebviewWindow,
    state: State<'_, Desktop>,
    request_id: String,
    ok: bool,
) -> Reply<()> {
    if window.label() != NOTE {
        return Err("无效窗口".into());
    }
    if let Some(request) = state
        .pending
        .lock()
        .map_err(message)?
        .as_mut()
        .filter(|r| r.id == request_id)
    {
        if let Some(send) = request.complete.take() {
            let _ = send.send(ok);
        }
    }
    Ok(())
}

#[tauri::command]
fn toggle_note(app: AppHandle) -> Reply<()> {
    let window = note_window(&app)?;
    if window.is_visible().map_err(message)? {
        request_action(&app, Action::Hide)
    } else {
        show_note(&app, &window)
    }
}

#[tauri::command]
fn exit_app(app: AppHandle) -> Reply<()> {
    request_action(&app, Action::Exit)
}

#[tauri::command]
fn restart_app(app: AppHandle) -> Reply<()> {
    request_action(&app, Action::Restart)
}

#[tauri::command]
async fn open_panel(app: AppHandle, tab: String) -> Reply<()> {
    if !matches!(tab.as_str(), "style" | "hotkey" | "history" | "data") {
        return Err("无效设置页".into());
    }
    if let Some(window) = app.get_webview_window(SETTINGS) {
        window.unminimize().map_err(message)?;
        window.show().map_err(message)?;
        window.set_focus().map_err(message)?;
        return emit(&app, SETTINGS, "panel-tab", &tab);
    }
    WebviewWindowBuilder::new(
        &app,
        SETTINGS,
        WebviewUrl::App(format!("index.html?tab={tab}").into()),
    )
    .title("FloatNote · 设置与历史")
    .inner_size(600., 520.)
    .min_inner_size(400., 340.)
    .always_on_top(true)
    .center()
    .build()
    .map_err(message)?;
    Ok(())
}

fn install_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = MenuBuilder::new(app)
        .text("tray-toggle", "显示 / 隐藏便签")
        .text("tray-style", "外观设置…")
        .text("tray-hotkey", "快捷键设置…")
        .text("tray-history", "查看变更记录…")
        .text("tray-data", "数据与导入…")
        .separator()
        .text("tray-restart", "重新启动")
        .text("tray-quit", "退出 FloatNote")
        .build()?;
    TrayIconBuilder::new()
        .tooltip("FloatNote 悬浮便签")
        .icon(app.default_window_icon().expect("bundled icon").clone())
        .menu(&menu)
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .on_menu_event(|app, event| {
            let Some(id) = event.id.as_ref().strip_prefix("tray-") else {
                return;
            };
            let result = match id {
                "toggle" => toggle_note(app.clone()),
                "restart" => restart_app(app.clone()),
                "quit" => exit_app(app.clone()),
                tab => {
                    let app = app.clone();
                    let tab = tab.to_string();
                    tauri::async_runtime::spawn(async move {
                        if let Err(error) = open_panel(app.clone(), tab).await {
                            report_action_error(&app, &error);
                        }
                    });
                    Ok(())
                }
            };
            if let Err(error) = result {
                report_action_error(app, &error);
            }
        })
        .on_tray_icon_event(|icon, event| {
            if cfg!(target_os = "windows")
                && matches!(
                    event,
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    }
                )
            {
                let app = icon.app_handle();
                if let Err(error) = toggle_note(app.clone()) {
                    report_action_error(app, &error);
                }
            }
        })
        .build(app)?;
    Ok(())
}

fn persist_position(state: &Desktop, position: ScreenPosition) -> Reply<()> {
    let mut config = state.config.lock().map_err(message)?;
    if config.position.as_ref() != Some(&position) {
        let mut updated = config.clone();
        updated.position = Some(position);
        state.store.save_config(&updated)?;
        *config = updated;
    }
    Ok(())
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);

    let store = Store::discover(app.path().app_data_dir()?)?;
    let config = store.load_config()?;
    let text = store.load_notes()?;
    let warning = store.initialize_history(&text).err();
    let (positions, moves) = mpsc::channel();
    let shortcut = config.shortcut.parse::<Shortcut>().ok();
    app.manage(Desktop {
        store,
        config: Mutex::new(config),
        history_warning: Mutex::new(warning),
        shortcut_id: AtomicU32::new(0),
        pending: Mutex::new(None),
        sequence: AtomicU64::new(1),
        ready: AtomicBool::new(false),
        exiting: AtomicBool::new(false),
        positions,
        note_lock: Mutex::new(()),
    });
    if let Some(key) = shortcut {
        if app.global_shortcut().register(key).is_ok() {
            app.state::<Desktop>()
                .shortcut_id
                .store(key.id(), Ordering::Relaxed);
        }
    }

    let handle = app.handle().clone();
    std::thread::spawn(move || {
        while let Ok(mut position) = moves.recv() {
            while let Ok(next) = moves.recv_timeout(Duration::from_millis(150)) {
                position = next;
            }
            if let Err(error) = persist_position(&handle.state::<Desktop>(), position) {
                report_action_error(&handle, &error);
            }
        }
    });
    install_tray(app.handle())?;
    Ok(())
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Ok(window) = note_window(app) {
                if !window.is_visible().unwrap_or_default() {
                    let _ = show_note(app, &window);
                }
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, key, event| {
                    let state = app.state::<Desktop>();
                    if event.state() == ShortcutState::Pressed
                        && state.shortcut_id.load(Ordering::Relaxed) == key.id()
                    {
                        if let Err(error) = toggle_note(app.clone()) {
                            report_action_error(app, &error);
                        }
                    }
                })
                .build(),
        )
        .setup(setup)
        .on_window_event(|window, event| {
            if window.label() != NOTE {
                return;
            }
            let Some(state) = window.try_state::<Desktop>() else {
                return;
            };
            match event {
                tauri::WindowEvent::Moved(_) if state.ready.load(Ordering::Acquire) => {
                    if let Some(window) = window.get_webview_window(NOTE) {
                        if let Ok(Some(position)) = geometry::capture(&window) {
                            let _ = state.positions.send(position);
                        }
                    }
                }
                tauri::WindowEvent::CloseRequested { api, .. }
                    if !state.exiting.load(Ordering::Acquire) =>
                {
                    api.prevent_close();
                    if let Err(error) = request_action(window.app_handle(), Action::Hide) {
                        report_action_error(window.app_handle(), &error);
                    }
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            load_state,
            save_note,
            save_config,
            set_shortcut,
            history,
            history_bounds,
            reset_position,
            fit_note,
            note_ready,
            note_error,
            toggle_note,
            flush_complete,
            open_panel,
            exit_app,
            restart_app,
            pick_import_directory,
            inspect_import,
            import_data,
        ])
        .build(tauri::generate_context!())
        .expect("FloatNote startup");

    app.run(|app, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event {
            if !app.state::<Desktop>().exiting.load(Ordering::Acquire) {
                api.prevent_exit();
                if let Err(error) = request_action(app, Action::Exit) {
                    report_action_error(app, &error);
                }
            }
        }
    });
}
