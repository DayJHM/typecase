/* Non-Windows stand-in (§42: no Linux/macOS font management in v1). Compiles
   everywhere so the app shell, commands and tests exist on all platforms;
   every operation returns an honest error instead of pretending. */

use super::{FontManager, InstallRecord};

pub struct UnsupportedFontManager;

impl FontManager for UnsupportedFontManager {
    fn install(&self, _record: &InstallRecord) -> Result<(), String> {
        Err("font installation is only supported on Windows 10/11".into())
    }

    fn uninstall(&self, record: &InstallRecord) -> Result<(), String> {
        Err(format!(
            "font uninstallation is only supported on Windows 10/11 ({} not uninstalled)",
            record.family
        ))
    }

    fn is_installed(&self, _record: &InstallRecord) -> bool {
        false
    }
}
