//! Scene edits behind the MCP tools. Each applies to a copy of the scene and
//! is committed only if the whole batch validates, so batches are atomic.

#[cfg(test)]
mod tests;

use std::collections::HashSet;

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::scene::{Layer, Scene, check_keys};

/// `{ id }` or `{ role }`. A role may match several layers.
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum Target {
    /// The layer with this id.
    Id {
        /// Layer id.
        id: String,
    },
    /// Every layer with this role.
    Role {
        /// Layer role.
        role: String,
    },
    /// A named text style; `set` creates or changes it, `delete` removes it.
    Style {
        /// Style name.
        style: String,
    },
}

impl Target {
    fn matches(&self, l: &Layer) -> bool {
        match self {
            Target::Id { id } => l.id == *id,
            Target::Role { role } => l.role.as_deref() == Some(role),
            Target::Style { .. } => false,
        }
    }
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Target::Id { id } => write!(f, "id {id}"),
            Target::Role { role } => write!(f, "role {role}"),
            Target::Style { style } => write!(f, "style {style}"),
        }
    }
}

/// One edit in a `layer_update` batch.
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Op {
    /// The layer or layers to change.
    pub target: Target,
    /// Fields to change; `null` resets a field to its default. Nested
    /// objects merge (JSON merge patch).
    #[serde(default)]
    pub set: Option<Map<String, Value>>,
    /// Remove the layer (and its children).
    #[serde(default)]
    pub delete: bool,
}

/// Adds or replaces text `styles`, then appends layers, each on top of its
/// parent frame (or the scene root when it has no `parent`). Returns the
/// ids of the added top-level layers.
pub fn add_layers(
    scene: &mut Scene,
    styles: Map<String, Value>,
    layers: Vec<Value>,
) -> Result<Vec<String>, String> {
    let mut next = scene.clone();
    for (name, style) in styles {
        let Value::Object(style) = style else {
            return Err(format!("style {name} must be an object of text fields"));
        };
        next.styles.insert(name, style);
    }
    let mut taken: HashSet<String> = HashSet::new();
    next.walk(&mut |l| {
        taken.insert(l.id.clone());
    });
    // Parse the whole batch first, so generated ids avoid the batch's own ids.
    let mut parsed = Vec::new();
    for (i, mut v) in layers.into_iter().enumerate() {
        let at = |e: String| format!("layers[{i}]: {e}");
        let parent = match v.as_object_mut().and_then(|o| o.remove("parent")) {
            None | Some(Value::Null) => None,
            Some(Value::String(p)) => Some(p),
            Some(_) => return Err(at("parent must be a frame id".into())),
        };
        let mut layer = parse_layer(&v).map_err(at)?;
        resolve_assets(&mut layer, &next.assets);
        reserve_ids(&layer, &mut taken);
        parsed.push((i, parent, layer));
    }
    let mut added = Vec::new();
    for (i, parent, mut layer) in parsed {
        let at = |e: String| format!("layers[{i}]: {e}");
        assign_ids(&mut layer, &mut taken);
        added.push(layer.id.clone());
        match parent {
            None => next.layers.push(layer),
            Some(p) => match find_frame(&mut next.layers, &p) {
                Some(children) => children.push(layer),
                None => return Err(at(format!("no frame with id {p}"))),
            },
        }
    }
    next.validate()?;
    next.version += 1;
    *scene = next;
    Ok(added)
}

/// Applies `ops` in order. Returns ids of every layer changed or deleted.
pub fn update_layers(scene: &mut Scene, ops: &[Op]) -> Result<Vec<String>, String> {
    let mut next = scene.clone();
    let mut changed = Vec::new();
    for (i, op) in ops.iter().enumerate() {
        let at = |e: String| format!("ops[{i}]: {e}");
        if op.delete == op.set.is_some() {
            return Err(at("give exactly one of set or delete".into()));
        }
        if let Some(set) = &op.set
            && let Some(k) = ["id", "type", "children"]
                .iter()
                .find(|k| set.contains_key(**k))
        {
            return Err(at(format!("can't set {k}; delete and re-add instead")));
        }
        if let Target::Style { style } = &op.target {
            restyle(&mut next, style, op).map_err(at)?;
            changed.push(style.clone());
            continue;
        }
        let before = changed.len();
        apply(&mut next.layers, op, &mut changed).map_err(at)?;
        if changed.len() == before {
            return Err(at(format!("no layer with {}", op.target)));
        }
    }
    next.validate()?;
    next.version += 1;
    *scene = next;
    Ok(changed)
}

/// Creates, changes (merge patch) or deletes the text style `name`.
fn restyle(scene: &mut Scene, name: &str, op: &Op) -> Result<(), String> {
    match &op.set {
        None => scene
            .styles
            .remove(name)
            .map(drop)
            .ok_or_else(|| format!("no style {name}")),
        Some(set) => {
            let mut v = Value::Object(scene.styles.remove(name).unwrap_or_default());
            merge_patch(&mut v, &Value::Object(set.clone()));
            if let Value::Object(style) = v {
                scene.styles.insert(name.to_owned(), style);
            }
            Ok(())
        }
    }
}

