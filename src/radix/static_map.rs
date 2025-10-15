use super::{HTTP_METHOD_COUNT, node::RadixTreeNode};
use hashbrown::HashMap as FastHashMap;

pub(super) fn collect_static(
    n: &RadixTreeNode,
    buf: &mut String,
    maps: &mut [FastHashMap<Box<str>, u16>; HTTP_METHOD_COUNT],
) {
    let base_len = buf.len();
    if let Some(edge) = n.fused_edge.as_ref() {
        if buf.is_empty() {
            buf.push('/');
        }
        buf.push_str(edge.as_ref());
    }
    for (i, &rk) in n.routes.iter().enumerate() {
        if rk != 0 {
            let key = if buf.is_empty() {
                Box::<str>::from("/")
            } else {
                buf.to_owned().into_boxed_str()
            };
            maps[i].insert(key, rk);
        }
    }
    if !n.static_keys.is_empty() && n.static_vals_idx.len() == n.static_keys.len() {
        for (k_idx, nb) in n.static_keys.iter().zip(n.static_vals_idx.iter()) {
            let prev = buf.len();
            buf.push('/');
            buf.push_str(k_idx.as_ref());
            collect_static(nb.as_ref(), buf, maps);
            buf.truncate(prev);
        }
    }
    for (k, v) in n.static_children.iter() {
        let prev = buf.len();
        buf.push('/');
        buf.push_str(k.as_ref());
        collect_static(v.as_ref(), buf, maps);
        buf.truncate(prev);
    }
    if let Some(fc) = n.fused_child.as_ref() {
        collect_static(fc.as_ref(), buf, maps);
    }
    buf.truncate(base_len);
}
