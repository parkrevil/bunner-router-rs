use super::node::RadixTreeNode;

fn can_compress_here(n: &RadixTreeNode) -> bool {
    n.patterns.is_empty()
        && !n.routes.iter().any(|k| *k != 0)
        && !n.wildcard_routes.iter().any(|k| *k != 0)
        && n.static_children.len() <= 1
        && n.static_keys.len() <= 1
}

fn compress_node(n: &mut RadixTreeNode) {
    if let Some(c) = n.pattern_nodes.first_mut() {
        compress_node(c.as_mut());
    }
    if !n.static_children.is_empty() {
        for (_, v) in n.static_children.iter_mut() {
            compress_node(v.as_mut());
        }
    } else {
        for v in n.static_vals.iter_mut() {
            compress_node(v.as_mut());
        }
    }

    if n.fused_edge.is_some() {
        return;
    }
    if !can_compress_here(n) {
        return;
    }

    let (mut edge, mut child) = if !n.static_children.is_empty() {
        if n.static_children.len() != 1 {
            return;
        }
        let (k, c) = n.static_children.drain().next().unwrap();
        (k.to_string(), c)
    } else if !n.static_keys.is_empty() {
        if n.static_keys.len() != 1 {
            return;
        }
        let k = n.static_keys.pop().unwrap();
        let c = n.static_vals.pop().unwrap();
        (k.to_string(), c)
    } else {
        return;
    };

    loop {
        let terminal =
            child.routes.iter().any(|k| *k != 0) || child.wildcard_routes.iter().any(|k| *k != 0);
        if child.patterns.is_empty() && !terminal {
            if !child.static_children.is_empty() && child.static_children.len() == 1 {
                let (k2, c2) = child.static_children.drain().next().unwrap();
                edge.push('/');
                edge.push_str(&k2);
                child = c2;
                continue;
            }
            if !child.static_keys.is_empty() && child.static_keys.len() == 1 {
                let k2 = child.static_keys.pop().unwrap();
                let c2 = child.static_vals.pop().unwrap();
                edge.push('/');
                edge.push_str(&k2);
                child = c2;
                continue;
            }
        }
        break;
    }
    n.fused_edge = Some(edge);
    n.fused_child = Some(child);
}

pub(super) fn compress_root_node(root: &mut RadixTreeNode) {
    compress_node(root);
}
