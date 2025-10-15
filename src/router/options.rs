use crate::enums::HttpMethod;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

const ROUTE_PRIORITY_MIN: i32 = -100;
const ROUTE_PRIORITY_MAX: i32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MatchOrder {
    #[default]
    SpecificFirst,
    DefinedFirst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ParamStyle {
    #[default]
    Colon,
    Braces,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParserOptions {
    pub allow_regex_in_param: bool,
    pub allow_nested_optional: bool,
    pub allow_repeat_in_optional: bool,
    pub param_style: ParamStyle,
    pub escape_chars: Vec<char>,
    pub validate_regex_syntax: bool,
}

impl Default for ParserOptions {
    fn default() -> Self {
        Self {
            allow_regex_in_param: false,
            allow_nested_optional: false,
            allow_repeat_in_optional: false,
            param_style: ParamStyle::Colon,
            escape_chars: vec!['\\'],
            validate_regex_syntax: true,
        }
    }
}

impl ParserOptions {
    pub fn builder() -> ParserOptionsBuilder {
        ParserOptionsBuilder::default()
    }

    pub fn validate(&self) -> Result<(), RouterConfigError> {
        let mut seen = HashSet::new();
        for ch in &self.escape_chars {
            if !seen.insert(*ch) {
                return Err(RouterConfigError::DuplicateEscapeChar { ch: *ch });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct ParserOptionsBuilder {
    options: ParserOptions,
}

impl ParserOptionsBuilder {
    pub fn allow_regex_in_param(mut self, value: bool) -> Self {
        self.options.allow_regex_in_param = value;
        self
    }

    pub fn allow_nested_optional(mut self, value: bool) -> Self {
        self.options.allow_nested_optional = value;
        self
    }

    pub fn allow_repeat_in_optional(mut self, value: bool) -> Self {
        self.options.allow_repeat_in_optional = value;
        self
    }

    pub fn param_style(mut self, value: ParamStyle) -> Self {
        self.options.param_style = value;
        self
    }

    pub fn escape_chars<I>(mut self, value: I) -> Self
    where
        I: Into<Vec<char>>,
    {
        self.options.escape_chars = value.into();
        self
    }

    pub fn validate_regex_syntax(mut self, value: bool) -> Self {
        self.options.validate_regex_syntax = value;
        self
    }

    pub fn build(self) -> Result<ParserOptions, RouterConfigError> {
        self.options.validate()?;
        Ok(self.options)
    }
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
pub struct RouterTuning {
    pub enable_root_level_pruning: bool,
    pub enable_static_route_full_mapping: bool,
    pub enable_automatic_optimization: bool,
}

impl Default for RouterTuning {
    fn default() -> Self {
        Self {
            enable_root_level_pruning: false,
            enable_static_route_full_mapping: false,
            enable_automatic_optimization: true,
        }
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
    pub param_pattern_default: String,
    pub max_param_depth: usize,
    pub cache_routes: bool,
    pub debug: bool,
    pub route_defaults: RouteOptions,
    pub parser: ParserOptions,
    pub tuning: RouterTuning,
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
            param_pattern_default: String::from("[^/]+"),
            max_param_depth: 8,
            cache_routes: true,
            debug: false,
            route_defaults: RouteOptions::default(),
            parser: ParserOptions::default(),
            tuning: RouterTuning::default(),
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
        if self.param_pattern_default.trim().is_empty() {
            return Err(RouterConfigError::EmptyParamPatternDefault);
        }
        self.parser.validate()?;
        self.route_defaults.validate()?;
        Ok(())
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

    pub fn param_pattern_default<S: Into<String>>(mut self, value: S) -> Self {
        self.config.param_pattern_default = value.into();
        self
    }

    pub fn max_param_depth(mut self, value: usize) -> Self {
        self.config.max_param_depth = value;
        self
    }

    pub fn cache_routes(mut self, value: bool) -> Self {
        self.config.cache_routes = value;
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

    pub fn parser(mut self, parser: ParserOptions) -> Self {
        self.config.parser = parser;
        self
    }

    pub fn tuning(mut self, tuning: RouterTuning) -> Self {
        self.config.tuning = tuning;
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
    #[error("param_pattern_default cannot be empty")]
    EmptyParamPatternDefault,
    #[error("route methods cannot be empty")]
    EmptyRouteMethods,
    #[error("route priority {value} is outside the supported range {min}..={max}")]
    RoutePriorityOutOfRange { value: i32, min: i32, max: i32 },
    #[error("alias must not be empty")]
    EmptyAlias,
    #[error("duplicate escape character '{ch}' in parser options")]
    DuplicateEscapeChar { ch: char },
}

pub type RouterOptions = RouterConfig;
pub type RouterOptionsBuilder = RouterConfigBuilder;
pub type RouterOptionsError = RouterConfigError;
