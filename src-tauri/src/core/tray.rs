//! Tray icon: satu-satunya UI saat Deep Sleep. Klik kiri = tampil/sembunyi (bangun bila tidur).

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::core::lifecycle;
use crate::core::window_manager;

const ID: &str = "main";

pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Tampilkan", true, None::<&str>)?;
    let reset = MenuItem::with_id(app, "reset", "Reset posisi", true, None::<&str>)?;
    let sleep = MenuItem::with_id(app, "sleep", "Deep Sleep", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Keluar", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &reset, &sleep, &PredefinedMenuItem::separator(app)?, &quit])?;

    let mut builder = TrayIconBuilder::with_id(ID)
        .tooltip("OmniWidget")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => lifecycle::wake(app),
            "reset" => match app.get_webview_window(lifecycle::MAIN) {
                Some(w) => {
                    let _ = window_manager::snap_to_top_center(&w, 140);
                }
                None => lifecycle::wake(app), // bangun selalu top-center
            },
            "sleep" => {
                if !lifecycle::enter_deep_sleep(app) {
                    let _ = app
                        .notification()
                        .builder()
                        .title("OmniWidget")
                        .body("Tidak bisa tidur: konversi masih berjalan.")
                        .show();
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                window_manager::toggle(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

pub fn set_sleeping(app: &AppHandle, sleeping: bool) {
    if let Some(t) = app.tray_by_id(ID) {
        let _ = t.set_tooltip(Some(if sleeping { "OmniWidget (Deep Sleep)" } else { "OmniWidget" }));
    }
}
