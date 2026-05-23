#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginManifest {
    pub id: String,
    pub display_name: String,
    pub enabled: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PluginRegistry {
    manifests: Vec<PluginManifest>,
    loaded: bool,
}

impl PluginRegistry {
    pub fn register(&mut self, manifest: PluginManifest) {
        self.manifests.push(manifest);
        self.loaded = true;
    }

    pub fn manifests(&self) -> &[PluginManifest] {
        &self.manifests
    }

    pub fn is_loaded(&self) -> bool {
        self.loaded
    }
}
