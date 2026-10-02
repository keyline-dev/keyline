//! `layer_update` edits to the layer tree itself, which agents reach for by
//! name: `set {children}` replaces a frame's children, `set {parent}` moves
//! a layer into another frame, `set {index}` moves it within its parent.

use std::collections::HashSet;

use serde_json::Value;

use super::{
    assign_ids, blame_token, culprit, find_frame, parse_layer, reserve_ids, resolve_assets,
};
use crate::scene::{Layer, Scene};

/// Replaces frame `id`'s children with `kids`, a list of layers (or `null`,
/// none), parsed and given ids as `layer_add` does.
pub(super) fn set_children(scene: &mut Scene, id: &str, kids: Value) -> Result<(), String> {
    let list = match kids {
        Value::Null => Vec::new(),
        Value::Array(list) => list,
        _ => return Err("children takes a list of layers, or null for none".into()),
    };
    // Ids the new children may not reuse: every layer but the ones replaced.
    let mut taken = HashSet::new();
    outside(&scene.layers, id, &mut taken);
    let mut parsed = Vec::with_capacity(list.len());
    for (i, mut v) in list.into_iter().enumerate() {
        let at = |e: String| format!("children[{i}]: {e}");
        super::guesses::dollar::whole(&mut v, &scene.tokens);
        crate::reuse::tokens::bind(&mut v, &scene.tokens).map_err(at)?;
        let mut layer = parse_layer(&v).map_err(|e| match culprit(&v) {
            Some((path, c, e)) => at(format!("{path}: {}", blame_token(c, e))),
            None => at(blame_token(&v, e)),
        })?;
        resolve_assets(&mut layer, &scene.assets);
        reserve_ids(&layer, &mut taken);
        parsed.push(layer);
    }
    for layer in &mut parsed {
        assign_ids(layer, &mut taken);
    }
    let children =
        find_frame(&mut scene.layers, id).ok_or_else(|| format!("no frame with id {id}"))?;
    *children = parsed;
    Ok(())
}

/// The ids of every layer except frame `id`'s descendants.
fn outside(layers: &[Layer], id: &str, ids: &mut HashSet<String>) {
    for l in layers {
        ids.insert(l.id.clone());
        if l.id != id
            && let Some(children) = l.kind.children()
        {
            outside(children, id, ids);
        }
    }
}

/// Moves layer `id` into frame `parent` (the scene's root for `null`;
/// its own parent when not given), at `index` (on top when not given).
pub(super) fn move_layer(
    scene: &mut Scene,
    id: &str,
    parent: Option<Value>,
    index: Option<Value>,
) -> Result<(), String> {
    let index = match index {
        None | Some(Value::Null) => None,
        Some(v) => Some(
            v.as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or("index is a position from 0, bottom to top")?,
        ),
    };
    let to = match parent {
        None => Where::Same,
        Some(Value::Null) => Where::Root,
        Some(Value::String(p)) if p == id => return Err("a layer can't hold itself".into()),
        Some(Value::String(p)) => Where::Frame(p),
        Some(_) => return Err("parent is a frame id, or null for the scene".into()),
    };
    let from = holder(&scene.layers, id, None).ok_or_else(|| format!("no layer with id {id}"))?;
    let layer = take(&mut scene.layers, id).ok_or_else(|| format!("no layer with id {id}"))?;
    let dest = match to {
        Where::Root => None,
        Where::Same => from,
        Where::Frame(p) => Some(p),
    };
    let list = match &dest {
        None => &mut scene.layers,
        // Gone from the tree with the layer: a frame inside it.
        Some(p) => find_frame(&mut scene.layers, p)
            .ok_or_else(|| format!("no frame with id {p} outside {id}"))?,
    };
    let at = index.map_or(list.len(), |i| i.min(list.len()));
    list.insert(at, layer);
    Ok(())
}

/// Where a moved layer goes.
enum Where {
    /// The scene's top level.
    Root,
    /// The frame it's in now.
    Same,
    /// That frame.
    Frame(String),
}

/// The id of the frame holding layer `id` (`Some(None)` at the top level).
fn holder(layers: &[Layer], id: &str, parent: Option<&str>) -> Option<Option<String>> {
    for l in layers {
        if l.id == id {
            return Some(parent.map(str::to_owned));
        }
        if let Some(children) = l.kind.children()
            && let Some(found) = holder(children, id, Some(&l.id))
        {
            return Some(found);
        }
    }
    None
}

