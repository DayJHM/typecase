// Prevents an additional console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // M6/M7: elevation-only mode — when Typecase re-execs itself for a single
    // system-scope font operation (§23), the child performs it and exits
    // without ever starting the UI.
    #[cfg(windows)]
    {
        if let Some(code) = typecase_lib::fontmanager::windows_imp::run_elevated_from_args() {
            std::process::exit(code);
        }
        if let Some(code) = typecase_lib::externalfonts::run_elevated_from_args() {
            std::process::exit(code);
        }
    }
    typecase_lib::run()
}
