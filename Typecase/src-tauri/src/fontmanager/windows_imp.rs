/* Windows FontManager implementation (M6_FONTMANAGER_DESIGN §1).

   Per-user (1809+): copy → HKCU value → AddFontResourceW → WM_FONTCHANGE.
   System scope: HKLM + %WINDIR%\Fonts, performed by a self-elevated helper
   process so the interactive app never runs elevated (§23). Compile-level
   validation happens in CI on windows-latest; runtime truth is the §5 VM
   session (§38). */

use super::{hive_root, FontManager, InstallEntry, InstallRecord};
use crate::library::model::Scope;
use std::fs;
use std::path::{Path, PathBuf};

const REG_FONTS_KEY: &str = r"Software\Microsoft\Windows NT\CurrentVersion\Fonts";

fn user_fonts_dir() -> PathBuf {
    // %LOCALAPPDATA%\Microsoft\Windows\Fonts (per-user fonts, 1809+)
    let base = std::env::var("LOCALAPPDATA").expect("LOCALAPPDATA must exist on Windows");
    Path::new(&base)
        .join("Microsoft")
        .join("Windows")
        .join("Fonts")
}

fn system_fonts_dir() -> PathBuf {
    let windir = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    Path::new(&windir).join("Fonts")
}

pub(crate) fn fonts_dir(scope: Scope) -> PathBuf {
    match scope {
        Scope::User => user_fonts_dir(),
        Scope::System => system_fonts_dir(),
    }
}

/// Registry value name → writer for the scope's hive.
fn open_fonts_key(scope: Scope, write: bool) -> Result<windows::Win32::System::Registry::HKEY, String> {
    use windows::Win32::System::Registry::*;
    use windows::core::PCWSTR;
    let wide_key: Vec<u16> = REG_FONTS_KEY
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut key = HKEY::default();
    let (root, access) = match (scope, write) {
        (Scope::User, false) => (HKEY_CURRENT_USER, KEY_READ),
        (Scope::User, true) => (HKEY_CURRENT_USER, KEY_SET_VALUE),
        (Scope::System, false) => (HKEY_LOCAL_MACHINE, KEY_READ),
        (Scope::System, true) => (HKEY_LOCAL_MACHINE, KEY_SET_VALUE),
    };
    // SAFETY: root is a valid hive handle; `wide` is a NUL-terminated UTF-16
    // literal path owned for the duration of the (synchronous) call.
    let status = unsafe {
        RegOpenKeyExW(root, PCWSTR(wide_key.as_ptr()), 0, access, &mut key)
    };
    if status.is_ok() {
        Ok(key)
    } else {
        Err(format!(
            "cannot open {}\\{REG_FONTS_KEY}: WIN32_ERROR({})",
            hive_root(scope),
            status.0
        ))
    }
}

fn set_value(scope: Scope, value_name: &str, data: &str) -> Result<(), String> {
    use windows::Win32::System::Registry::*;
    use windows::core::PCWSTR;
    let key = open_fonts_key(scope, true)?;
    let name: Vec<u16> = value_name.encode_utf16().chain(std::iter::once(0)).collect();
    let data: Vec<u16> = data.encode_utf16().chain(std::iter::once(0)).collect();
    // UTF-16 code units little-endian, as REG_SZ expects.
    let bytes: Vec<u8> = data.iter().flat_map(|u| u.to_le_bytes()).collect();
    // SAFETY: all buffers are valid for the synchronous call; key is an open
    // handle with KEY_SET_VALUE.
    let status = unsafe {
        RegSetValueExW(key, PCWSTR(name.as_ptr()), 0, REG_SZ, Some(&bytes))
    };
    let _ = unsafe { RegCloseKey(key) };
    if status.is_ok() {
        Ok(())
    } else {
        Err(format!("cannot write value \"{value_name}\": WIN32_ERROR({})", status.0))
    }
}

fn delete_value(scope: Scope, value_name: &str) -> Result<(), String> {
    use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
    use windows::Win32::System::Registry::*;
    use windows::core::PCWSTR;
    let key = open_fonts_key(scope, true)?;
    let name: Vec<u16> = value_name.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: `name` is a valid NUL-terminated UTF-16 buffer for the call.
    let status = unsafe { RegDeleteValueW(key, PCWSTR(name.as_ptr())) };
    let _ = unsafe { RegCloseKey(key) };
    match status {
        s if s == ERROR_FILE_NOT_FOUND || s.is_ok() => Ok(()),
        s => Err(format!(
            "cannot delete value \"{value_name}\": WIN32_ERROR({})",
            s.0
        )),
    }
}

