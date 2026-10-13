use eframe::Storage;
use egui_dock::DockState;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use crate::app::state::AppState;
use crate::features::ktp::KtpPrefs;
use crate::features::qris::QrisPrefs;
use crate::features::settings::SettingsPrefs;
use crate::ui::dock::{self, Tab};

// Separate keys, so a section that stops parsing resets alone instead of wiping the rest.
trait Section: Serialize + DeserializeOwned + Default {
    const KEY: &'static str;
}

#[derive(Serialize, Deserialize, Default)]
struct DockLayout(Option<DockState<Tab>>);

impl Section for DockLayout {
    const KEY: &'static str = "dock";
}

impl Section for SettingsPrefs {
    const KEY: &'static str = "settings";
}

impl Section for QrisPrefs {
    const KEY: &'static str = "qris";
}

impl Section for KtpPrefs {
    const KEY: &'static str = "ktp";
}

fn load<T: Section>(storage: Option<&dyn Storage>) -> T {
    storage.and_then(|s| eframe::get_value(s, T::KEY)).unwrap_or_default()
}

fn save<T: Section>(storage: &mut dyn Storage, value: &T) {
    eframe::set_value(storage, T::KEY, value);
}

pub fn restore(storage: Option<&dyn Storage>, state: &mut AppState) -> DockState<Tab> {
    state.settings = load(storage);
    state.qris.prefs = load(storage);
    state.ktp.prefs = load(storage);
    load::<DockLayout>(storage).0.unwrap_or_else(dock::initial_layout)
}

pub fn persist(storage: &mut dyn Storage, dock: &DockState<Tab>, state: &AppState) {
    save(storage, &DockLayout(Some(dock.clone())));
    save(storage, &state.settings);
    save(storage, &state.qris.prefs);
    save(storage, &state.ktp.prefs);
}