fn apply(layers: &mut Vec<Layer>, op: &Op, changed: &mut Vec<String>) -> Result<(), String> {
    let mut i = 0;
    while i < layers.len() {
        if op.target.matches(&layers[i]) {
            changed.push(layers[i].id.clone());
            if op.delete {
                layers.remove(i);
                continue;
            }
            if let Some(set) = &op.set {
                let mut v = serde_json::to_value(&layers[i]).map_err(|e| e.to_string())?;
                merge_patch(&mut v, &Value::Object(set.clone()));
                layers[i] = parse_layer(&v).map_err(|e| format!("{}: {e}", layers[i].id))?;
            }
        }
        if let Some(children) = layers[i].kind.children_mut() {
            apply(children, op, changed)?;
        }
        i += 1;
    }
    Ok(())
}

/// Parses a layer and rejects unknown keys, recursing into frame children.
fn parse_layer(v: &Value) -> Result<Layer, String> {
    let mut v = v.clone();
    normalize(&mut v);
    let layer: Layer = serde_json::from_value(v.clone()).map_err(|e| e.to_string())?;
    check_keys(&v, &layer)?;
    if let (Some(_), Some(children)) = (
        layer.kind.children(),
        v.get("children").and_then(Value::as_array),
    ) {
        for c in children {
            parse_layer(c)?;
        }
    }
    Ok(layer)
}

/// An asset reference written with its size, as `asset_add` replies
/// (`photo 864×530`), means the asset whose id is its first word.
fn resolve_assets(
    layer: &mut Layer,
    assets: &std::collections::BTreeMap<String, crate::scene::Asset>,
) {
    let fix = |id: &mut String| {
        if !assets.contains_key(id.as_str())
            && let Some(first) = id.split_whitespace().next()
            && assets.contains_key(first)
        {
            *id = first.to_owned();
        }
    };
    match &mut layer.kind {
        crate::scene::Kind::Image { asset, .. } => fix(asset),
        crate::scene::Kind::Text { fill: Some(f), .. } => fix(&mut f.asset),
        _ => {}
    }
    if let Some(crate::scene::OneOrMany::One(crate::scene::Paint::Image(i))) = &mut layer.look.fills
    {
        fix(&mut i.image);
    }
    if let Some(crate::scene::OneOrMany::Many(fs)) = &mut layer.look.fills {
        for f in fs {
            if let crate::scene::Paint::Image(i) = f {
                fix(&mut i.image);
            }
        }
    }
    for c in layer.kind.children_mut().into_iter().flatten() {
        resolve_assets(c, assets);
    }
}

/// Accepts the names agents reach for first: on shapes, `fill` and
/// `shadow` mean `fills` and `shadows` (text keeps its own `fill`, an image
/// in the letters, and `shadow`). A rejected guess costs the agent a
/// resend of its whole batch, so these are cheaper to accept than to refuse.
pub(crate) fn normalize(v: &mut Value) {
    let Some(o) = v.as_object_mut() else { return };
    // A generic "shape" is a path when it names one, else a rect.
    if o.get("type").and_then(Value::as_str) == Some("shape") {
        let kind = if o.contains_key("d") || o.contains_key("shape") {
            "path"
        } else {
            "rect"
        };
        o.insert("type".into(), kind.into());
    }
    if o.get("type").and_then(Value::as_str) != Some("text") {
        for (short, full) in [("fill", "fills"), ("shadow", "shadows")] {
            if !o.contains_key(full)
                && let Some(x) = o.remove(short)
            {
                o.insert(full.into(), x);
            }
        }
    }
    if let Some(Value::Array(children)) = o.get_mut("children") {
        children.iter_mut().for_each(normalize);
    }
}

/// Marks the ids a layer and its children were given explicitly as taken.
fn reserve_ids(layer: &Layer, taken: &mut HashSet<String>) {
    if !layer.id.is_empty() {
        taken.insert(layer.id.clone());
    }
    for c in layer.kind.children().into_iter().flatten() {
        reserve_ids(c, taken);
    }
}

/// Gives every layer without an id one like `text3`, unique in the scene.
fn assign_ids(layer: &mut Layer, taken: &mut HashSet<String>) {
    if layer.id.is_empty() {
        let kind = layer.kind.name();
        let n = (1..)
            .find(|n| !taken.contains(&format!("{kind}{n}")))
            .unwrap_or(0);
        layer.id = format!("{kind}{n}");
    }
    taken.insert(layer.id.clone());
    for c in layer.kind.children_mut().into_iter().flatten() {
        assign_ids(c, taken);
    }
}

fn find_frame<'a>(layers: &'a mut [Layer], id: &str) -> Option<&'a mut Vec<Layer>> {
    for l in layers {
        let is_it = l.id == id;
        if let Some(children) = l.kind.children_mut() {
            if is_it {
                return Some(children);
            }
            if let Some(found) = find_frame(children, id) {
                return Some(found);
            }
        }
    }
    None
}

/// RFC 7386 JSON merge patch.
pub(crate) fn merge_patch(target: &mut Value, patch: &Value) {
    let Value::Object(p) = patch else {
        *target = patch.clone();
        return;
    };
    if !target.is_object() {
        *target = Value::Object(Map::new());
    }
    if let Value::Object(t) = target {
        for (k, v) in p {
            if v.is_null() {
                t.remove(k);
            } else {
                merge_patch(t.entry(k.clone()).or_insert(Value::Null), v);
            }
        }
    }
}