fn value_exists(scope: Scope, value_name: &str) -> bool {
    use windows::Win32::System::Registry::*;
    use windows::core::PCWSTR;
    let Ok(key) = open_fonts_key(scope, false) else {
        return false;
    };
    let name: Vec<u16> = value_name.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: `name` is a valid NUL-terminated UTF-16 buffer for the call.
    let status = unsafe { RegQueryValueExW(key, PCWSTR(name.as_ptr()), None, None, None, None) };
    let _ = unsafe { RegCloseKey(key) };
    status.is_ok()
}

/// Load/unload the font for this logon session. Returns the number of fonts
/// added/removed (0 is not necessarily an error after a fresh copy).
fn add_font_resource(path: &str) -> i32 {
    use windows::Win32::Graphics::Gdi::AddFontResourceW;
    use windows::core::PCWSTR;
    let w = wide(path);
    // SAFETY: `w` is a NUL-terminated UTF-16 path owned for the call.
    unsafe { AddFontResourceW(PCWSTR(w.as_ptr())) }
}

fn remove_font_resource(path: &str) {
    use windows::Win32::Graphics::Gdi::RemoveFontResourceW;
    use windows::core::PCWSTR;
    let w = wide(path);
    // SAFETY: as above.
    unsafe { RemoveFontResourceW(PCWSTR(w.as_ptr())) };
}

/// WM_FONTCHANGE: tell running apps the font set changed (§5.8 validation).
fn broadcast_font_change() {
    use windows::Win32::UI::WindowsAndMessaging::{SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG};
    use windows::Win32::Foundation::{WPARAM, LPARAM};
    const WM_FONTCHANGE: u32 = 0x001D;
    // SAFETY: HWND_BROADCAST with no buffer; the timeout bounds hung windows.
    // WPARAM/LPARAM are passed directly (Param impls), not wrapped in Option.
    unsafe {
        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_FONTCHANGE,
            WPARAM(0),
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            1000,
            None,
        );
    }
}

pub struct WindowsFontManager;

fn wide(s: &str) -> Vec<u16> {
    // std str method — no OsStrExt import needed
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Install/uninstall entry points that respect scope: user operations run
/// in-process; system operations re-exec Typecase itself elevated for exactly
/// that one operation (§23 — the interactive app never runs elevated).
pub fn install_scoped(record: &InstallRecord) -> Result<(), String> {
    match record.scope {
        Scope::User => WindowsFontManager.install(record),
        Scope::System => spawn_elevated(ELEVATE_INSTALL_ARG, record),
    }
}

pub fn uninstall_scoped(record: &InstallRecord) -> Result<(), String> {
    match record.scope {
        Scope::User => WindowsFontManager.uninstall(record),
        Scope::System => spawn_elevated(ELEVATE_UNINSTALL_ARG, record),
    }
}

/// M7: one-shot elevation for a system-scope EXTERNAL font removal. The
/// helper child receives the value name + path directly (no record file).
pub fn spawn_elevated_external(
    mode: &str,
    value_name: &str,
    file_path: &str,
) -> Result<(), String> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
    use windows::Win32::UI::Shell::{
        SHELLEXECUTEINFOW, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOCLOSEPROCESS, ShellExecuteExW,
    };
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let exe = std::env::current_exe().map_err(|e| format!("cannot resolve current exe: {e}"))?;
    let verb = wide("runas");
    let file = wide(&exe.to_string_lossy());
    let params = wide(&format!(
        "{mode} \"{}\" \"{}\"",
        value_name.replace('"', ""),
        file_path.replace('"', "")
    ));

    let mut sei = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_FLAG_NO_UI,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(params.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    // SAFETY: buffers alive across the synchronous call; NOCLOSEPROCESS
    // yields the child handle we wait on.
    unsafe { ShellExecuteExW(&mut sei) }
        .map_err(|e| format!("elevation request failed (declined?): {e}"))?;
    if sei.hProcess.is_invalid() {
        return Err("elevation returned no process handle".into());
    }
    let wait = unsafe { WaitForSingleObject(sei.hProcess, 120_000) };
    if wait != WAIT_OBJECT_0 {
        return Err("elevated operation timed out".into());
    }
    let mut code: u32 = 0;
    let _ = unsafe { GetExitCodeProcess(sei.hProcess, &mut code) };
    let _ = unsafe { CloseHandle(sei.hProcess) };
    if code == 0 {
        Ok(())
    } else {
        Err(format!(
            "elevated removal failed (exit {code}) — consent may have been declined"
        ))
    }
}

/* ---- scoped elevation (M6_FONTMANAGER_DESIGN §1) ----

   System-scope registry/file writes need admin consent; §23 forbids running
   the whole app elevated. So the parent re-execs Typecase itself with an
   elevation-only subcommand, waits, and checks the exit code. The elevated
   child performs exactly the recorded entries and exits. */

pub const ELEVATE_INSTALL_ARG: &str = "--typecase-elevated-install";
pub const ELEVATE_UNINSTALL_ARG: &str = "--typecase-elevated-uninstall";

/// Entry point for the elevated helper process: Some(exit code) when this
/// process was started with an elevation-only subcommand.
pub fn run_elevated_from_args() -> Option<i32> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        return None;
    }
    let (mode, path) = (args[1].as_str(), args[2].as_str());
    if mode != ELEVATE_INSTALL_ARG && mode != ELEVATE_UNINSTALL_ARG {
        return None;
    }
    let text = fs::read_to_string(path).ok()?;
    let record: InstallRecord = serde_json::from_str(&text).ok()?;
    let mgr = WindowsFontManager;
    let result = match mode {
        ELEVATE_INSTALL_ARG => mgr.install(&record),
        _ => mgr.uninstall(&record),
    };
    Some(match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("[typecase] elevated operation failed: {e}");
            1
        }
    })
}

