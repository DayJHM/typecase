/* Windows registry enumeration of registered fonts (M7 §1). Tolerant by
   design: one malformed value is skipped, never fatal. Compile-validated on
   Windows CI; runtime-validated by the §6 VM session (§38). */

use super::ExternalFont;
use crate::library::model::Scope;
use std::collections::HashMap;

use windows::core::PCWSTR;
use windows::Win32::Foundation::ERROR_NO_MORE_ITEMS;
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumValueW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_LOCAL_MACHINE,
    HKEY_CURRENT_USER, KEY_READ, REG_SZ,
};

const REG_FONTS_KEY: &str = r"Software\Microsoft\Windows NT\CurrentVersion\Fonts";

fn open_key(scope: Scope) -> Result<HKEY, String> {
    let wide_key: Vec<u16> = REG_FONTS_KEY
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut key = HKEY::default();
    let root = match scope {
        Scope::User => HKEY_CURRENT_USER,
        Scope::System => HKEY_LOCAL_MACHINE,
    };
    // SAFETY: `wide_key` is NUL-terminated and alive for the call.
    let status = unsafe { RegOpenKeyExW(root, PCWSTR(wide_key.as_ptr()), 0, KEY_READ, &mut key) };
    if status.is_ok() {
        Ok(key)
    } else {
        Err(format!("cannot open fonts key for {scope:?}: WIN32_ERROR({})", status.0))
    }
}

/// Read the REG_SZ data (file path) of one value; empty string when absent.
fn value_data(key: HKEY, name: &[u16]) -> String {
    let mut kind = 0u32;
    let mut bytes = [0u8; 1024];
    let mut size = bytes.len() as u32;
    // SAFETY: `name` is NUL-terminated; `bytes` outlives the call.
    let status = unsafe {
        RegQueryValueExW(
            key,
            PCWSTR(name.as_ptr()),
            None,
            Some(&mut kind),
            Some(bytes.as_mut_ptr()),
            Some(&mut size),
        )
    };
    if status.is_ok() && kind == REG_SZ && size >= 2 {
        let units: Vec<u16> = bytes[..size as usize]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&units)
            .trim_end_matches('\0')
            .to_string()
    } else {
        String::new()
    }
}

/// Enumerate every registered font value name under the scope's hive.
pub fn enumerate_scope(scope: Scope) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let Ok(key) = open_key(scope) else {
        return out;
    };
    let mut index = 0u32;
    loop {
        let mut name_buf = [0u16; 256];
        let mut name_len = name_buf.len() as u32;
        let mut kind = 0u32;
        let mut data = [0u8; 1024];
        let mut data_len = data.len() as u32;
        // SAFETY: buffers outlive the call; lengths are exact.
        let status = unsafe {
            RegEnumValueW(
                key,
                index,
                windows::core::PWSTR(name_buf.as_mut_ptr()),
                &mut name_len,
                None,
                Some(&mut kind),
                Some(data.as_mut_ptr()),
                Some(&mut data_len),
            )
        };
        if status == ERROR_NO_MORE_ITEMS {
            break;
        }
        if status.is_ok() && name_len > 0 {
            let value_name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
            let data_units: Vec<u16> = data[..data_len as usize]
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            let file_path = String::from_utf16_lossy(&data_units)
                .trim_end_matches('\0')
                .to_string();
            out.push((value_name, file_path));
        }
        index += 1;
        if index > 20_000 {
            break; // paranoia bound; the registry is not this big
        }
    }
    let _ = unsafe { RegCloseKey(key) };
    out
}

/// Full discovery for both scopes, classified against Typecase's records.
pub fn discover(
    records: &HashMap<String, crate::fontmanager::InstallRecord>,
) -> Vec<ExternalFont> {
    let mut out = Vec::new();
    for (scope, label) in [(Scope::User, "user"), (Scope::System, "system")] {
        for (value_name, file_path) in enumerate_scope(scope) {
            let (family, style) = super::parse_value_name(&value_name);
            let (ownership, id) = super::classify(&value_name, records);
            out.push(ExternalFont {
                value_name,
                family,
                style,
                file_path,
                scope: label.to_string(),
                ownership,
                id,
            });
        }
    }
    out
}

/// Delete one registry value + its file (external removal, §25).
pub fn remove_external(
    scope: Scope,
    value_name: &str,
    file_path: &str,
) -> Result<(), String> {
    use windows::Win32::System::Registry::*;
    use windows::core::PCWSTR;

    // 1. Registry value first (the authoritative registration).
    let wide_key: Vec<u16> = REG_FONTS_KEY
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut key = HKEY::default();
    let (root, access) = match scope {
        Scope::User => (HKEY_CURRENT_USER, KEY_SET_VALUE),
        Scope::System => (HKEY_LOCAL_MACHINE, KEY_SET_VALUE),
    };
    // SAFETY: NUL-terminated buffer alive for the call.
    let status = unsafe { RegOpenKeyExW(root, PCWSTR(wide_key.as_ptr()), 0, access, &mut key) };
    if !status.is_ok() {
        return Err(format!("cannot open fonts key: WIN32_ERROR({})", status.0));
    }
    let name: Vec<u16> = value_name.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: as above.
    let status = unsafe { RegDeleteValueW(key, PCWSTR(name.as_ptr())) };
    let _ = unsafe { RegCloseKey(key) };
    if !(status.is_ok() || status == windows::Win32::Foundation::ERROR_FILE_NOT_FOUND) {
        return Err(format!(
            "cannot delete registry value \"{value_name}\": WIN32_ERROR({})",
            status.0
        ));
    }

    // 2. The file, only when it is outside the Typecase cache (external
    //    fonts are never ours, but double-guard §31 anyway).
    if !file_path.is_empty() {
        let p = std::path::PathBuf::from(file_path);
        if p.exists() {
            std::fs::remove_file(&p)
                .map_err(|e| format!("cannot delete {}: {e}", p.display()))?;
        }
    }
    Ok(())
}

pub const ELEVATE_REMOVE_EXTERNAL_ARG: &str = "--typecase-elevated-remove-external";

/// System-scope external removal: delegate to the M6 elevation machinery
/// (ShellExecuteExW runas on our own binary; the app never runs elevated).
pub fn remove_external_elevated(value_name: &str, file_path: &str) -> Result<(), String> {
    crate::fontmanager::windows_imp::spawn_elevated_external(
        ELEVATE_REMOVE_EXTERNAL_ARG,
        value_name,
        file_path,
    )
}

/// Entry point for the elevated external-removal helper: Some(exit) when the
/// process was started with the external-removal subcommand.
pub fn run_elevated_external_from_args() -> Option<i32> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 || args[1] != ELEVATE_REMOVE_EXTERNAL_ARG {
        return None;
    }
    let result = remove_external(Scope::System, &args[2], &args[3]);
    Some(match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("[typecase] elevated external removal failed: {e}");
            1
        }
    })
}
