mod error;
mod normalize;

pub use error::{PathError, PathResult};
pub use normalize::{NormalizationOptions, normalize_and_validate_path, normalize_path};
