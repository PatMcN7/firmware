use crate::ui;
use std::{fs, path};

pub const SETTINGS_PATH: &str = "settings.json";
pub const DEFAULT_LOG_FOLDER: &str = "logs";

pub fn database_dir() -> Option<path::PathBuf> {
    let path = path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../superdbc");
    path.is_dir().then_some(path)
}

const DEFAULT_UDP_PORT: u16 = 5005;
const DEFAULT_CAN_SPEED: daqcore::connection::CanBusSpeed =
    daqcore::connection::CanBusSpeed::Kbps500;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Settings {
    pub superdbc_path: Option<path::PathBuf>,
    pub bus_selection: Option<daqcore::can_thread::BusSelection>,
    pub selected_source: Option<daqcore::connection::ConnectionSource>,
    pub selected_speed: daqcore::connection::CanBusSpeed,
    pub udp_port: u16,
    #[serde(default = "default_beamng_port")]
    pub beamng_port: u16,
    pub theme: ui::theme::ThemeSelection,
    pub pixels_per_point: Option<f32>,
    pub log_folder: Option<path::PathBuf>,
    pub window_secs: f64,
}

fn default_beamng_port() -> u16 {
    daqcore::beamng::DEFAULT_PORT
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            superdbc_path: None,
            bus_selection: None,
            selected_source: None,
            selected_speed: DEFAULT_CAN_SPEED,
            udp_port: DEFAULT_UDP_PORT,
            beamng_port: default_beamng_port(),
            theme: ui::theme::ThemeSelection::Default,
            pixels_per_point: None,
            log_folder: None,
            window_secs: 30.0,
        }
    }
}

impl Settings {
    fn path() -> path::PathBuf {
        path::PathBuf::from(SETTINGS_PATH)
    }

    pub fn load() -> Self {
        let path = Self::path();
        if let Ok(json) = fs::read_to_string(&path) {
            serde_json::from_str(&json).unwrap_or_default()
        } else {
            let default = Settings::default();
            default.save();
            default
        }
    }

    pub fn save(&self) {
        let json = match serde_json::to_string_pretty(self) {
            Ok(json) => json,
            Err(error) => {
                log::error!("Failed to serialize settings: {error}");
                return;
            }
        };

        let path = Self::path();
        if let Err(error) = fs::write(&path, json) {
            log::error!("Failed to write {}: {error}", path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_settings_keep_their_values_and_default_the_beamng_port() {
        let mut value = serde_json::to_value(Settings::default()).unwrap();
        value.as_object_mut().unwrap().remove("beamng_port");
        value["udp_port"] = 5010.into();
        let settings: Settings = serde_json::from_value(value).unwrap();
        assert_eq!(settings.udp_port, 5010);
        assert_eq!(settings.beamng_port, daqcore::beamng::DEFAULT_PORT);
    }

    #[test]
    fn beamng_source_and_custom_port_persist() {
        let settings = Settings {
            selected_source: Some(daqcore::connection::ConnectionSource::BeamNg(4446)),
            beamng_port: 4446,
            ..Settings::default()
        };
        let restored: Settings =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert_eq!(restored.selected_source, settings.selected_source);
        assert_eq!(restored.beamng_port, 4446);
    }
}
