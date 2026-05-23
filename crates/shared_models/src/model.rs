#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelDomain {
    Ocr,
    Translation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelSource {
    BuiltIn,
    LocalPath(String),
    Download { url: String, sha256: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelFile {
    pub role: String,
    pub path: String,
    pub required: bool,
}

impl ModelFile {
    pub fn required(role: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            path: path.into(),
            required: true,
        }
    }

    pub fn optional(role: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            path: path.into(),
            required: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelManifest {
    pub id: String,
    pub name: String,
    pub domain: ModelDomain,
    pub family: String,
    pub backend: String,
    pub version: String,
    pub source_languages: Vec<String>,
    pub target_languages: Vec<String>,
    pub quantization: Option<String>,
    pub low_spec_friendly: bool,
    pub multilingual: bool,
    pub source: ModelSource,
    pub files: Vec<ModelFile>,
}

impl ModelManifest {
    pub fn supports_language_pair(&self, source: Option<&str>, target: &str) -> bool {
        let source_ok = source.is_none_or(|language| {
            self.source_languages.is_empty()
                || self.source_languages.iter().any(|item| item == language)
        });

        let target_ok = self.target_languages.is_empty()
            || self.target_languages.iter().any(|item| item == target);

        source_ok && target_ok
    }
}
