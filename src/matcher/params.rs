use crate::types::{CapturedParam, RouteParams};
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static PARAM_BUF: RefCell<Vec<CapturedParam>> = RefCell::new(Vec::with_capacity(4));
}

pub fn with_param_buffer<R>(f: impl FnOnce(&mut Vec<CapturedParam>) -> R) -> R {
    PARAM_BUF.with(|cell| {
        let mut buf = cell.borrow_mut();
        buf.clear();
        f(&mut buf)
    })
}

pub(crate) fn captures_to_map(path: &str, captures: Vec<CapturedParam>) -> RouteParams {
    let mut map = HashMap::with_capacity(captures.len());
    for (name, (start, len)) in captures {
        let end = start.saturating_add(len);
        if start <= path.len() && end <= path.len() {
            map.insert(name, path[start..end].to_string());
        }
    }
    map
}
