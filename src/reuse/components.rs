//! Components: named layer trees placed by `use` layers. A `use` becomes one
//! instance (its `props`), or one per entry of `each`, in the parent's
//! flow. `{{prop}}` in a component's strings is filled from the instance's
//! props; the `use` layer's own fields (width, constraints, …) apply to
//! each instance's root. Ids are the use's id, then the instance number
//! (with `each`), then each inner layer's id or role: `cands.1.name`.

use serde_json::{Map, Value};

use crate::scene::{Kind, Layer, Scene};

/// Components may place components, this many levels deep.
const MAX_DEPTH: usize = 8;

/// Replaces every `use` layer in `layers` (at any depth) with its instances.
///
/// # Errors
/// An unknown component, a bad template, or components nested too deep.
pub fn expand(scene: &Scene, layers: &mut Vec<Layer>) -> Result<(), String> {
    expand_at(scene, layers, 0)
}

fn expand_at(scene: &Scene, layers: &mut Vec<Layer>, depth: usize) -> Result<(), String> {
    let mut i = 0;
    while i < layers.len() {
        if let Kind::Use { .. } = layers[i].kind {
            let mut instances = instances(scene, &layers[i], depth)?;
            expand_at(scene, &mut instances, depth + 1)?;
            let n = instances.len();
            layers.splice(i..=i, instances);
            i += n;
            continue;
        }
        if let Some(children) = layers[i].kind.children_mut() {
            expand_at(scene, children, depth)?;
        }
        i += 1;
    }
    Ok(())
}

/// The instances of one `use` layer, parsed and with their ids.
fn instances(scene: &Scene, u: &Layer, depth: usize) -> Result<Vec<Layer>, String> {
    let Kind::Use {
        component,
        props,
        each,
    } = &u.kind
    else {
        return Ok(Vec::new());
    };
    if depth >= MAX_DEPTH {
        return Err(format!(
            "{}: components nest more than {MAX_DEPTH} deep",
            u.id
        ));
    }
    let template = scene.components.get(component).ok_or_else(|| {
        let known: Vec<&str> = scene.components.keys().map(String::as_str).collect();
        format!(
            "{}: unknown component {component}; components: {}",
            u.id,
            known.join(", ")
        )
    })?;
    // The use layer's own fields, minus what makes it a use, go on each root.
    let mut own = match serde_json::to_value(u).map_err(|e| e.to_string())? {
        Value::Object(o) => o,
        _ => Map::new(),
    };
    for k in ["id", "type", "component", "props", "each", "$tokens"] {
        own.remove(k);
    }
    // `stagger` on a use: its instances enter one after another.
    let stagger = own
        .remove("stagger")
        .and_then(|v| v.as_f64())
        .map(|g| g as f32);
    let entries: Vec<Map<String, Value>> = if each.is_empty() {
        vec![props.clone()]
    } else {
        each.iter()
            .map(|e| {
                let mut p = props.clone();
                p.extend(e.clone());
                p
            })
            .collect()
    };
    entries
        .iter()
        .enumerate()
        .map(|(n, p)| {
            let mut v = template.clone();
            fill(&mut v, p);
            super::tokens::substitute(&mut v, &scene.tokens)
                .map_err(|e| format!("{}: {e}", u.id))?;
            let root = if each.is_empty() {
                u.id.clone()
            } else {
                format!("{}.{n}", u.id)
            };
            if let Value::Object(o) = &mut v {
                for (k, x) in &own {
                    o.insert(k.clone(), x.clone());
                }
                if let (Some(gap), Some(enter)) = (stagger, o.get("enter")) {
                    let m: crate::anim::motion::Motion = serde_json::from_value(enter.clone())
                        .map_err(|e| format!("{}: in: {e}", u.id))?;
                    let delayed = serde_json::to_value(m.delayed(n as f32 * gap))
                        .map_err(|e| e.to_string())?;
                    o.insert("enter".into(), delayed);
                }
            }
            name(&mut v, &root);
            crate::ops::parse_layer(&v)
                .map_err(|e| format!("{} (component {component}): {e}", u.id))
        })
        .collect()
}

