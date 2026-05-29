#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayAction {
    Capture,
    ToggleHotkeys,
    OpenSettings,
    OpenHistory,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayMenuModel {
    pub actions: Vec<TrayAction>,
}

impl Default for TrayMenuModel {
    fn default() -> Self {
        Self {
            actions: vec![
                TrayAction::Capture,
                TrayAction::ToggleHotkeys,
                TrayAction::OpenSettings,
                TrayAction::OpenHistory,
                TrayAction::Quit,
            ],
        }
    }
}

pub fn install(app: &mut tauri::App) -> tauri::Result<()> {
    use tauri::{
        Manager,
        menu::{Menu, MenuItem},
        tray::TrayIconBuilder,
    };

    log::info!("installing tray icon");
    let labels = labels(current_language(app.handle()));
    let capture = MenuItem::with_id(app, "capture", labels.capture, true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", labels.settings, true, None::<&str>)?;
    let history = MenuItem::with_id(app, "history", labels.history, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", labels.quit, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&capture, &settings, &history, &quit])?;

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip(labels.app_name)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => {
                log::info!("tray quit selected");
                app.exit(0);
            }
            "capture" => {
                log::info!("tray capture selected");
                if let Err(error) = crate::capture::launcher::launch_capture_overlay(app) {
                    log::error!("failed to launch capture overlay from tray: {error}");
                }
            }
            "settings" | "history" => {
                log::info!("tray {} selected", event.id.as_ref());
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                } else {
                    log::error!("main window not found for tray action");
                }
            }
            _ => log::warn!("unknown tray event id={}", event.id.as_ref()),
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(app)?;
    log::info!("tray icon installed");
    Ok(())
}

pub fn refresh(app: &tauri::AppHandle, language: &str) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};

    log::info!("refreshing tray language={language}");
    let labels = labels(language);
    let capture = MenuItem::with_id(app, "capture", labels.capture, true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", labels.settings, true, None::<&str>)?;
    let history = MenuItem::with_id(app, "history", labels.history, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", labels.quit, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&capture, &settings, &history, &quit])?;

    if let Some(tray) = app.tray_by_id("main") {
        tray.set_menu(Some(menu))?;
        tray.set_tooltip(Some(labels.app_name))?;
    } else {
        log::warn!("tray icon not found while refreshing");
    }

    log::info!("tray refreshed");
    Ok(())
}

fn current_language(app: &tauri::AppHandle) -> String {
    use tauri::Manager;

    app.state::<std::sync::Mutex<crate::shell_state::ShellState>>()
        .lock()
        .ok()
        .map(|state| state.settings().interface.language.clone())
        .unwrap_or_else(|| "zh-CN".to_owned())
}

#[derive(Debug, Clone, Copy)]
struct TrayLabels {
    app_name: &'static str,
    capture: &'static str,
    settings: &'static str,
    history: &'static str,
    quit: &'static str,
}

fn labels(language: impl AsRef<str>) -> TrayLabels {
    match language.as_ref() {
        "en" => TrayLabels {
            app_name: "snap pin",
            capture: "Start capture",
            settings: "Settings",
            history: "History",
            quit: "Quit",
        },
        "ja" => TrayLabels {
            app_name: "snap pin",
            capture: "キャプチャ開始",
            settings: "設定",
            history: "履歴",
            quit: "終了",
        },
        "ko" => TrayLabels {
            app_name: "snap pin",
            capture: "캡처 시작",
            settings: "설정",
            history: "기록",
            quit: "종료",
        },
        "fr" => TrayLabels {
            app_name: "snap pin",
            capture: "Demarrer la capture",
            settings: "Reglages",
            history: "Historique",
            quit: "Quitter",
        },
        "de" => TrayLabels {
            app_name: "snap pin",
            capture: "Aufnahme starten",
            settings: "Einstellungen",
            history: "Verlauf",
            quit: "Beenden",
        },
        _ => TrayLabels {
            app_name: "贴图钉",
            capture: "开始截图",
            settings: "设置",
            history: "历史",
            quit: "退出",
        },
    }
}
