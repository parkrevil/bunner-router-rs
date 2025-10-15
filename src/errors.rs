use crate::path::PathError;
use crate::pattern::PatternError;
use crate::radix::RadixError;
use crate::readonly::ReadOnlyError;
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
    Path(#[from] PathError),
    #[error(transparent)]
    Pattern(#[from] PatternError),
    #[error(transparent)]
    Radix(#[from] RadixError),
    #[error(transparent)]
    ReadOnly(#[from] ReadOnlyError),
}

pub type RouterResult<T> = Result<T, RouterError>;
