//! Selecting a value with a singular JSONPath (`$.body.items[0]["a.b"]`).

use model::{parse_json_path, JsonPathError, PathSeg};
use serde_json::Value;

/// `Ok(None)` when the path is valid but selects nothing. A key on a non-object or an index
/// on a non-array selects nothing.
pub fn select<'v>(value: &'v Value, path: &str) -> Result<Option<&'v Value>, JsonPathError> {
    let segments = parse_json_path(path)?;
    Ok(select_segments(value, &segments))
}

pub fn select_segments<'v>(value: &'v Value, segments: &[PathSeg]) -> Option<&'v Value> {
    segments.iter().try_fold(value, |current, seg| match seg {
        PathSeg::Key(k) => current.as_object()?.get(k),
        PathSeg::Index(i) => current.as_array()?.get(*i),
    })
}
