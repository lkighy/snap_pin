use shared_models::Settings;

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigStore {
    settings: Settings,
    loaded: bool,
}

impl ConfigStore {
    pub fn from_settings(settings: Settings) -> Self {
        Self {
            settings,
            loaded: true,
        }
    }

    pub fn replace(&mut self, settings: Settings) {
        self.settings = settings;
        self.loaded = true;
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn is_loaded(&self) -> bool {
        self.loaded
    }
}
