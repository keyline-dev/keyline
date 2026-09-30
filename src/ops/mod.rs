//! Scene edits behind the MCP tools. Each applies to a copy of the scene and
//! is committed only if the whole batch validates, so batches are atomic.

mod guesses;
#[cfg(test)]
mod reuse_tests;
mod scene_set;
#[cfg(test)]
mod tests;

use std::collections::HashSet;

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::scene::{Layer, Scene, check_keys};

pub(crate) use guesses::dollar::hints as dollar_hints;
pub(crate) use guesses::normalize;

/// `{ id }` or `{ role }`. A role may match several layers.
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum Target {
    // Before `Role`: untagged variants are tried in order, and a
    // `{component, role}` target must not read as a bare `{role}`.
    /// A component's template, or with `role` a layer inside it; `set`
    /// changes every instance, `delete` removes it.
    Component {
        /// Component name.
        component: String,
        /// A layer inside the template, by role.
        #[serde(default)]
        role: Option<String>,
    },
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
    /// A named style; `set` creates or changes it, `delete` removes it.
    Style {
        /// Style name.
        style: String,
    },
    /// The scene itself (`{"scene": true}`): `set` changes its background,
    /// sizes or timing.
    Scene {
        /// Always true.
        scene: bool,
    },
}

impl Target {
    fn matches(&self, l: &Layer) -> bool {
        match self {
            Target::Id { id } => l.id == *id,
            Target::Role { role } => l.role.as_deref() == Some(role),
            Target::Style { .. } | Target::Component { .. } | Target::Scene { .. } => false,
        }
    }
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Target::Id { id } => write!(f, "id {id}"),
            Target::Role { role } => write!(f, "role {role}"),
            Target::Style { style } => write!(f, "style {style}"),
            Target::Scene { .. } => f.write_str("the scene"),
            Target::Component {
                component,
                role: None,
            } => write!(f, "component {component}"),
            Target::Component {
                component,
                role: Some(r),
            } => write!(f, "role {r} in component {component}"),
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
    /// Turn a `use` layer into plain layers that no longer follow its component.
    #[serde(default)]
    pub detach: bool,
}

/// What an edit may also carry besides layers: shared styles, tokens and
/// components, added or replaced by name.
#[derive(Debug, Clone, Default)]
pub struct Shared {
    /// Styles by name.
    pub styles: Map<String, Value>,
    /// Tokens by name.
    pub tokens: Map<String, Value>,
    /// Components by name.
    pub components: Map<String, Value>,
}

/// Adds or replaces shared styles, tokens and components; tokens that
/// changed are re-applied to every layer bound to them.
fn share(next: &mut Scene, shared: Shared) -> Result<(), String> {
    // Tokens first, so styles and components written alongside can use them.
    let mut changed = Vec::new();
    for (name, v) in shared.tokens {
        if !crate::reuse::tokens::is_name(&name) {
            return Err(format!("bad token name {name}: letters, digits, _ . -"));
        }
        if name == crate::reuse::tokens::COUNT {
            return Err("{{n}} is the counting number; give the token another name".into());
        }
        if next.tokens.insert(name.clone(), v).is_some() {
            changed.push(name);
        }
    }
    for (name, style) in shared.styles {
        let Value::Object(mut style) = style else {
            return Err(format!("style {name} must be an object of layer fields"));
        };
        // `type` says what the style is for: it steers the guesses, then goes.
        let kind = style.remove("type");
        guesses::fields(&mut style, kind.as_ref().and_then(Value::as_str));
        let mut style = Value::Object(style);
        guesses::dollar::whole(&mut style, &next.tokens);
        if let Value::Object(style) = style {
            next.styles.insert(name, style);
        }
    }
    for (name, mut c) in shared.components {
        if !c.is_object() {
            return Err(format!("component {name} must be a layer object"));
        }
        guesses::dollar::whole(&mut c, &next.tokens);
        next.components.insert(name, c);
    }
    crate::reuse::tokens::rebind(next, &changed)
}

