use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelDomain {
    Ocr,
    Translation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelSource {
    BuiltIn,
    LocalPath(String),
    Download { url: String, sha256: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
        let source = source.and_then(normalized_optional_language);
        let source_ok = match source {
            Some(language) => {
                self.source_languages.is_empty()
                    || self.source_languages.iter().any(|item| item == language)
            }
            None => source_language_count(&self.source_languages) <= 1,
        };

        let target_ok = self.target_languages.is_empty()
            || self.target_languages.iter().any(|item| item == target);

        source_ok && target_ok
    }
}

fn normalized_optional_language(language: &str) -> Option<&str> {
    let language = language.trim();
    if language.is_empty() || language.eq_ignore_ascii_case("auto") {
        None
    } else {
        Some(language)
    }
}

fn source_language_count(languages: &[String]) -> usize {
    languages
        .iter()
        .map(|language| language.trim())
        .filter(|language| !language.is_empty())
        .take(2)
        .count()
}

#[cfg(test)]
mod tests {
    use super::{ModelDomain, ModelFile, ModelManifest, ModelSource};

    fn translation_model(source_languages: Vec<&str>) -> ModelManifest {
        ModelManifest {
            id: "test-translation".to_owned(),
            name: "Test Translation".to_owned(),
            domain: ModelDomain::Translation,
            family: "test".to_owned(),
            backend: "ctranslate2".to_owned(),
            version: "test".to_owned(),
            source_languages: source_languages
                .into_iter()
                .map(|language| language.to_owned())
                .collect(),
            target_languages: vec!["zh-CN".to_owned()],
            quantization: None,
            low_spec_friendly: true,
            multilingual: false,
            source: ModelSource::BuiltIn,
            files: vec![ModelFile::required("model", "model.bin")],
        }
    }

    #[test]
    fn auto_source_matches_single_source_model() {
        let model = translation_model(vec!["en"]);

        assert!(model.supports_language_pair(Some("auto"), "zh-CN"));
        assert!(model.supports_language_pair(None, "zh-CN"));
    }

    #[test]
    fn auto_source_does_not_match_multi_source_model() {
        let model = translation_model(vec!["en", "fr"]);

        assert!(!model.supports_language_pair(Some("auto"), "zh-CN"));
        assert!(!model.supports_language_pair(None, "zh-CN"));
        assert!(model.supports_language_pair(Some("en"), "zh-CN"));
    }
}
