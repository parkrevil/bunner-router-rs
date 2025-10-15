use crate::enums::HttpMethod;
use crate::matcher::ParamEntry;
use crate::pattern::match_segment;
use crate::readonly::ReadOnlyNode;
use crate::types::RouteMatch;
use regex::Regex;

pub fn find_route(
    root: &ReadOnlyNode,
    method: HttpMethod,
    normalized: &str,
    params: &mut Vec<ParamEntry>,
    default_param_pattern: &Regex,
) -> Option<RouteMatch> {
    find_from(root, method, normalized, 0, params, default_param_pattern)
}

fn find_from(
    node: &ReadOnlyNode,
    method: HttpMethod,
    path: &str,
    index: usize,
    params: &mut Vec<ParamEntry>,
    default_param_pattern: &Regex,
) -> Option<RouteMatch> {
    let current_index = skip_slashes(path, index);

    if let Some(edge) = node.fused_edge.as_deref() {
        let remainder = &path[current_index..];
        if !remainder.starts_with(edge) {
            return None;
        }
        if let Some(child) = node.fused_child.as_deref() {
            return find_from(
                child,
                method,
                path,
                current_index + edge.len(),
                params,
                default_param_pattern,
            );
        }
        return None;
    }

    if current_index >= path.len() {
        if path.as_bytes().last() == Some(&b'/')
            && let Some(next_node) = node.static_children.get("")
            && let Some(found) = find_from(
                next_node,
                method,
                path,
                current_index,
                params,
                default_param_pattern,
            )
        {
            return Some(found);
        }
        return handle_terminal(node, method, params);
    }

    let (segment, next_index) = split_segment(path, current_index);

    if let Some(next_node) = node.static_children.get(segment)
        && let Some(found) = find_from(
            next_node,
            method,
            path,
            next_index,
            params,
            default_param_pattern,
        )
    {
        return Some(found);
    }

    for (pattern, child) in node.patterns.iter() {
        if let Some(kvs) = match_segment(segment, pattern, default_param_pattern) {
            let checkpoint = params.len();
            for (name, (offset, len)) in kvs.into_iter() {
                let abs_offset = current_index + offset;
                if abs_offset + len <= path.len() {
                    params.push((name, (abs_offset, len)));
                }
            }
            if let Some(found) = find_from(
                child,
                method,
                path,
                next_index,
                params,
                default_param_pattern,
            ) {
                return Some(found);
            }
            params.truncate(checkpoint);
        }
    }

    if let Some(match_wildcard) = handle_wildcard(node, method, path, current_index, params) {
        return Some(match_wildcard);
    }

    None
}

fn handle_terminal(
    node: &ReadOnlyNode,
    method: HttpMethod,
    params: &mut Vec<ParamEntry>,
) -> Option<RouteMatch> {
    let method_index = method as usize;
    let rk = node.routes[method_index];
    if rk != 0 {
        return Some((rk - 1, params.to_owned()));
    }
    let wildcard = node.wildcard_routes[method_index];
    if wildcard != 0 {
        return Some((wildcard - 1, params.to_owned()));
    }
    None
}

fn handle_wildcard(
    node: &ReadOnlyNode,
    method: HttpMethod,
    path: &str,
    start_index: usize,
    params: &mut Vec<ParamEntry>,
) -> Option<RouteMatch> {
    let wildcard = node.wildcard_routes[method as usize];
    if wildcard == 0 {
        return None;
    }

    let mut capture_start = start_index;
    if capture_start < path.len() && path.as_bytes()[capture_start] == b'/' {
        capture_start += 1;
    }
    if capture_start <= path.len() {
        let rest_len = path.len().saturating_sub(capture_start);
        if rest_len > 0 {
            params.push(("*".to_string(), (capture_start, rest_len)));
        }
    }

    Some((wildcard - 1, params.clone()))
}

fn skip_slashes(s: &str, mut index: usize) -> usize {
    let bytes = s.as_bytes();
    if index < bytes.len() && bytes[index] == b'/' {
        index += 1;
    }
    index
}

fn split_segment(s: &str, start: usize) -> (&str, usize) {
    let bytes = s.as_bytes();
    let mut end = start;
    while end < bytes.len() && bytes[end] != b'/' {
        end += 1;
    }
    (&s[start..end], end)
}
