/* Startup-failure reporting (CONTEXT §35 Distribution; §38 test boundary).

   Typecase renders through WebView2 on Windows. The installers deal with a
   machine that lacks the runtime — `bundle.windows.webviewInstallMode` in
   tauri.conf.json names the strategy explicitly, and Tauri runs the WebView2
   bootstrapper during setup — but the PORTABLE build is a bare exe with no
   installer to help it: without the runtime it used to die with no
   explanation at all.

   This module turns that into an actionable message. It deliberately does not
   pre-check for the runtime: a probe that guessed wrong could tell a perfectly
   healthy machine that it is broken. Instead the message is produced only
   after `typecase_lib::run()` has actually failed, and it names the runtime as
   the likely cause with the download page, so a portable user has somewhere to
   go.

   Pure and cross-platform where it can be (the wording is unit-tested
   everywhere); the dialog itself is Windows-only and runtime-verified only in
   the WINDOWS_VALIDATION session (§38 — never claimed from Linux). */

/// Microsoft's canonical WebView2 download page (the "Evergreen Bootstrapper").
pub const WEBVIEW2_URL: &str = "https://developer.microsoft.com/microsoft-edge/webview2/";

/// What the user is shown when the window could not be created. `err` is the
/// underlying startup error verbatim — paraphrasing it would hide the real
/// cause when it is something else entirely.
pub fn startup_failure_message(err: &str) -> String {
    format!(
        "Typecase could not open its window.\n\n\
         {err}\n\n\
         The most common cause is a missing Microsoft Edge WebView2 runtime, \
         which Typecase needs to display its interface. If it is not installed \
         on this PC, install it from:\n\
         {WEBVIEW2_URL}\n\n\
         The installer builds add this runtime automatically; the portable \
         build relies on it already being present."
    )
}

/// Show a startup failure to the user. On Windows that means a native message
/// box — a release build has no console (`windows_subsystem = "windows"`), so
/// printing would go nowhere.
#[cfg(windows)]
pub fn report_startup_failure(err: &str) {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_ICONERROR, MB_OK, MB_SETFOREGROUND,
    };

    let message = startup_failure_message(err);
    let title: Vec<u16> = "Typecase — startup failed\0".encode_utf16().collect();
    let body: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: both buffers are NUL-terminated and outlive the call.
    let _ = unsafe {
        MessageBoxW(
            HWND::default(),
            PCWSTR(body.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONERROR | MB_SETFOREGROUND,
        )
    };
}

/// Other platforms have a console (or a journal) where this belongs.
#[cfg(not(windows))]
pub fn report_startup_failure(err: &str) {
    eprintln!("[typecase] {}", startup_failure_message(err));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_keeps_the_error_and_points_at_the_runtime() {
        let m = startup_failure_message("failed to create webview: class not registered");
        // The real error survives verbatim — it is the diagnostic.
        assert!(m.contains("failed to create webview: class not registered"));
        // The actionable part: what is missing and where to get it.
        assert!(m.contains("WebView2"));
        assert!(m.contains(WEBVIEW2_URL));
        assert!(m.contains("portable"));
        assert!(m.contains("installer"));
    }

    #[test]
    fn message_is_ascii_so_any_windows_dialog_renders_it() {
        // MessageBoxW takes UTF-16, but keeping the wording plain avoids any
        // font surprises on stripped images.
        let m = startup_failure_message("boom");
        assert!(m.is_ascii(), "non-ASCII in the startup message: {m}");
    }
}