fn spawn_elevated(mode: &str, record: &InstallRecord) -> Result<(), String> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::WAIT_OBJECT_0;
    use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
    use windows::Win32::UI::Shell::{SHELLEXECUTEINFOW, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOCLOSEPROCESS, ShellExecuteExW};
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let record_path = std::env::temp_dir()
        .join(format!("typecase-elevate-{}-{}.json", record.id, std::process::id()));
    let text = serde_json::to_string(record).map_err(|e| format!("serialize record: {e}"))?;
    fs::write(&record_path, text).map_err(|e| format!("cannot write elevation record: {e}"))?;

    let exe = std::env::current_exe().map_err(|e| format!("cannot resolve current exe: {e}"))?;
    let verb = wide("runas");
    let file = wide(&exe.to_string_lossy());
    let params = wide(&format!("{mode} \"{}\"", record_path.display()));

    let mut sei = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_FLAG_NO_UI,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(params.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    // SAFETY: every PCWSTR points to a buffer alive across the synchronous
    // call; SEE_MASK_NOCLOSEPROCESS yields the child handle we wait on.
    unsafe { ShellExecuteExW(&mut sei) }
        .map_err(|e| format!("elevation request failed (declined?): {e}"))?;
    if sei.hProcess.is_invalid() {
        return Err("elevation returned no process handle".into());
    }
    let wait = unsafe { WaitForSingleObject(sei.hProcess, 120_000) };
    if wait != WAIT_OBJECT_0 {
        return Err("elevated operation timed out".into());
    }
    let mut code: u32 = 0;
    let _ = unsafe { GetExitCodeProcess(sei.hProcess, &mut code) };
    let _ = unsafe { windows::Win32::Foundation::CloseHandle(sei.hProcess) };
    let _ = fs::remove_file(&record_path);
    if code == 0 {
        Ok(())
    } else {
        Err(format!("elevated operation failed (exit {code}) — consent may have been declined"))
    }
}

impl FontManager for WindowsFontManager {
    fn install(&self, record: &InstallRecord) -> Result<(), String> {
        let scope = record.scope;
        let dir = fonts_dir(scope);
        fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;

        // Copy files first; registry/session writes come after so a copy
        // failure leaves no half-registered family.
        let mut copied: Vec<InstallEntry> = Vec::new();
        for e in &record.entries {
            let target = PathBuf::from(&e.file);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .map_err(|err| format!("cannot create {}: {err}", parent.display()))?;
            }
            fs::copy(&e.source, &target).map_err(|err| {
                // roll back copies made so far in this call
                for c in &copied {
                    let _ = fs::remove_file(&c.file);
                }
                format!("cannot copy {} → {}: {err}", e.source, e.file)
            })?;
            copied.push(e.clone());
        }

        for e in &copied {
            set_value(scope, &e.value_name, &e.file)?;
        }
        for e in &copied {
            add_font_resource(&e.file);
        }
        broadcast_font_change();
        Ok(())
    }

    fn uninstall(&self, record: &InstallRecord) -> Result<(), String> {
        let scope = record.scope;
        // Registry first (the persistence source), then session unload, then files.
        for e in &record.entries {
            delete_value(scope, &e.value_name)?;
        }
        for e in &record.entries {
            remove_font_resource(&e.file);
        }
        for e in &record.entries {
            let p = PathBuf::from(&e.file);
            if p.exists() {
                fs::remove_file(&p).map_err(|err| format!("cannot delete {}: {err}", e.file))?;
            }
        }
        broadcast_font_change();
        Ok(())
    }

    fn is_installed(&self, record: &InstallRecord) -> bool {
        !record.entries.is_empty()
            && record
                .entries
                .iter()
                .all(|e| value_exists(record.scope, &e.value_name))
    }
}
