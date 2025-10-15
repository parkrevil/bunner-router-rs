use crate::radix::RadixError;
use crate::types::HttpMethod;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RouterError {
    #[error("router is sealed; cannot add route '{path}'")]
    AddWhileSealed { path: String },
    #[error("router is sealed; cannot add {count} routes in bulk")]
    BulkAddWhileSealed { count: usize },
    #[error("router is not sealed; cannot perform route lookup")]
    FindWhileMutable,
    #[error("router is not sealed; readonly snapshot is unavailable")]
    ReadOnlyUnavailable,
    #[error(transparent)]
    Radix(#[from] RadixError),
    #[error("no route matched for method {method:?} and path '{path}'")]
    RouteNotFound { method: HttpMethod, path: String },
}

pub type RouterResult<T> = Result<T, RouterError>;
