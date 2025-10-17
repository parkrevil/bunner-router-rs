use crate::path::{NormalizationOptions, PathResult, normalize_path};
use crate::router::RouterOptions;

#[derive(Debug, Clone)]
pub struct PreprocessOutcome {
    original: String,
    normalized: String,
    cache_key: String,
}

impl PreprocessOutcome {
    pub fn original(&self) -> &str {
        &self.original
    }

    pub fn normalized(&self) -> &str {
        &self.normalized
    }

    pub fn cache_key(&self) -> &str {
        &self.cache_key
    }
}

#[derive(Debug, Clone, Default)]
pub struct Preprocessor {
    config: RouterOptions,
}

impl Preprocessor {
    pub fn new(config: RouterOptions) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &RouterOptions {
        &self.config
    }

    pub fn update_config(&mut self, config: RouterOptions) {
        self.config = config;
    }

    pub fn apply(&self, path: &str) -> PathResult<PreprocessOutcome> {
        apply(path, &self.config)
    }
}

pub fn apply(path: &str, config: &RouterOptions) -> PathResult<PreprocessOutcome> {
    let options = NormalizationOptions {
        decode_percent: config.decode_uri,
        normalize_path: config.normalize_path,
        allow_duplicate_slash: config.allow_duplicate_slash,
        strict_trailing_slash: config.strict_trailing_slash,
        case_sensitive: config.case_sensitive,
    };

    let normalized = normalize_path(path, &options)?;
    let cache_key = normalized.clone();

    Ok(PreprocessOutcome {
        original: path.to_string(),
        normalized,
        cache_key,
    })
}