/// Adds or replaces text `styles`, then appends layers, each on top of its
/// parent frame (or the scene root when it has no `parent`). Returns the
/// ids of the added top-level layers.
pub fn add_layers(
    scene: &mut Scene,
    shared: Shared,
    layers: Vec<Value>,
) -> Result<Vec<String>, String> {
    let mut next = scene.clone();
    share(&mut next, shared)?;
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
        guesses::dollar::whole(&mut v, &next.tokens);
        crate::reuse::tokens::bind(&mut v, &next.tokens).map_err(at)?;
        let mut layer = parse_layer(&v).map_err(|e| at(blame_token(&v, e)))?;
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
pub fn update_layers(scene: &mut Scene, shared: Shared, ops: &[Op]) -> Result<Vec<String>, String> {
    let mut next = scene.clone();
    share(&mut next, shared)?;
    let mut changed = Vec::new();
    for (i, op) in ops.iter().enumerate() {
        let at = |e: String| format!("ops[{i}]: {e}");
        if usize::from(op.delete) + usize::from(op.set.is_some()) + usize::from(op.detach) != 1 {
            return Err(at("give exactly one of set, delete or detach".into()));
        }
        if op.detach {
            let Target::Id { id } = &op.target else {
                return Err(at("detach needs {id} of a use layer".into()));
            };
            crate::reuse::components::detach(&mut next, id).map_err(at)?;
            changed.push(id.clone());
            continue;
        }
        if let Target::Scene { .. } = &op.target {
            let set = op
                .set
                .as_ref()
                .ok_or_else(|| at("the scene can only be set".into()))?;
            scene_set::set_scene(&mut next, set).map_err(at)?;
            changed.push("scene".into());
            continue;
        }
        if let Target::Component { component, role } = &op.target {
            recompose(&mut next, component, role.as_deref(), op).map_err(at)?;
            changed.push(component.clone());
            continue;
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
        apply(&mut next.layers, op, &next.tokens, &mut changed).map_err(at)?;
        if changed.len() == before {
            return Err(at(format!("no layer with {}", op.target)));
        }
    }
    next.validate()?;
    next.version += 1;
    *scene = next;
    Ok(changed)
}

/// Changes (merge patch) or deletes component `name`, or with `role` the
/// layer inside it that has that role.
fn recompose(scene: &mut Scene, name: &str, role: Option<&str>, op: &Op) -> Result<(), String> {
    let unknown = |scene: &Scene| {
        let known: Vec<&str> = scene.components.keys().map(String::as_str).collect();
        format!("no component {name}; components: {}", known.join(", "))
    };
    if !scene.components.contains_key(name) {
        return Err(unknown(scene));
    }
    if op.delete && role.is_none() {
        scene.components.remove(name);
        return Ok(());
    }
    let Some(root) = scene.components.get_mut(name) else {
        return Err(unknown(scene));
    };
    fn find<'v>(v: &'v mut Value, role: &str) -> Option<&'v mut Value> {
        if v.get("role").and_then(Value::as_str) == Some(role) {
            return Some(v);
        }
        v.get_mut("children")?
            .as_array_mut()?
            .iter_mut()
            .find_map(|c| find(c, role))
    }
    let slot = match role {
        None => root,
        Some(r) => find(root, r).ok_or_else(|| format!("no role {r} in component {name}"))?,
    };
    match &op.set {
        Some(set) => merge_patch(slot, &Value::Object(set.clone())),
        None => *slot = Value::Null,
    }
    // A deleted inner layer leaves a null in its parent's children.
    fn prune(v: &mut Value) {
        if let Some(Value::Array(c)) = v.get_mut("children") {
            c.retain(|x| !x.is_null());
            c.iter_mut().for_each(prune);
        }
    }
    if let Some(root) = scene.components.get_mut(name) {
        prune(root);
        // Deleting the root by its role deletes the component.
        if root.is_null() {
            scene.components.remove(name);
        }
    }
    Ok(())
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
            let mut set = Value::Object(set.clone());
            guesses::dollar::whole(&mut set, &scene.tokens);
            merge_patch(&mut v, &set);
            if let Value::Object(mut style) = v {
                guesses::fields(&mut style, None);
                scene.styles.insert(name.to_owned(), style);
            }
            Ok(())
        }
    }
}

