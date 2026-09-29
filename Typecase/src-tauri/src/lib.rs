// Public for the network integration test (tests/download_pipeline.rs).
pub mod catalog;
mod commands;
pub mod downloads;
pub mod exports;
pub mod externalfonts;
pub mod fontmanager;
pub mod library;
/// §35: reporting a startup failure in a way the user can act on (the
/// portable build has no installer to fall back on).
pub mod startup;

use tauri::Manager;

/// Start the UI. Returns the error rather than panicking, so `main` can show
/// the user what happened instead of a silent exit — the portable exe is the
/// case that matters (no installer, no console).
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() -> Result<(), String> {
    let app = tauri::Builder::default()
        // M5: serve cached font files (fonts/<id>/<sha8>.ttf) to the webview.
        // Strict path validation lives in downloads::serve_font_file (§31);
        // this handler is a thin adapter over it.
        .register_uri_scheme_protocol("font", |ctx, request| {
            let state = ctx.app_handle().state::<commands::TypecaseState>();
            let uri = request.uri().to_string();
            let (status, content_type, body) =
                match crate::downloads::serve_font_file(&state.data_dir, &uri) {
                    Ok((bytes, content_type)) => {
                        #[cfg(debug_assertions)]
                        eprintln!("[typecase] font:// served {} ({} bytes)", uri, bytes.len());
                        (200u16, content_type, bytes)
                    }
                    Err(status) => (status, "text/plain", Vec::new()),
                };
            tauri::http::Response::builder()
                .status(status)
                .header("Content-Type", content_type)
                .header("Cache-Control", "no-store")
                .body(body)
                .expect("infallible font:// response")
        })
        .setup(|app| {
            app.manage(commands::init_state(app));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_catalog,
            commands::get_library,
            commands::get_font_details,
            commands::get_font_sources,
            commands::download_font,
            commands::delete_cached_family,
            commands::install_font,
            commands::uninstall_font,
            commands::get_installed_fonts,
            commands::cache_external_font,
            commands::remove_external_font,
            commands::refresh_catalog,
            commands::apply_catalog_refresh,
            commands::export_font,
            commands::apply_window_theme,
            ping,
            app_version,
        ])
        .build(tauri::generate_context!())
        .map_err(|e| format!("cannot create the application window: {e}"))?;
    // The window and its webview are created by `build`, so a missing or
    // broken WebView2 runtime surfaces above; once the loop starts, Tauri owns
    // error handling (its `run` is infallible).
    app.run(|_, _| {});
    Ok(())
}

/// Scaffold IPC health check.
#[tauri::command]
fn ping() -> String {
    "typecase-ipc-ok".to_string()
}

#[tauri::command]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
