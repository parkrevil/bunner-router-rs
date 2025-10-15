use crate::types::HttpMethod;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReadOnlyError {
    #[error("no route matched for method {method:?} and path '{path}'")]
    RouteNotFound { method: HttpMethod, path: String },
}
