// Prevents an additional console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // M6/M7: elevation-only mode — when Typecase re-execs itself for a single
    // system-scope font operation (§23), the child performs it and exits
    // without ever starting the UI. Checked first, so an elevated helper never
    // reaches the startup reporting below.
    #[cfg(windows)]
    {
        if let Some(code) = typecase_lib::fontmanager::windows_imp::run_elevated_from_args() {
            std::process::exit(code);
        }
        if let Some(code) = typecase_lib::externalfonts::run_elevated_from_args() {
            std::process::exit(code);
        }
    }

    // §35: a failed start must say something. The portable build has no
    // installer and a release build has no console, so on Windows this is a
    // native message box naming the likely cause (WebView2 runtime) with the
    // download page. See startup.rs.
    if let Err(err) = typecase_lib::run() {
        typecase_lib::startup::report_startup_failure(&err);
        std::process::exit(1);
    }
}
