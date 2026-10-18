/*!
 * SOURCE OF TRUTH KEYWORDS: AppProfile, bundle_id, overrides, per_app_profile,
 *   excluded
 * WHAT:  One application's sparse setting overrides.
 * WHY:   Sparse on purpose: a profile names only what it CHANGES, so everything
 *        it does not mention keeps following the global setting. A profile that
 *        carried every setting would silently freeze the rest at whatever they
 *        were the day it was created. Lives in types/ (not beside the service
 *        that stores it) because the backup file also carries it, and a type
 *        two layers need must not be owned by one of them.
 * WHERE: Stored by services/profiles.rs, applied by session/settings_view.rs,
 *        edited in Settings and carried by ipc/commands/backup.rs.
 */

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use specta::Type;

use super::settings::SettingValue;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AppProfile {
    /// e.g. "com.apple.Terminal". Matched against the frontmost app.
    pub bundle_id: String,
    pub display_name: String,
    /// Sparse: only the settings this profile changes.
    pub overrides: HashMap<String, SettingValue>,
    pub enabled: bool,
    /// Vocs does not record while this app is in front. `default` so a backup
    /// made before this existed still restores.
    #[serde(default)]
    pub excluded: bool,
}
