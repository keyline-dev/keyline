//! Tokens: named values used as `"$name"` in any field of a layer, style or
//! component. `text` is never scanned, so `"$29"` or `"$SALE"` stays text.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::scene::{Layer, Scene};

/// The token name in `s` when the whole string is a reference: `$` then a
/// letter or `_`, then letters, digits, `_`, `.` or `-`.
pub fn reference(s: &str) -> Option<&str> {
    let name = s.strip_prefix('$')?;
    let mut chars = name.chars();
    let first = chars.next()?;
    ((first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c)))
    .then_some(name)
}

/// Replaces token references in a raw layer tree, recording in each layer
/// object (`"$tokens"`) which JSON pointer came from which token.
///
/// # Errors
/// An unknown token, naming the tokens there are.
pub fn bind(layer: &mut Value, tokens: &BTreeMap<String, Value>) -> Result<(), String> {
    let Some(obj) = layer.as_object_mut() else {
        return Ok(());
    };
    let mut refs: BTreeMap<String, String> = match obj.remove("$tokens") {
        Some(Value::Object(m)) => m
            .into_iter()
            .filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_owned())))
            .collect(),
        _ => BTreeMap::new(),
    };
    for (key, val) in obj.iter_mut() {
        match key.as_str() {
            "children" => {
                if let Value::Array(children) = val {
                    for c in children {
                        bind(c, tokens)?;
                    }
                }
            }
            "text" => text_token(val, "/text", tokens, &mut refs),
            "id" | "type" | "$tokens" => {}
            _ => replace(val, &format!("/{}", escape(key)), tokens, &mut refs)?,
        }
    }
    if !refs.is_empty() {
        obj.insert(
            "$tokens".into(),
            Value::Object(
                refs.into_iter()
                    .map(|(k, v)| (k, Value::String(v)))
                    .collect(),
            ),
        );
    }
    Ok(())
}

/// Text is a token only when it's wholly the name of one: a template's
/// "$headline". "$29", or "$SALE" with no such token, stays text.
fn text_token(
    val: &mut Value,
    at: &str,
    tokens: &BTreeMap<String, Value>,
    refs: &mut BTreeMap<String, String>,
) {
    if let Some(name) = val.as_str().and_then(reference)
        && let Some(value) = tokens.get(name)
    {
        refs.insert(at.to_owned(), name.to_owned());
        *val = value.clone();
    }
}

/// Replaces token references anywhere in `v` (styles, components), without
/// recording them.
///
/// # Errors
/// An unknown token.
pub fn substitute(v: &mut Value, tokens: &BTreeMap<String, Value>) -> Result<(), String> {
    replace(v, "", tokens, &mut BTreeMap::new())
}