/// Removes layer `id` from the tree and returns it.
fn take(layers: &mut Vec<Layer>, id: &str) -> Option<Layer> {
    if let Some(i) = layers.iter().position(|l| l.id == id) {
        return Some(layers.remove(i));
    }
    layers
        .iter_mut()
        .filter_map(|l| l.kind.children_mut())
        .find_map(|children| take(children, id))
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use crate::ops::{Op, Shared, update_layers};
    use crate::scene::Scene;

    fn scene() -> Scene {
        serde_json::from_value(json!({
            "width": 100, "height": 100, "sizes": [{"id": "a", "width": 100, "height": 100}],
            "layers": [
                {"id": "bg", "type": "frame", "width": "fill", "height": "fill", "children": [
                    {"id": "a1", "type": "rect"}, {"id": "a2", "type": "rect"}]},
                {"id": "glow", "type": "rect"},
                {"id": "col", "type": "frame", "flexDirection": "column", "children": [
                    {"id": "old", "type": "text", "text": "Old"}]}
            ]
        }))
        .unwrap()
    }

    fn update(s: &mut Scene, ops: Value) -> Result<Vec<String>, String> {
        let ops: Vec<Op> = serde_json::from_value(ops).unwrap();
        update_layers(s, Shared::default(), &ops)
    }

    /// Ids in drawing order, bottom to top, `parent: child child`.
    fn tree(s: &Scene) -> String {
        s.layers
            .iter()
            .map(|l| match l.kind.children() {
                Some(c) => format!(
                    "{}[{}]",
                    l.id,
                    c.iter()
                        .map(|c| c.id.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                ),
                None => l.id.clone(),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn set_children_replaces_or_empties_a_frame() {
        let mut s = scene();
        let ids = update(&mut s, json!([{"target": {"id": "col"}, "set": {"children": [
            {"id": "title", "type": "text", "text": "New"}, {"type": "text", "text": "Untitled"}]}}]))
        .unwrap();
        assert_eq!(ids, ["col"]);
        assert_eq!(tree(&s), "bg[a1 a2] glow col[title text1]");
        // A replaced child's id is free again; another layer's isn't.
        update(&mut s, json!([{"target": {"id": "col"}, "set": {"children": [{"id": "title", "type": "rect"}]}}])).unwrap();
        let e = update(&mut s, json!([{"target": {"id": "col"}, "set": {"children": [{"id": "glow", "type": "rect"}]}}]))
            .unwrap_err();
        assert!(e.contains("glow"), "{e}");
        update(
            &mut s,
            json!([{"target": {"id": "col"}, "set": {"children": null, "gap": 8}}]),
        )
        .unwrap();
        assert_eq!(tree(&s), "bg[a1 a2] glow col[]");
        let e = update(
            &mut s,
            json!([{"target": {"id": "col"}, "set": {"children": [{"type": "rect", "nope": 1}]}}]),
        )
        .unwrap_err();
        assert!(e.starts_with("ops[0]: children[0]: "), "{e}");
    }

    #[test]
    fn parent_and_index_move_a_layer() {
        let mut s = scene();
        // Into another frame, on top; then to the bottom of it.
        update(
            &mut s,
            json!([{"target": {"id": "glow"}, "set": {"parent": "bg"}}]),
        )
        .unwrap();
        assert_eq!(tree(&s), "bg[a1 a2 glow] col[old]");
        update(
            &mut s,
            json!([{"target": {"id": "glow"}, "set": {"index": 0, "opacity": 0.5}}]),
        )
        .unwrap();
        assert_eq!(tree(&s), "bg[glow a1 a2] col[old]");
        // Back to the scene's top level, at a place.
        update(
            &mut s,
            json!([{"target": {"id": "glow"}, "set": {"parent": null, "index": 1}}]),
        )
        .unwrap();
        assert_eq!(tree(&s), "bg[a1 a2] glow col[old]");
        // Not into itself, nor a frame that isn't there.
        assert!(
            update(
                &mut s,
                json!([{"target": {"id": "bg"}, "set": {"parent": "bg"}}])
            )
            .is_err()
        );
        assert!(
            update(
                &mut s,
                json!([{"target": {"id": "glow"}, "set": {"parent": "nope"}}])
            )
            .is_err()
        );
        assert!(
            update(
                &mut s,
                json!([{"target": {"id": "glow"}, "set": {"index": -1}}])
            )
            .is_err()
        );
    }
}
