// Public for the network integration test (tests/download_pipeline.rs).
pub mod catalog;
mod commands;
pub mod downloads;
pub mod exports;
pub mod externalfonts;
pub mod fontmanager;
pub mod library;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
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
            // One-shot dev-only M8 check: exercises the real command paths
            // (export → live refresh → apply → reloaded catalog) inside the
            // native window and leaves disk evidence. Removed after the run.
            #[cfg(debug_assertions)]
            if std::env::var("TYPECASE_M8_CHECK").is_ok() {
                let handle = app.handle().clone();
                std::thread::spawn(move || match run_m8_check(&handle) {
                    Ok(lines) => {
                        for l in lines {
                            eprintln!("[m8-check] {l}");
                        }
                        let _ = std::fs::write("m8-check-done", "ok\n");
                    }
                    Err(e) => {
                        eprintln!("[m8-check] FAILED: {e}");
                        let _ = std::fs::write("m8-check-done", format!("fail {e}\n"));
                    }
                });
            }
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
            commands::remove_external_font,
            commands::refresh_catalog,
            commands::apply_catalog_refresh,
            commands::export_font,
            commands::apply_window_theme,
            ping,
            app_version,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// One-shot dev-only M8 verification (removed after the run; see STAGE2_PLAN
/// §10 for the precedent). Calls the real command functions directly.
#[cfg(debug_assertions)]
fn run_m8_check(app: &tauri::AppHandle) -> Result<Vec<String>, String> {
    use tauri::Manager;
    let state = app.state::<commands::TypecaseState>();
    let mut lines = Vec::new();

    // 1. Export the cached `inter` family through the real command body.
    let out = commands::export_font_inner(&state, "inter")?;
    lines.push(format!(
        "export: {} file(s), source={}, path={}",
        out.file_count,
        out.source,
        std::path::Path::new(&out.file).file_name().unwrap_or_default().to_string_lossy()
    ));
    let written = std::fs::read(&out.file).map_err(|e| format!("export unreadable: {e}"))?;
    let locals = written.windows(4).filter(|w| *w == [0x50, 0x4b, 0x03, 0x04]).count();
    lines.push(format!("zip entries: {locals}"));
    if locals < 2 {
        return Err("zip has too few entries".into());
    }

    // 2. Live refresh through the real command body (network required).
    let diff = tauri::async_runtime::block_on(commands::refresh_catalog_inner(&state))?;
    lines.push(format!(
        "refresh: +{} ~{} -{} of {} (unchanged={})",
        diff.added, diff.changed, diff.removed, diff.total, diff.unchanged
    ));

    // 3. Apply and reload — the disk catalog takes over.
    let applied = commands::apply_catalog_refresh_inner(&state)?;
    lines.push(format!(
        "apply: {} records, {} marked removed-from-source",
        applied.total, applied.removed_total
    ));
    let reloaded = crate::catalog::Catalog::for_dir(&state.data_dir);
    lines.push(format!("reload: {} families from disk", reloaded.records.len()));
    if reloaded.records.len() < 1000 {
        return Err("reloaded catalog implausibly small".into());
    }
    Ok(lines)
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
