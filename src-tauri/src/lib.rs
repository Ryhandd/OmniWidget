pub mod core;
pub mod plugins;

use tauri::Manager;
use tauri_plugin_global_shortcut::{Builder as ShortcutBuilder, ShortcutState};

use crate::core::{lifecycle, state::ShellState, storage::Storage, tray, window_manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        // Harus plugin PERTAMA: instance kedua langsung keluar dan memunculkan widget yang sudah jalan.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| window_manager::toggle_show(app)))
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--autostart"])))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            ShortcutBuilder::new()
                .with_shortcuts(["ctrl+alt+space"])
                .expect("hotkey tidak valid")
                .with_handler(|app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        window_manager::toggle(app);
                    }
                })
                .build(),
        )
        .on_window_event(|window, event| {
            // Overlay capture ditutup dengan cara apa pun (Esc, Alt+F4): buang snapshot & tampilkan widget lagi.
            if window.label() == crate::plugins::capture::OVERLAY {
                if let tauri::WindowEvent::Destroyed = event {
                    crate::plugins::capture::cleanup(window.app_handle());
                }
                return;
            }
            if window.label() != lifecycle::MAIN {
                return;
            }
            let Some(w) = window.app_handle().get_webview_window(window.label()) else { return };
            match event {
                tauri::WindowEvent::Moved(_) => window_manager::on_moved(&w),
                tauri::WindowEvent::ScaleFactorChanged { .. } => window_manager::on_display_changed(&w),
                _ => {}
            }
        })
        .setup(|app| {
            app.manage(ShellState::new());
            // SQLite (WAL) di app_data: draft & pengaturan modul. Dibuka sekali; dilepas bersama proses.
            let db_path = app.path().app_data_dir()?.join("omniwidget.db");
            app.manage(Storage::open(&db_path)?);
            // Satu timer untuk reminder & Pomodoro (TDD 4.3/4.4).
            app.manage(crate::core::scheduler::Scheduler::new());
            app.manage(crate::plugins::capture::CaptureState::default());
            app.manage(crate::plugins::qr::QrState::default());
            app.manage(crate::plugins::sysmon::SysState::default());
            crate::plugins::todo::setup(app.handle())?;
            crate::plugins::pomodoro::setup(app.handle())?;
            crate::core::scheduler::start(app.handle());
            // Clipboard Manager: resident (watcher event-driven), butuh Storage di atas.
            crate::plugins::clipboard::setup(app.handle())?;
            crate::plugins::converter::setup(app.handle());
            crate::plugins::spotify::setup(app.handle())?;
            // Now-playing Spotify Free lewat Windows SMTC (tanpa Premium).
            crate::plugins::nowplaying::setup(app.handle())?;
            tray::init(app.handle())?;
            lifecycle::spawn_idle_watcher(app.handle().clone());
            // Penjaga RAM: kembalikan working set ke OS saat memori sistem menekan, tanpa menutup widget.
            crate::plugins::sysmon::spawn_memory_guard(app.handle().clone());
            // Jendela dibuat tersembunyi (tauri.conf.json): posisikan dulu, baru tampil -> tanpa kedip.
            if let Some(w) = app.get_webview_window(lifecycle::MAIN) {
                window_manager::show(&w, window_manager::Placement::TopCenter);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            window_manager::window_set_mode,
            window_manager::window_snap,
            window_manager::window_set_lock_snap,
            window_manager::window_cursor_inside,
            lifecycle::shell_activity,
            lifecycle::shell_get_state,
            lifecycle::shell_set_sleep_after,
            lifecycle::shell_deep_sleep,
            crate::plugins::converter::converter_probe,
            crate::plugins::converter::converter_start,
            crate::plugins::converter::converter_cancel,
            crate::plugins::converter::staging::converter_stage_upload,
            crate::plugins::llm::llm_get_config,
            crate::plugins::llm::llm_set_config,
            crate::plugins::llm::llm_complete,
            crate::plugins::spotify::spotify_status,
            crate::plugins::spotify::spotify_set_client_id,
            crate::plugins::spotify::spotify_login,
            crate::plugins::spotify::spotify_login_cancel,
            crate::plugins::spotify::spotify_logout,
            crate::plugins::spotify::spotify_player,
            crate::plugins::spotify::spotify_devices,
            crate::plugins::spotify::spotify_control,
            crate::plugins::spotify::spotify_cover,
            crate::plugins::nowplaying::nowplaying_state,
            crate::plugins::nowplaying::nowplaying_control,
            crate::plugins::nowplaying::nowplaying_volume,
            crate::plugins::nowplaying::nowplaying_set_volume,
            crate::plugins::nowplaying::nowplaying_open_uri,
            crate::core::storage::kv_get,
            crate::core::storage::kv_set,
            crate::core::storage::kv_delete,
            crate::core::clipboard::clipboard_write,
            crate::plugins::clipboard::clipboard_snapshot,
            crate::plugins::clipboard::clipboard_thumb,
            crate::plugins::clipboard::clipboard_text,
            crate::plugins::clipboard::clipboard_paste,
            crate::plugins::clipboard::clipboard_pin,
            crate::plugins::clipboard::clipboard_remove,
            crate::plugins::clipboard::clipboard_clear,
            crate::plugins::clipboard::clipboard_set_settings,
            crate::core::autostart::app_autostart_get,
            crate::core::autostart::app_autostart_set,
            crate::plugins::todo::todo_list,
            crate::plugins::todo::todo_add,
            crate::plugins::todo::todo_update,
            crate::plugins::todo::todo_set_done,
            crate::plugins::todo::todo_snooze,
            crate::plugins::todo::todo_delete,
            crate::plugins::todo::todo_clear_done,
            crate::plugins::pomodoro::pomo_get,
            crate::plugins::pomodoro::pomo_start,
            crate::plugins::pomodoro::pomo_pause,
            crate::plugins::pomodoro::pomo_skip,
            crate::plugins::pomodoro::pomo_reset,
            crate::plugins::pomodoro::pomo_set_config,
            crate::plugins::brightness::brightness_get,
            crate::plugins::brightness::brightness_set,
            crate::plugins::capture::capture_start,
            crate::plugins::capture::capture_patch,
            crate::plugins::capture::capture_pick,
            crate::plugins::capture::capture_qr,
            crate::plugins::capture::capture_annotate,
            crate::plugins::capture::capture_close,
            crate::plugins::capture::capture_recent,
            crate::plugins::qr::qr_generate,
            crate::plugins::qr::qr_export,
            crate::plugins::qr::qr_scan_file,
            crate::plugins::qr::qr_scan_clipboard,
            crate::plugins::qr::qr_take_last,
            crate::plugins::qr::qr_open_url,
            crate::plugins::sysmon::sysmon_sample,
            crate::plugins::sysmon::sysmon_top,
            crate::plugins::sysmon::sysmon_close,
            crate::plugins::sysmon::sysmon_trim_self,
            crate::plugins::sysmon::sysmon_stop,
        ])
        .build(tauri::generate_context!())
        .expect("error while building omniwidget application");

    app.run(|app, event| match event {
        // Deep Sleep menghancurkan jendela terakhir; itu BUKAN permintaan keluar dari pengguna.
        // Keluar eksplisit (tray "Keluar" -> app.exit(0)) membawa code = Some(0) dan tetap lolos.
        tauri::RunEvent::ExitRequested { code, api, .. } => {
            if code.is_none() {
                api.prevent_exit();
            }
        }
        // Keluar sungguhan (bukan Deep Sleep): terapkan "bersihkan saat keluar" & simpan/bersihkan riwayat.
        tauri::RunEvent::Exit => crate::plugins::clipboard::on_exit(app),
        _ => {}
    });
}
