//! Built-in icons, so an agent names an icon instead of drawing one:
//! [Lucide](https://lucide.dev) (outline, ISC) and
//! [Font Awesome Free](https://fontawesome.com) (solid, regular and brand
//! icons, CC BY 4.0). Licenses sit next to the sets in `icons/`.
//!
//! Each set is one TSV file: a `#` line with the root SVG attributes, then
//! `name`, `viewBox` and the SVG body per line, drawn with `currentColor`.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::scene::{Color, IconSet};

/// Icons are 24 px tall when their box doesn't say otherwise.
pub const DEFAULT_SIZE: f32 = 24.0;

struct Set {
    attrs: &'static str,
    icons: HashMap<&'static str, (&'static str, &'static str)>,
}

fn parse(src: &'static str) -> Set {
    let mut lines = src.lines();
    let attrs = lines
        .next()
        .and_then(|l| l.strip_prefix('#'))
        .unwrap_or_default();
    let icons = lines
        .filter_map(|l| {
            let mut cols = l.splitn(3, '\t');
            Some((cols.next()?, (cols.next()?, cols.next()?)))
        })
        .collect();
    Set { attrs, icons }
}

fn set(s: IconSet) -> &'static Set {
    static SETS: [OnceLock<Set>; 4] = [const { OnceLock::new() }; 4];
    let (i, src) = match s {
        IconSet::Lucide => (0, include_str!("../icons/lucide.tsv")),
        IconSet::Solid => (1, include_str!("../icons/fa-solid.tsv")),
        IconSet::Regular => (2, include_str!("../icons/fa-regular.tsv")),
        IconSet::Brands => (3, include_str!("../icons/fa-brands.tsv")),
    };
    SETS[i].get_or_init(|| parse(src))
}

/// Width ÷ height of the icon's artwork.
pub fn aspect(s: IconSet, name: &str) -> Option<f32> {
    let (vb, _) = set(s).icons.get(name)?;
    let mut n = vb.split(' ').skip(2).filter_map(|v| v.parse::<f32>().ok());
    let (w, h) = (n.next()?, n.next()?);
    (h > 0.0).then(|| w / h)
}

/// A standalone SVG of the icon in `color`; `stroke_width` overrides a
/// stroked set's line width (in the icon's own units).
pub fn svg(s: IconSet, name: &str, color: Color, stroke_width: Option<f32>) -> Option<String> {
    let set = set(s);
    let (vb, body) = set.icons.get(name)?;
    let rgb = format!("#{:06X}", color.0 & 0x00FF_FFFF);
    let mut attrs = set.attrs.replace("currentColor", &rgb);
    if let Some(w) = stroke_width {
        attrs = attrs.replace("stroke-width=\"2\"", &format!("stroke-width=\"{w}\""));
    }
    let alpha = f64::from(color.0 >> 24) / 255.0;
    Some(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{vb}\" {attrs}><g opacity=\"{alpha}\">{body}</g></svg>"
    ))
}

/// `Ok` when the set has `name`; otherwise an error naming close matches.
pub fn check(s: IconSet, name: &str) -> Result<(), String> {
    let icons = &set(s).icons;
    if icons.contains_key(name) {
        return Ok(());
    }
    // ponytail: substring match on each word; fuzzy search if agents miss often.
    let mut close: Vec<&str> = icons
        .keys()
        .filter(|k| name.split('-').any(|w| w.len() > 2 && k.contains(w)))
        .copied()
        .collect();
    close.sort_unstable_by_key(|k| (k.len(), *k));
    close.truncate(8);
    Err(if close.is_empty() {
        format!("no {s} icon {name}")
    } else {
        format!("no {s} icon {name}; close: {}", close.join(", "))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_set_loads_with_known_icons() {
        for (s, name) in [
            (IconSet::Lucide, "mail"),
            (IconSet::Solid, "envelope"),
            (IconSet::Regular, "envelope"),
            (IconSet::Brands, "instagram"),
        ] {
            assert!(set(s).icons.len() > 100, "{s}");
            assert!(check(s, name).is_ok(), "{s} {name}");
            assert!(aspect(s, name).is_some_and(|a| a > 0.0), "{s} {name}");
        }
    }

    #[test]
    fn svgs_take_the_color_and_stroke_width() {
        let svg = svg(IconSet::Lucide, "mail", Color(0x80FF_0000), Some(3.0)).unwrap();
        assert!(svg.contains("stroke=\"#FF0000\""), "{svg}");
        assert!(svg.contains("stroke-width=\"3\""), "{svg}");
        assert!(svg.contains("opacity=\"0.50"), "{svg}");
        assert!(!svg.contains("currentColor"), "{svg}");
        let solid = super::svg(IconSet::Solid, "envelope", Color(0xFFFF_FFFF), None).unwrap();
        assert!(solid.contains("fill=\"#FFFFFF\""), "{solid}");
    }

    #[test]
    fn unknown_names_suggest_close_ones() {
        let err = check(IconSet::Lucide, "mailbox-open").unwrap_err();
        assert!(err.contains("close:") && err.contains("mailbox"), "{err}");
        assert_eq!(
            check(IconSet::Solid, "zzz").unwrap_err(),
            "no solid icon zzz"
        );
    }
}