fn apply(
    layers: &mut Vec<Layer>,
    op: &Op,
    tokens: &std::collections::BTreeMap<String, Value>,
    changed: &mut Vec<String>,
) -> Result<(), String> {
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
                // An explicit value replaces a token binding; a new "{{name}}" binds again.
                crate::reuse::tokens::unbind_set(&mut v, set);
                // A field the set mentions is no longer a kept default:
                // its new value (or its reset) decides.
                if let Some(Value::Object(kept)) = v.get_mut("$defaults") {
                    kept.retain(|k, _| !set.contains_key(k));
                }
                let mut set = Value::Object(set.clone());
                guesses::dollar::whole(&mut set, tokens);
                merge_patch(&mut v, &set);
                crate::reuse::tokens::bind(&mut v, tokens)
                    .map_err(|e| format!("{}: {e}", layers[i].id))?;
                layers[i] = parse_layer(&v)
                    .map_err(|e| format!("{}: {}", layers[i].id, blame_token(&v, e)))?;
            }
        }
        if let Some(children) = layers[i].kind.children_mut() {
            apply(children, op, tokens, changed)?;
        }
        i += 1;
    }
    Ok(())
}

/// Parses a layer and rejects unknown keys, recursing into frame children.
pub(crate) fn parse_layer(v: &Value) -> Result<Layer, String> {
    let mut v = v.clone();
    normalize(&mut v);
    let mut layer: Layer = serde_json::from_value(v.clone()).map_err(|e| e.to_string())?;
    check_keys(&v, &layer)?;
    layer.own_defaults = if layer.style.is_some() {
        explicit_defaults(&v, &layer)
    } else {
        Map::new()
    };
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

/// Names the token behind a layer that doesn't parse, when dropping one
/// bound field makes it parse: a token whose value doesn't suit a field.
fn blame_token(v: &Value, e: String) -> String {
    let Some(Value::Object(refs)) = v.get("$tokens") else {
        return e;
    };
    for (ptr, name) in refs {
        let mut without = v.clone();
        let Some((parent, key)) = ptr.rsplit_once('/') else {
            continue;
        };
        let key = key.replace("~1", "/").replace("~0", "~");
        let dropped = without
            .pointer_mut(parent)
            .and_then(Value::as_object_mut)
            .is_some_and(|o| o.remove(&key).is_some());
        if dropped && parse_layer(&without).is_ok() {
            let field = ptr.trim_start_matches('/');
            let written = name.as_str().unwrap_or_default();
            // A whole value's name, or the template that spliced tokens in.
            let shown = if written.contains("{{") {
                written.to_owned()
            } else {
                crate::reuse::tokens::braced(written)
            };
            return format!("token {shown} doesn't suit {field}: {e}");
        }
    }
    e
}

/// The fields `v` gives (or kept from before, in `$defaults`) that `layer`
/// holds at their default value, so storage would drop them. A field
/// counts only when adding it back leaves the layer the same, which rules
/// out short forms stored under another name.
fn explicit_defaults(v: &Value, layer: &Layer) -> Map<String, Value> {
    let mut plain = layer.clone();
    plain.own_defaults.clear();
    let Ok(Value::Object(saved)) = serde_json::to_value(&plain) else {
        return Map::new();
    };
    let mut given = match v.get("$defaults") {
        Some(Value::Object(kept)) => kept.clone(),
        _ => Map::new(),
    };
    if let Value::Object(o) = v {
        for (k, val) in o {
            given.insert(k.clone(), val.clone());
        }
    }
    given
        .into_iter()
        .filter(|(k, val)| {
            !k.starts_with('$') && !saved.contains_key(k) && {
                let mut with = saved.clone();
                with.insert(k.clone(), val.clone());
                serde_json::from_value::<Layer>(Value::Object(with)).is_ok_and(|w| w == plain)
            }
        })
        .collect()
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