/// Fills `{{prop}}` placeholders: a string that is only a placeholder takes
/// the prop's value as is (a number stays a number); otherwise it's spliced
/// into the text. Unknown placeholders are left alone.
fn fill(v: &mut Value, props: &Map<String, Value>) {
    match v {
        Value::String(s) => {
            // `{{name}}`, as in Mustache and Handlebars.
            if let Some(key) = s.strip_prefix("{{").and_then(|r| r.strip_suffix("}}"))
                && let Some(p) = props.get(key.trim())
            {
                *v = p.clone();
                return;
            }
            if s.contains("{{") {
                for (k, p) in props {
                    let text = match p {
                        Value::String(t) => t.clone(),
                        other => other.to_string(),
                    };
                    *s = s.replace(&format!("{{{{{k}}}}}"), &text);
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| fill(x, props)),
        Value::Object(o) => o.values_mut().for_each(|x| fill(x, props)),
        _ => {}
    }
}

/// Gives the instance root `root` as its id and every inner layer
/// `parent.<own id, role, or type+index>`.
fn name(v: &mut Value, root: &str) {
    let Some(o) = v.as_object_mut() else { return };
    o.insert("id".into(), Value::String(root.to_owned()));
    if let Some(Value::Array(children)) = o.get_mut("children") {
        for (i, c) in children.iter_mut().enumerate() {
            let part = c
                .get("id")
                .or_else(|| c.get("role"))
                .and_then(Value::as_str)
                .map_or_else(
                    || {
                        format!(
                            "{}{i}",
                            c.get("type").and_then(Value::as_str).unwrap_or("layer")
                        )
                    },
                    str::to_owned,
                );
            name(c, &format!("{root}.{part}"));
        }
    }
}

/// Turns the `use` layer `id` into plain layers: its instances, which then
/// no longer follow the component.
///
/// # Errors
/// No such `use` layer, or expansion errors.
pub fn detach(scene: &mut Scene, id: &str) -> Result<(), String> {
    fn go(scene: &Scene, layers: &mut Vec<Layer>, id: &str) -> Result<bool, String> {
        if let Some(i) = layers
            .iter()
            .position(|l| l.id == id && matches!(l.kind, Kind::Use { .. }))
        {
            let inst = instances(scene, &layers[i], 0)?;
            layers.splice(i..=i, inst);
            return Ok(true);
        }
        for l in layers.iter_mut() {
            if let Some(children) = l.kind.children_mut()
                && go(scene, children, id)?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
    let snapshot = scene.clone();
    if go(&snapshot, &mut scene.layers, id)? {
        Ok(())
    } else {
        Err(format!("no use layer {id}"))
    }
}

#[cfg(test)]
mod tests {
    use super::{expand, fill};
    use crate::scene::Scene;
    use serde_json::json;

    fn scene(layers: serde_json::Value) -> Scene {
        let mut v = json!({"width": 400,
            "height": 400,
            "sizes": [{"id": "a", "width": 400, "height": 400}],
            "tokens": {"red": "#D0202E"},
            "components": {"card": {"type": "frame", "flexDirection": "column", "alignItems": "flex-start", "children": [{"type": "text", "role": "name", "text": "{{name}}", "color": "$red"}, {"type": "text", "role": "office", "text": "Runs for {{office}}", "fontSize": "{{size}}"}]}}});
        v["layers"] = layers;
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn each_places_one_instance_per_entry_with_stable_ids() {
        let s = scene(
            json!([{"id": "cands", "type": "use", "component": "card", "props": {"size": 20},
            "width": "fill", "each": [{"name": "Dana", "office": "Mayor"}, {"name": "Omar", "office": "Council"}]}]),
        );
        let mut layers = s.layers.clone();
        expand(&s, &mut layers).unwrap();
        assert_eq!(
            layers.iter().map(|l| l.id.as_str()).collect::<Vec<_>>(),
            ["cands.0", "cands.1"]
        );
        let v = serde_json::to_value(&layers[1]).unwrap();
        assert_eq!(v["width"], "fill");
        assert_eq!(v["children"][0]["id"], "cands.1.name");
        assert_eq!(v["children"][0]["text"], "Omar");
        assert_eq!(v["children"][0]["color"], "#D0202E");
        assert_eq!(v["children"][1]["text"], "Runs for Council");
        assert_eq!(v["children"][1]["fontSize"], 20.0);
    }

    #[test]
    fn unknown_components_list_the_known_ones() {
        let s = scene(json!([{"id": "x", "type": "use", "component": "cardd"}]));
        let mut layers = s.layers.clone();
        let e = expand(&s, &mut layers).unwrap_err();
        assert_eq!(e, "x: unknown component cardd; components: card");
    }

    #[test]
    fn placeholders_keep_numbers_and_splice_into_text() {
        let mut v = json!({"a": "{{n}}", "b": "{{n}} px", "c": "{{missing}}"});
        fill(&mut v, &serde_json::from_value(json!({"n": 3})).unwrap());
        assert_eq!(v, json!({"a": 3, "b": "3 px", "c": "{{missing}}"}));
    }
}
