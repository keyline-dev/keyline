//! Scene edits behind the MCP tools. Each applies to a copy of the scene and
//! is committed only if the whole batch validates, so batches are atomic.

use std::collections::HashSet;

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::scene::{Kind, Layer, Scene, check_keys};

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
        let layer = parse_layer(&v).map_err(at)?;
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
        if let Kind::Frame { children, .. } = &mut layers[i].kind {
            apply(children, op, changed)?;
        }
        i += 1;
    }
    Ok(())
}

/// Parses a layer and rejects unknown keys, recursing into frame children.
fn parse_layer(v: &Value) -> Result<Layer, String> {
    let layer: Layer = serde_json::from_value(v.clone()).map_err(|e| e.to_string())?;
    check_keys(v, &layer)?;
    if let (Kind::Frame { .. }, Some(children)) =
        (&layer.kind, v.get("children").and_then(Value::as_array))
    {
        for c in children {
            parse_layer(c)?;
        }
    }
    Ok(layer)
}

/// Marks the ids a layer and its children were given explicitly as taken.
fn reserve_ids(layer: &Layer, taken: &mut HashSet<String>) {
    if !layer.id.is_empty() {
        taken.insert(layer.id.clone());
    }
    if let Kind::Frame { children, .. } = &layer.kind {
        for c in children {
            reserve_ids(c, taken);
        }
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
    if let Kind::Frame { children, .. } = &mut layer.kind {
        for c in children {
            assign_ids(c, taken);
        }
    }
}

fn find_frame<'a>(layers: &'a mut [Layer], id: &str) -> Option<&'a mut Vec<Layer>> {
    for l in layers {
        let is_it = l.id == id;
        if let Kind::Frame { children, .. } = &mut l.kind {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn scene() -> Scene {
        serde_json::from_value(json!({
            "width": 100, "height": 100, "sizes": [{"id": "a", "width": 100, "height": 100}],
            "layers": [
                {"id": "bar", "type": "frame", "width": 100, "height": 20, "children": [
                    {"id": "label", "role": "cta", "type": "text", "text": "Go"}
                ]},
                {"id": "t2", "role": "cta", "type": "text", "text": "Now"}
            ]
        }))
        .unwrap()
    }

    fn op(v: Value) -> Op {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn a_style_target_creates_changes_and_deletes_styles() {
        let mut s = scene();
        let op = |v: Value| serde_json::from_value::<Op>(v).unwrap();
        let ids = update_layers(
            &mut s,
            &[op(
                json!({"target": {"style": "h"}, "set": {"fontSize": 40, "weight": 800}}),
            )],
        )
        .unwrap();
        assert_eq!(ids, ["h"]);
        update_layers(
            &mut s,
            &[op(
                json!({"target": {"style": "h"}, "set": {"weight": null}}),
            )],
        )
        .unwrap();
        assert_eq!(s.styles["h"], *json!({"fontSize": 40}).as_object().unwrap());
        update_layers(
            &mut s,
            &[op(json!({"target": {"style": "h"}, "delete": true}))],
        )
        .unwrap();
        assert!(s.styles.is_empty());
        let err = update_layers(
            &mut s,
            &[op(json!({"target": {"style": "h"}, "delete": true}))],
        );
        assert!(err.unwrap_err().contains("no style h"));
    }

    #[test]
    fn add_generates_ids_and_nests_under_parent() {
        let mut s = scene();
        let ids = add_layers(
            &mut s,
            Map::new(),
            vec![
                json!({"type": "rect", "color": "#FF0000"}),
                json!({"type": "rect", "parent": "bar"}),
                json!({"type": "frame", "children": [{"type": "text", "text": "x"}]}),
            ],
        )
        .unwrap();
        assert_eq!(ids, ["rect1", "rect2", "frame1"]);
        assert_eq!(s.version, 1);
        let Kind::Frame { children, .. } = &s.layers[0].kind else {
            panic!()
        };
        assert_eq!(children[1].id, "rect2");
        let Kind::Frame { children, .. } = &s.layers[3].kind else {
            panic!()
        };
        assert_eq!(children[0].id, "text1");
    }

    #[test]
    fn generated_ids_avoid_ids_given_later_in_the_batch() {
        let mut s = scene();
        let ids = add_layers(
            &mut s,
            Map::new(),
            vec![
                json!({"type": "rect"}),
                json!({"id": "rect1", "type": "rect"}),
            ],
        )
        .unwrap();
        assert_eq!(ids, ["rect2", "rect1"]);
    }

    #[test]
    fn add_is_atomic() {
        let mut s = scene();
        let err = add_layers(
            &mut s,
            Map::new(),
            vec![
                json!({"type": "rect"}),
                json!({"type": "text", "text": "x", "fontsize": 3}),
            ],
        )
        .unwrap_err();
        assert!(
            err.starts_with("layers[1]") && err.contains("fontsize"),
            "{err}"
        );
        assert_eq!(s, scene());
        assert!(
            add_layers(
                &mut s,
                Map::new(),
                vec![json!({"type": "rect", "parent": "t2"})]
            )
            .is_err()
        );
        assert!(
            add_layers(
                &mut s,
                Map::new(),
                vec![json!({"id": "bar", "type": "rect"})]
            )
            .unwrap_err()
            .contains("duplicate")
        );
    }

    #[test]
    fn update_by_role_hits_every_match_and_merges() {
        let mut s = scene();
        let changed = update_layers(
            &mut s,
            &[op(json!({"target": {"role": "cta"}, "set": {"color": "#FFFFFF", "constraints": {"h": "center"}}}))],
        )
        .unwrap();
        assert_eq!(changed, ["label", "t2"]);
        update_layers(
            &mut s,
            &[op(
                json!({"target": {"id": "t2"}, "set": {"constraints": {"v": "bottom"}}}),
            )],
        )
        .unwrap();
        let t2 = serde_json::to_value(&s.layers[1]).unwrap();
        assert_eq!(t2["constraints"], json!({"h": "center", "v": "bottom"}));
        assert_eq!(t2["color"], "#FFFFFF");
    }

    #[test]
    fn null_resets_and_delete_removes() {
        let mut s = scene();
        update_layers(
            &mut s,
            &[op(json!({"target": {"id": "t2"}, "set": {"role": null}}))],
        )
        .unwrap();
        assert_eq!(s.layers[1].role, None);
        update_layers(
            &mut s,
            &[op(json!({"target": {"id": "label"}, "delete": true}))],
        )
        .unwrap();
        let Kind::Frame { children, .. } = &s.layers[0].kind else {
            panic!()
        };
        assert!(children.is_empty());
    }

    #[test]
    fn update_errors_leave_scene_untouched() {
        let mut s = scene();
        for bad in [
            json!({"target": {"id": "nope"}, "set": {"x": 1}}),
            json!({"target": {"id": "t2"}, "set": {"type": "rect"}}),
            json!({"target": {"id": "t2"}, "set": {"weight": 450}}),
            json!({"target": {"id": "t2"}}),
        ] {
            assert!(
                update_layers(
                    &mut s,
                    &[
                        op(json!({"target": {"id": "t2"}, "set": {"x": 5}})),
                        op(bad.clone())
                    ]
                )
                .is_err(),
                "{bad}"
            );
            assert_eq!(s, scene());
        }
    }
}
