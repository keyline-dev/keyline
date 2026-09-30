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

/// What `layer_add` adds to its description when motion is on.
const LAYER_ADD_MOTION: &str = " Motion (needs the scene's duration, or shots): enter/exit \
fade|fade-up|-down|-left|-right (the way it moves)|pop|zoom-in|zoom-out|blur-in or {effect, delay, duration, ease, \
distance}; animate {opacity|scale|rotate|blur|translate|skew|color|draw (stroke drawn, 0–1)|count (text's {{n}}; \
decimals, separator): [values] or {from,to}, delay, duration, ease, repeat (-1), yoyo, times} or a list; values may be \"random(a,b)\"; ease: GSAP names (power2.out, back.out, \
elastic.inOut, steps(n), none); stagger (s) on frame, use or split text; split chars|words. Durations are seconds. \
video: asset (a clip), fit, focus, crop, filter, trimStart (s into the clip), delay, playbackRate, loop, muted. \
{type: shot, duration, transition} (top level; shots play in turn, their times are their own): transition \
cut|fade|slide-*|push-*|wipe-* (left|right|up|down: the way the new shot moves)|zoom or {type, duration, ease}.";

/// What `asset_add` adds to its description when motion is on.
const ASSET_ADD_MOTION: &str =
    " Or a video (mp4, mov, webm) or sound (mp3, m4a, wav) by path or url.";

/// What `render` adds to its description when motion is on.
const RENDER_MOTION: &str = " apng, gif, mp4 or webm (mp4/webm need ffmpeg) renders the motion, its preview 6 moments; time (s) a still; muted drops the sound.";

/// Arguments that exist only for motion, by tool.
const MOTION_ARGS: &[(&str, &[&str])] = &[
    ("scene_create", &["duration", "fps", "loop"]),
    ("render", &["time", "muted"]),
];

/// Trims every tool's input schema in place, and adds motion's words to the
/// tools, or takes its arguments out (`--no-motion`). `asset_add`'s `path`
/// names the `folders` it may read, and is left out when there are none.
pub(super) fn compact_all<S>(
    router: &mut ToolRouter<S>,
    motion: bool,
    folders: &[std::path::PathBuf],
) {
    for route in router.map.values_mut() {
        let mut schema = Value::Object(route.attr.input_schema.as_ref().clone());
        compact(&mut schema, true);
        if !motion
            && let Some((_, args)) = MOTION_ARGS.iter().find(|(t, _)| *t == route.attr.name)
            && let Some(Value::Object(props)) = schema.get_mut("properties")
        {
            for a in *args {
                props.remove(*a);
            }
        }
        // A local `path` is offered only where the server may read, and
        // says where that is.
        if matches!(route.attr.name.as_ref(), "asset_add" | "scene_create")
            && let Some(Value::Object(props)) = schema.get_mut("properties")
        {
            if folders.is_empty() {
                props.remove("path");
                let d = route.attr.description.clone().unwrap_or_default();
                route.attr.description =
                    Some(d.replace("url, path or base64", "url or base64").into());
            } else if let Some(Value::Object(path)) = props.get_mut("path") {
                let list: Vec<_> = folders.iter().map(|f| f.display().to_string()).collect();
                let or = if route.attr.name == "asset_add" {
                    "Or"
                } else {
                    "or"
                };
                path.insert(
                    "description".into(),
                    format!("{or} a local file in {}", list.join(", ")).into(),
                );
            }
        }
        if let Value::Object(o) = schema {
            route.attr.input_schema = Arc::new(o);
        }
        let extra = match route.attr.name.as_ref() {
            "layer_add" => LAYER_ADD_MOTION,
            "asset_add" => ASSET_ADD_MOTION,
            "render" => RENDER_MOTION,
            _ => "",
        };
        if motion && !extra.is_empty() {
            let base = route.attr.description.clone().unwrap_or_default();
            route.attr.description = Some(format!("{base}{extra}").into());
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
