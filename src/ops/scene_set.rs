//! `layer_update` on the scene itself: `{target: {scene: true}, set: {…}}`
//! changes its background, sizes, master size or timing after creation.

use serde_json::{Map, Value};

use crate::scene::{Scene, SizeSpec};

/// Applies `set` to the scene's own fields; `null` resets one.
///
/// # Errors
/// A field the scene doesn't take, or a value that doesn't parse.
pub(super) fn set_scene(scene: &mut Scene, set: &Map<String, Value>) -> Result<(), String> {
    fn read<T: serde::de::DeserializeOwned>(k: &str, v: &Value) -> Result<T, String> {
        serde_json::from_value(v.clone()).map_err(|e| format!("{k}: {e}"))
    }
    for (k, v) in set {
        let null = v.is_null();
        match k.as_str() {
            "background" if null => scene.background = crate::scene::white(),
            "background" => scene.background = read(k, v)?,
            "sizes" => {
                scene.sizes = read::<Vec<SizeSpec>>(k, v)?
                    .into_iter()
                    .map(SizeSpec::resolve)
                    .collect::<Result<_, _>>()?;
            }
            "width" => scene.width = read(k, v)?,
            "height" => scene.height = read(k, v)?,
            "duration" => scene.duration = read(k, v)?,
            "fps" if null => scene.fps = crate::scene::thirty(),
            "fps" => scene.fps = read(k, v)?,
            "loop" => scene.looping = !null && read::<bool>(k, v)?,
            other => {
                return Err(format!(
                    "the scene takes background, sizes, width, height, duration, fps and loop, not {other}"
                ));
            }
        }
    }
    Ok(())
}
