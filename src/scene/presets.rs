//! Output sizes by name: `"instagram-story"` or `"1200x628"` instead of
//! `{id, width, height}`. Expanded when a scene is created, so stored scenes
//! always hold explicit sizes.

use serde::Deserialize;

use super::Size;

/// A size as `scene_create` takes it: a full size, a preset slug, or `"WxH"`.
#[derive(Debug, Clone, PartialEq, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum SizeSpec {
    /// A preset slug (see [`PRESETS`]) or `"1200x628"`; the string is the id.
    Named(String),
    /// Every field spelled out.
    Full(Size),
}

/// Named sizes: `(slug, width, height, safe [top, right, bottom, left])`.
pub const PRESETS: &[(&str, f32, f32, [f32; 4])] = &[
    ("instagram-portrait", 1080.0, 1350.0, [0.0; 4]),
    ("instagram-square", 1080.0, 1080.0, [0.0; 4]),
    ("instagram-story", 1080.0, 1920.0, [250.0, 0.0, 340.0, 0.0]),
    ("facebook-feed", 1200.0, 628.0, [0.0; 4]),
    ("linkedin-post", 1200.0, 627.0, [0.0; 4]),
    ("x-post", 1600.0, 900.0, [0.0; 4]),
    ("youtube-thumbnail", 1280.0, 720.0, [0.0; 4]),
    ("iab-medium-rectangle", 300.0, 250.0, [0.0; 4]),
    ("iab-leaderboard", 728.0, 90.0, [0.0; 4]),
    ("iab-skyscraper", 160.0, 600.0, [0.0; 4]),
    ("iab-half-page", 300.0, 600.0, [0.0; 4]),
    ("a4-portrait", 2480.0, 3508.0, [0.0; 4]),
];

/// Print presets and their resolution, dots per inch: in a PDF their pixels
/// print at this size (A4 is 2480×3508 px at 300 dpi, a 595×842 pt page).
const PRINT: &[(&str, f32)] = &[("a4-portrait", 300.0)];

/// Points per pixel in a PDF of size `id`: 72/dpi for a print preset, else
/// 1 (a pixel is a point).
pub fn pdf_points_per_px(id: &str) -> f32 {
    PRINT
        .iter()
        .find(|p| p.0 == id)
        .map_or(1.0, |&(_, dpi)| 72.0 / dpi)
}

impl SizeSpec {
    /// The explicit size.
    ///
    /// # Errors
    /// An unknown slug or a malformed `"WxH"`, with the known slugs listed.
    pub fn resolve(self) -> Result<Size, String> {
        let name = match self {
            SizeSpec::Full(s) => return Ok(s),
            SizeSpec::Named(n) => n,
        };
        let size = |width, height, safe| Size {
            id: name.clone(),
            width,
            height,
            scale: 1.0,
            safe,
        };
        if let Some(&(_, w, h, safe)) = PRESETS.iter().find(|p| p.0 == name) {
            return Ok(size(w, h, safe));
        }
        if let Some((w, h)) = name.split_once('x')
            && let (Ok(w), Ok(h)) = (w.parse::<f32>(), h.parse::<f32>())
        {
            return Ok(size(w, h, [0.0; 4]));
        }
        let known: Vec<&str> = PRESETS.iter().map(|p| p.0).collect();
        Err(format!(
            "unknown size {name}; use \"WxH\" or one of {}",
            known.join(", ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::SizeSpec;
    use serde_json::json;

    fn resolve(v: serde_json::Value) -> Result<crate::scene::Size, String> {
        serde_json::from_value::<SizeSpec>(v).unwrap().resolve()
    }

    #[test]
    fn presets_and_wxh_expand_to_full_sizes() {
        let s = resolve(json!("instagram-story")).unwrap();
        assert_eq!(
            (s.id.as_str(), s.width, s.height, s.safe),
            ("instagram-story", 1080.0, 1920.0, [250.0, 0.0, 340.0, 0.0])
        );
        let s = resolve(json!("1200x628")).unwrap();
        assert_eq!(
            (s.id.as_str(), s.width, s.height),
            ("1200x628", 1200.0, 628.0)
        );
        let s = resolve(json!({"id": "sky", "width": 160, "height": 600, "scale": 0.3})).unwrap();
        assert_eq!((s.id.as_str(), s.scale), ("sky", 0.3));
    }

    #[test]
    fn unknown_names_list_the_presets() {
        let e = resolve(json!("tiktok")).unwrap_err();
        assert!(
            e.contains("unknown size tiktok") && e.contains("instagram-story"),
            "{e}"
        );
    }
}
