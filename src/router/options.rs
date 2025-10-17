use crate::enums::HttpMethod;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

const ROUTE_PRIORITY_MIN: i32 = -100;
const ROUTE_PRIORITY_MAX: i32 = 100;
pub const DEFAULT_PARAM_PATTERN: &str = "[^/]+";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MatchOrder {
    #[default]
    SpecificFirst,
    DefinedFirst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RepeatMatchMode {
    #[default]
    Greedy,
    Lazy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ParamStyle {
    #[default]
    Colon,
    Braces,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteOptions {
    pub pattern: Option<String>,
    pub methods: Vec<HttpMethod>,
    pub constraints: HashMap<String, String>,
    pub optional: bool,
    pub repeatable: bool,
    pub priority: i32,
    pub meta: HashMap<String, String>,
    pub alias: Option<String>,
}

impl Default for RouteOptions {
    fn default() -> Self {
        Self {
            pattern: None,
            methods: vec![HttpMethod::Get],
            constraints: HashMap::new(),
            optional: false,
            repeatable: false,
            priority: 0,
            meta: HashMap::new(),
            alias: None,
        }
    }
}

impl RouteOptions {
    pub fn builder() -> RouteOptionsBuilder {
        RouteOptionsBuilder::default()
    }

    pub fn validate(&self) -> Result<(), RouterConfigError> {
        if self.methods.is_empty() {
            return Err(RouterConfigError::EmptyRouteMethods);
        }
        if !(ROUTE_PRIORITY_MIN..=ROUTE_PRIORITY_MAX).contains(&self.priority) {
            return Err(RouterConfigError::RoutePriorityOutOfRange {
                value: self.priority,
                min: ROUTE_PRIORITY_MIN,
                max: ROUTE_PRIORITY_MAX,
            });
        }
        if self
            .alias
            .as_ref()
            .is_some_and(|alias| alias.trim().is_empty())
        {
            return Err(RouterConfigError::EmptyAlias);
        }
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct RouteOptionsBuilder {
    options: RouteOptions,
}

impl RouteOptionsBuilder {
    pub fn pattern<S: Into<String>>(mut self, pattern: S) -> Self {
        self.options.pattern = Some(pattern.into());
        self
    }

    pub fn methods<I>(mut self, methods: I) -> Self
    where
        I: Into<Vec<HttpMethod>>,
    {
        self.options.methods = methods.into();
        self
    }

    pub fn constraints(mut self, constraints: HashMap<String, String>) -> Self {
        self.options.constraints = constraints;
        self
    }

    pub fn optional(mut self, value: bool) -> Self {
        self.options.optional = value;
        self
    }

    pub fn repeatable(mut self, value: bool) -> Self {
        self.options.repeatable = value;
        self
    }

    pub fn priority(mut self, value: i32) -> Self {
        self.options.priority = value;
        self
    }

    pub fn meta(mut self, meta: HashMap<String, String>) -> Self {
        self.options.meta = meta;
        self
    }

    pub fn alias<S: Into<String>>(mut self, alias: S) -> Self {
        self.options.alias = Some(alias.into());
        self
    }

    pub fn build(self) -> Result<RouteOptions, RouterConfigError> {
        self.options.validate()?;
        Ok(self.options)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouterConfig {
    pub case_sensitive: bool,
    pub strict_trailing_slash: bool,
    pub decode_uri: bool,
    pub normalize_path: bool,
    pub allow_duplicate_slash: bool,
    pub match_order: MatchOrder,
    pub repeat_match_mode: RepeatMatchMode,
    pub max_param_depth: usize,
    pub debug: bool,
    pub route_defaults: RouteOptions,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            strict_trailing_slash: false,
            decode_uri: false,
            normalize_path: true,
            allow_duplicate_slash: false,
            match_order: MatchOrder::default(),
            repeat_match_mode: RepeatMatchMode::default(),
            max_param_depth: 8,
            debug: false,
            route_defaults: RouteOptions::default(),
        }
    }
}

impl RouterConfig {
    pub fn builder() -> RouterConfigBuilder {
        RouterConfigBuilder::default()
    }

    pub fn validate(&self) -> Result<(), RouterConfigError> {
        if self.max_param_depth == 0 {
            return Err(RouterConfigError::MaxParamDepthInvalid { provided: 0 });
        }
        self.route_defaults.validate()?;
        Ok(())
    }

    pub fn param_pattern_default_regex(&self) -> Regex {
        Regex::new(&format!("^(?:{})$", DEFAULT_PARAM_PATTERN))
            .expect("default param pattern should compile")
    }
}

#[derive(Debug, Default, Clone)]
pub struct RouterConfigBuilder {
    config: RouterConfig,
}

impl RouterConfigBuilder {
    pub fn case_sensitive(mut self, value: bool) -> Self {
        self.config.case_sensitive = value;
        self
    }

    pub fn strict_trailing_slash(mut self, value: bool) -> Self {
        self.config.strict_trailing_slash = value;
        self
    }

    pub fn decode_uri(mut self, value: bool) -> Self {
        self.config.decode_uri = value;
        self
    }

    pub fn normalize_path(mut self, value: bool) -> Self {
        self.config.normalize_path = value;
        self
    }

    pub fn allow_duplicate_slash(mut self, value: bool) -> Self {
        self.config.allow_duplicate_slash = value;
        self
    }

    pub fn match_order(mut self, value: MatchOrder) -> Self {
        self.config.match_order = value;
        self
    }

    pub fn repeat_match_mode(mut self, value: RepeatMatchMode) -> Self {
        self.config.repeat_match_mode = value;
        self
    }

    pub fn max_param_depth(mut self, value: usize) -> Self {
        self.config.max_param_depth = value;
        self
    }

    pub fn debug(mut self, value: bool) -> Self {
        self.config.debug = value;
        self
    }

    pub fn route_defaults(mut self, route_defaults: RouteOptions) -> Self {
        self.config.route_defaults = route_defaults;
        self
    }

    pub fn build(self) -> Result<RouterConfig, RouterConfigError> {
        let config = self.config;
        config.validate()?;
        Ok(config)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RouterConfigError {
    #[error("max_param_depth must be at least 1 (got {provided})")]
    MaxParamDepthInvalid { provided: usize },
    #[error("route methods cannot be empty")]
    EmptyRouteMethods,
    #[error("route priority {value} is outside the supported range {min}..={max}")]
    RoutePriorityOutOfRange { value: i32, min: i32, max: i32 },
    #[error("alias must not be empty")]
    EmptyAlias,
}

pub type RouterOptions = RouterConfig;
pub type RouterOptionsBuilder = RouterConfigBuilder;
pub type RouterOptionsError = RouterConfigError;
