use crate::enums::HttpMethod;
use crate::path::PathError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReadOnlyError {
    #[error(transparent)]
    Path(#[from] PathError),
    #[error("no route matched for method {method:?} and path '{path}'")]
    RouteNotFound { method: HttpMethod, path: String },
}

pub type ReadOnlyResult<T> = Result<T, ReadOnlyError>;