fn replace(
    v: &mut Value,
    at: &str,
    tokens: &BTreeMap<String, Value>,
    refs: &mut BTreeMap<String, String>,
) -> Result<(), String> {
    match v {
        Value::String(s) => {
            if let Some(name) = reference(s) {
                let value = tokens.get(name).ok_or_else(|| unknown(name, tokens))?;
                refs.insert(at.to_owned(), name.to_owned());
                *v = value.clone();
            }
        }
        Value::Array(a) => {
            for (i, x) in a.iter_mut().enumerate() {
                replace(x, &format!("{at}/{i}"), tokens, refs)?;
            }
        }
        Value::Object(o) => {
            for (k, x) in o.iter_mut() {
                let ptr = format!("{at}/{}", escape(k));
                if k == "text" {
                    text_token(x, &ptr, tokens, refs);
                } else {
                    replace(x, &ptr, tokens, refs)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn unknown(name: &str, tokens: &BTreeMap<String, Value>) -> String {
    let known: Vec<&str> = tokens.keys().map(String::as_str).collect();
    if known.is_empty() {
        format!("unknown token ${name}; add it under tokens")
    } else {
        format!("unknown token ${name}; tokens: {}", known.join(", "))
    }
}

/// Replaces tokens used as markup attribute values in text, as in
/// `<span color="$red">`; the rest of the text is never scanned.
pub fn in_markup(text: &mut String, tokens: &BTreeMap<String, Value>) {
    if !text.contains("=$") && !text.contains("=\"$") && !text.contains("='$") {
        return;
    }
    for (name, v) in tokens {
        let value = match v {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            _ => continue,
        };
        for q in ['"', '\''] {
            *text = text.replace(&format!("={q}${name}{q}"), &format!("={q}{value}{q}"));
        }
        // Unquoted, ended by a space or the tag's end.
        for end in [' ', '>', '/'] {
            *text = text.replace(&format!("=${name}{end}"), &format!("=\"{value}\"{end}"));
        }
    }
}

/// Puts `value` at JSON pointer `ptr` in `v`, adding missing object keys
/// on the way: a field whose first token value was its default isn't
/// saved, so it's absent when the token changes.
fn set_at(v: &mut Value, ptr: &str, value: Value) {
    let mut at = v;
    for key in ptr.split('/').skip(1) {
        let key = key.replace("~1", "/").replace("~0", "~");
        if at.is_null() {
            *at = Value::Object(Map::new());
        }
        at = match at {
            Value::Object(o) => o.entry(key).or_insert(Value::Null),
            Value::Array(a) => match key.parse::<usize>().ok().and_then(|i| a.get_mut(i)) {
                Some(x) => x,
                None => return,
            },
            _ => return,
        };
    }
    *at = value;
}

/// JSON-pointer escaping of one key.
fn escape(k: &str) -> String {
    k.replace('~', "~0").replace('/', "~1")
}

/// Forgets the bindings of fields a `set` overwrites, so a later token
/// change doesn't undo an explicit edit.
pub fn unbind_set(layer: &mut Value, set: &Map<String, Value>) {
    if let Some(Value::Object(refs)) = layer.get_mut("$tokens") {
        refs.retain(|ptr, _| {
            !set.keys().any(|k| {
                ptr == &format!("/{}", escape(k)) || ptr.starts_with(&format!("/{}/", escape(k)))
            })
        });
    }
}

/// Re-applies `changed` tokens to every layer bound to them.
///
/// # Errors
/// A layer that no longer parses with the new value.
pub fn rebind(scene: &mut Scene, changed: &[String]) -> Result<(), String> {
    fn go(
        layers: &mut [Layer],
        tokens: &BTreeMap<String, Value>,
        changed: &[String],
    ) -> Result<(), String> {
        for l in layers.iter_mut() {
            if l.token_refs.values().any(|t| changed.contains(t)) {
                let mut v = serde_json::to_value(&*l).map_err(|e| e.to_string())?;
                for (ptr, name) in &l.token_refs {
                    if changed.contains(name)
                        && let Some(value) = tokens.get(name)
                    {
                        set_at(&mut v, ptr, value.clone());
                    }
                }
                *l = serde_json::from_value(v).map_err(|e| {
                    let names: Vec<String> = l
                        .token_refs
                        .values()
                        .filter(|t| changed.contains(t))
                        .map(|t| format!("${t}"))
                        .collect();
                    format!("{}: token {} doesn't suit it: {e}", l.id, names.join(", "))
                })?;
            }
            if let Some(children) = l.kind.children_mut() {
                go(children, tokens, changed)?;
            }
        }
        Ok(())
    }
    go(&mut scene.layers, &scene.tokens, changed)
}

#[cfg(test)]
mod tests {
    use super::{bind, in_markup, reference};
    use serde_json::json;

    #[test]
    fn only_whole_token_strings_are_references() {
        assert_eq!(reference("$brand"), Some("brand"));
        assert_eq!(reference("$color.brand"), Some("color.brand"));
        assert_eq!(reference("$29"), None);
        assert_eq!(reference("pay $5"), None);
    }

    #[test]
    fn binding_substitutes_and_remembers_but_leaves_text_alone() {
        let tokens = serde_json::from_value(json!({"red": "#D0202E", "pad": 24})).unwrap();
        let mut v = json!({"type": "frame", "color": "$red", "stack": {"dir": "row", "padding": "$pad"},
            "children": [{"type": "text", "text": "$SALE", "color": "$red"}]});
        bind(&mut v, &tokens).unwrap();
        assert_eq!(v["color"], "#D0202E");
        assert_eq!(v["stack"]["padding"], 24);
        assert_eq!(
            v["$tokens"],
            json!({"/color": "red", "/stack/padding": "pad"})
        );
        assert_eq!(v["children"][0]["text"], "$SALE");
        assert_eq!(v["children"][0]["$tokens"], json!({"/color": "red"}));
        let e = bind(&mut json!({"type": "rect", "color": "$blue"}), &tokens).unwrap_err();
        assert_eq!(e, "unknown token $blue; tokens: pad, red");
    }

    #[test]
    fn a_default_first_value_still_follows_its_token() {
        let mut v = json!({"type": "text"});
        super::set_at(&mut v, "/color", json!("#D0202E"));
        super::set_at(&mut v, "/stack/padding", json!(8));
        assert_eq!(
            v,
            json!({"type": "text", "color": "#D0202E", "stack": {"padding": 8}})
        );
    }

    #[test]
    fn text_that_is_wholly_a_token_name_is_bound() {
        let tokens = serde_json::from_value(json!({"headline": "Summer sale"})).unwrap();
        let mut v = json!({"type": "text", "text": "$headline"});
        bind(&mut v, &tokens).unwrap();
        assert_eq!(v["text"], "Summer sale");
        assert_eq!(v["$tokens"], json!({"/text": "headline"}));
        let mut v =
            json!({"type": "text", "text": "$headline", "at": {"sky": {"text": "$headline"}}});
        bind(&mut v, &tokens).unwrap();
        assert_eq!(v["at"]["sky"]["text"], "Summer sale", "in at too");
        let mut v = json!({"type": "text", "text": "$headlines"});
        bind(&mut v, &tokens).unwrap();
        assert_eq!(v["text"], "$headlines", "no such token: still text");
    }

    #[test]
    fn tokens_fill_markup_attributes_only() {
        let tokens = serde_json::from_value(json!({"red": "#D0202E", "big": 60})).unwrap();
        let mut t = r#"Pay $red <span color="$red" fontSize='$big'>now</span>"#.to_owned();
        in_markup(&mut t, &tokens);
        assert_eq!(
            t,
            r##"Pay $red <span color="#D0202E" fontSize='60'>now</span>"##
        );
        let mut t = "<span color=$red>A</span> <span weight=700 color=$red>B</span>".to_owned();
        in_markup(&mut t, &tokens);
        assert_eq!(
            t,
            r##"<span color="#D0202E">A</span> <span weight=700 color="#D0202E">B</span>"##
        );
    }
}
