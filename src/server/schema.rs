//! Tool definitions go to the model on every turn, so their JSON schemas are
//! trimmed to what a model needs: argument names, types and the one-line
//! descriptions of top-level arguments. Nested types' docs (written for
//! developers), `$schema`, `format` and `additionalProperties` go. The
//! server still validates every argument itself.

use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use serde_json::Value;

/// Keys a model never needs in a tool schema.
const DROP: &[&str] = &[
    "$schema",
    "format",
    "additionalProperties",
    "minItems",
    "maxItems",
];

/// Trims every tool's input schema in place.
pub(super) fn compact_all<S>(router: &mut ToolRouter<S>) {
    for route in router.map.values_mut() {
        let mut schema = Value::Object(route.attr.input_schema.as_ref().clone());
        compact(&mut schema, true);
        if let Value::Object(o) = schema {
            route.attr.input_schema = Arc::new(o);
        }
    }
}

/// Drops the unneeded keys everywhere, and descriptions below the top-level
/// properties (`keep_docs` is true only for the root object's properties).
fn compact(v: &mut Value, keep_docs: bool) {
    match v {
        Value::Object(o) => {
            for k in DROP {
                o.remove(*k);
            }
            for (key, child) in o.iter_mut() {
                match key.as_str() {
                    "properties" if keep_docs => props(child),
                    _ => {
                        if let Value::Object(c) = child {
                            c.remove("description");
                        }
                        compact(child, false);
                    }
                }
            }
        }
        Value::Array(a) => {
            for x in a {
                if let Value::Object(c) = x {
                    c.remove("description");
                }
                compact(x, false);
            }
        }
        _ => {}
    }
}

/// Top-level arguments keep their own description, nothing deeper.
fn props(v: &mut Value) {
    if let Value::Object(ps) = v {
        for p in ps.values_mut() {
            if let Value::Object(o) = p {
                let doc = o.remove("description");
                let mut rest = Value::Object(std::mem::take(o));
                compact(&mut rest, false);
                if let Value::Object(mut r) = rest {
                    if let Some(d) = doc {
                        r.insert("description".into(), d);
                    }
                    *o = r;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn schemas_keep_top_level_docs_and_drop_the_rest() {
        let mut v = json!({
            "$schema": "x", "type": "object", "additionalProperties": false,
            "properties": {
                "sizes": {"description": "Target sizes.", "type": "array", "items": {"$ref": "#/$defs/Size"}},
                "width": {"type": "number", "format": "float"}
            },
            "$defs": {"Size": {"description": "One output size.", "type": "object",
                "properties": {"id": {"description": "Name used in replies.", "type": "string"}}}}
        });
        super::compact(&mut v, true);
        assert_eq!(
            v,
            json!({
                "type": "object",
                "properties": {
                    "sizes": {"description": "Target sizes.", "type": "array", "items": {"$ref": "#/$defs/Size"}},
                    "width": {"type": "number"}
                },
                "$defs": {"Size": {"type": "object", "properties": {"id": {"type": "string"}}}}
            })
        );
    }
}
