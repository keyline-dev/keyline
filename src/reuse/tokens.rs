//! Tokens: named values used as `{{name}}` in any field of a layer, style or
//! component, Mustache-style. A string that is only `{{name}}` takes the
//! token's value as is (a number stays a number); otherwise the value is
//! spliced into the text, markup attributes included. A string without a
//! `{{name}}` is never touched, so `"$29"` or `"{curly}"` stays text.

use std::collections::BTreeMap;
use std::ops::Range;

use serde_json::{Map, Value};

use crate::scene::{Layer, Scene};

/// The counting number's placeholder name (`{{n}}`): never a token.
pub const COUNT: &str = "n";

/// Whether `s` is a token name: a letter or `_`, then letters, digits, `_`,
/// `.` or `-`.
pub fn is_name(s: &str) -> bool {
    let mut chars = s.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c))
}

/// The placeholders in `s`: each `{{name}}`'s byte range and name. Braces
/// around anything but a name, and the counting `{{n}}`, aren't
/// placeholders.
pub fn placeholders(s: &str) -> Vec<(Range<usize>, &str)> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(open) = s[from..].find("{{").map(|i| from + i) {
        let Some(close) = s[open + 2..].find("}}").map(|i| open + 2 + i) else {
            break;
        };
        let name = s[open + 2..close].trim();
        if is_name(name) && name != COUNT {
            out.push((open..close + 2, name));
        }
        from = close + 2;
    }
    out
}

/// `name` as a placeholder: `{{name}}`.
pub fn braced(name: &str) -> String {
    format!("{{{{{name}}}}}")
}

/// The token name when all of `s` is one placeholder: `"{{brand}}"`.
pub fn reference(s: &str) -> Option<&str> {
    match placeholders(s).as_slice() {
        [(range, name)] if *range == (0..s.len()) => Some(name),
        _ => None,
    }
}

/// `s` with every placeholder replaced by its token's value.
///
/// # Errors
/// An unknown token, or one whose value is a list or object.
fn splice(s: &str, tokens: &BTreeMap<String, Value>) -> Result<String, String> {
    let mut out = String::with_capacity(s.len());
    let mut last = 0;
    for (range, name) in placeholders(s) {
        out.push_str(&s[last..range.start]);
        match tokens.get(name).ok_or_else(|| unknown(name, tokens))? {
            Value::String(t) => out.push_str(t),
            v @ (Value::Number(_) | Value::Bool(_)) => out.push_str(&v.to_string()),
            _ => {
                return Err(format!(
                    "token {} is a list or object; use it as a whole value",
                    braced(name)
                ));
            }
        }
        last = range.end;
    }
    out.push_str(&s[last..]);
    Ok(out)
}

/// What a field bound to tokens was written as: one token's name (a
/// whole value), or its template when tokens sit inside text.
fn bound_names(written: &str) -> Vec<&str> {
    if written.contains("{{") {
        placeholders(written).into_iter().map(|(_, n)| n).collect()
    } else {
        vec![written]
    }
}

/// Replaces tokens in a raw layer tree, recording in each layer object
/// (`"$tokens"`) what each bound JSON pointer was written as: the token's
/// name, or the text's template.
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
            "id" | "type" | "$defaults" => {}
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

/// Replaces tokens anywhere in `v` (styles, components), without recording
/// them.
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
        Value::String(s) if s.contains("{{") => {
            if let Some(name) = reference(s) {
                let value = tokens.get(name).ok_or_else(|| unknown(name, tokens))?;
                refs.insert(at.to_owned(), name.to_owned());
                *v = value.clone();
            } else if !placeholders(s).is_empty() {
                let spliced = splice(s, tokens)?;
                refs.insert(at.to_owned(), std::mem::replace(s, spliced));
            }
        }
        Value::Array(a) => {
            for (i, x) in a.iter_mut().enumerate() {
                replace(x, &format!("{at}/{i}"), tokens, refs)?;
            }
        }
        Value::Object(o) => {
            for (k, x) in o.iter_mut() {
                replace(x, &format!("{at}/{}", escape(k)), tokens, refs)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn unknown(name: &str, tokens: &BTreeMap<String, Value>) -> String {
    let known: Vec<&str> = tokens.keys().map(String::as_str).collect();
    if known.is_empty() {
        format!("unknown token {}; add it under tokens", braced(name))
    } else {
        format!(
            "unknown token {}; tokens: {}",
            braced(name),
            known.join(", ")
        )
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

/// Re-applies `changed` tokens to every layer bound to them: a whole value
/// takes the token's new value, a template is spliced again.
///
/// # Errors
/// A layer that no longer parses with the new value.
pub fn rebind(scene: &mut Scene, changed: &[String]) -> Result<(), String> {
    fn go(
        layers: &mut [Layer],
        tokens: &BTreeMap<String, Value>,
        changed: &[String],
    ) -> Result<(), String> {
        let hit = |written: &str| {
            bound_names(written)
                .iter()
                .any(|n| changed.iter().any(|c| c == n))
        };
        for l in layers.iter_mut() {
            if l.token_refs.values().any(|w| hit(w)) {
                let mut v = serde_json::to_value(&*l).map_err(|e| e.to_string())?;
                for (ptr, written) in l.token_refs.iter().filter(|(_, w)| hit(w)) {
                    let value = if written.contains("{{") {
                        Value::String(
                            splice(written, tokens).map_err(|e| format!("{}: {e}", l.id))?,
                        )
                    } else if let Some(value) = tokens.get(written) {
                        value.clone()
                    } else {
                        continue;
                    };
                    set_at(&mut v, ptr, value);
                }
                *l = serde_json::from_value(v).map_err(|e| {
                    let names: Vec<String> = l
                        .token_refs
                        .values()
                        .flat_map(|w| bound_names(w))
                        .filter(|n| changed.iter().any(|c| c == n))
                        .map(braced)
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
mod tests;
