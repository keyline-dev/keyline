//! Facts with no threshold (smallest text, upscaling) and the drawn-text
//! report `render` returns.

use std::fmt::Write;

use super::n;
use crate::layout::{Placed, layout};
use crate::render::image_scale;
use crate::scene::{Kind, Scene, Size};
use crate::text::Text;

/// Facts for the agent to judge against its medium, one line:
/// `smallest text: portrait 30px, sky 8.4px (footer); upscaled: wide photo 1.3x`.
/// No thresholds: 8 px may be fine print on a banner and unreadable on a
/// billboard.
pub fn facts(scene: &Scene) -> String {
    fn walk<'a>(placed: &'a [Placed<'a>], f: &mut impl FnMut(&'a Placed<'a>)) {
        for p in placed {
            f(p);
            walk(&p.children, f);
        }
    }
    let mut smallest = Vec::new();
    let mut upscaled = Vec::new();
    for size in &scene.sizes {
        let sized = scene.for_size(size);
        let placed = layout(&sized, size);
        let mut min: Option<(f32, &str)> = None;
        walk(&placed, &mut |p| match (&p.text, &p.layer.kind) {
            (Some((_, fit)), _) if min.is_none_or(|(px, _)| fit.font_size < px) => {
                min = Some((fit.font_size, &p.layer.id));
            }
            (_, kind) if let Some((asset, fit, crop, tile_scale)) = kind.picture() => {
                if let Some(a) = scene.assets.get(asset).filter(|a| !a.svg) {
                    let up = image_scale(p.rect, a.width, a.height, fit, crop, tile_scale * p.k);
                    if up > 1.005 {
                        upscaled.push(format!("{} {} {up:.1}x", size.id, p.layer.id));
                    }
                }
            }
            _ => {}
        });
        if let Some((px, id)) = min {
            smallest.push(format!("{} {}px ({id})", size.id, px_label(px)));
        }
    }
    let mut out = String::new();
    if !smallest.is_empty() {
        let _ = write!(out, "smallest text: {}", smallest.join(", "));
    }
    if !upscaled.is_empty() {
        let sep = if out.is_empty() { "" } else { "; " };
        let _ = write!(out, "{sep}upscaled: {}", upscaled.join(", "));
    }
    out
}

/// Whole pixels, or one decimal below 12 px where a fraction matters.
fn px_label(px: f32) -> String {
    if px < 12.0 {
        format!("{px:.1}").trim_end_matches(".0").to_owned()
    } else {
        n(px).to_string()
    }
}

/// A hint per size at least twice the master's width with no `scale`
/// (a print preset for a screen-sized master): laid out in master px, the
/// design would sit small in its corner.
pub fn scale_hints(scene: &Scene) -> Vec<String> {
    scene
        .sizes
        .iter()
        .filter(|s| (s.scale - 1.0).abs() < 1e-6 && s.width >= 2.0 * scene.width)
        .map(|s| {
            let k = (s.width / scene.width * 100.0).round() / 100.0;
            format!(
                "hint: {} is {k}× the master's width; give it \"scale\": {k} to keep the layout's proportions",
                s.id
            )
        })
        .collect()
}

/// A hint per image drawn as a halftone over a dark background (its
/// closest frame's plain fill, else the canvas): the dots are black, so
/// the picture all but vanishes.
pub fn halftone_hints(scene: &Scene) -> Vec<String> {
    fn go(layers: &[crate::scene::Layer], under: crate::scene::Color, out: &mut Vec<String>) {
        for l in layers {
            if let Kind::Image { adjust, .. } = &l.kind
                && adjust.halftone > 0.0
            {
                let [_, r, g, b] = under.0.to_be_bytes();
                if super::contrast::luminance([r, g, b]) < 0.2 {
                    out.push(format!(
                        "hint: {} halftone draws black dots; on {under} it barely shows (a light fill behind it, or duotone instead)",
                        l.id
                    ));
                }
            }
            if let Some(children) = l.kind.children() {
                let fill = l
                    .look
                    .fills
                    .as_ref()
                    .and_then(|f| match f.as_slice().last() {
                        Some(crate::scene::Paint::Solid(s)) if s.color.0 >> 24 == 0xFF => {
                            Some(s.color)
                        }
                        _ => None,
                    });
                go(children, fill.unwrap_or(under), out);
            }
        }
    }
    let mut out = Vec::new();
    go(&scene.layers, scene.background, &mut out);
    out
}

/// The lowest resolution of `scene`'s photos on a page `pt` points per
/// px, dots per inch: what print cares about. `None` without one.
pub fn image_dpi(scene: &Scene, size: &Size, pt: f32) -> Option<f32> {
    fn walk(scene: &Scene, placed: &[Placed], pt: f32, min: &mut Option<f32>) {
        for p in placed {
            if let Some((asset, fit, crop, tile_scale)) = p.layer.kind.picture()
                && let Some(a) = scene.assets.get(asset).filter(|a| !a.svg)
            {
                let up = image_scale(p.rect, a.width, a.height, fit, crop, tile_scale * p.k);
                let dpi = 72.0 / (pt * up);
                if min.is_none_or(|m| dpi < m) {
                    *min = Some(dpi);
                }
            }
            walk(scene, &p.children, pt, min);
        }
    }
    let sized = scene.for_size(size);
    let mut min = None;
    walk(&sized, &layout(&sized, size), pt, &mut min);
    min
}

/// The fonts `scene`'s texts are drawn in, one line for `render`:
/// `fonts: Bricolage Grotesque 800, Inter 400/600`. A weight the family
/// has no face for shows the one drawn (`Fraunces 900→700`); a family
/// with no face at all, `(fallback)`.
pub fn fonts_line(scene: &Scene) -> String {
    use std::collections::BTreeMap;
    let mut used: BTreeMap<String, std::collections::BTreeSet<(u16, bool)>> = BTreeMap::new();
    scene.walk(&mut |l| {
        if let Kind::Text {
            text,
            font_family,
            weight,
            more,
            ..
        } = &l.kind
        {
            let (_, spans) = crate::text::markup::parse(text);
            let mut add = |family: &str, w: u16, italic: bool| {
                used.entry(family.to_owned())
                    .or_default()
                    .insert((w, italic));
            };
            add(font_family, *weight, more.italic);
            for s in spans {
                add(
                    s.font_family.as_deref().unwrap_or(font_family),
                    s.weight.unwrap_or(*weight),
                    s.italic.unwrap_or(more.italic),
                );
            }
        }
    });
    let families: Vec<String> = used
        .iter()
        .map(|(family, styles)| {
            let mut weights: Vec<String> = Vec::new();
            for &(w, italic) in styles {
                let label = match crate::text::drawn_weight(family, w, italic) {
                    None => return format!("{family} (fallback)"),
                    Some(d) if d != w => format!("{w}→{d}"),
                    Some(_) => w.to_string(),
                };
                if !weights.contains(&label) {
                    weights.push(label);
                }
            }
            format!("{family} {}", weights.join("/"))
        })
        .collect();
    if families.is_empty() {
        String::new()
    } else {
        format!("fonts: {}", families.join(", "))
    }
}

/// For `render`: every text that wrapped, shrank or was cut, as actually
/// drawn, so the agent can check wording and breaks without an image.
/// Texts drawn on one line at their requested size are left out.
///
/// ```text
///  headline 68px: "Proven RESULTS for" / "WILLOWMERE Families"
///  cta 44px (max 56): "VOTE BY MAIL"
/// ```
pub fn text_report(scene: &Scene, size: &Size) -> String {
    fn go(out: &mut String, placed: &[Placed]) {
        for p in placed {
            if let (Some((para, fit)), Some(t), Kind::Text { font_size, .. }) =
                (&p.text, Text::of(p.layer, p.k), &p.layer.kind)
            {
                let max = font_size * p.k;
                let shrunk = n(max) != n(fit.font_size);
                if fit.lines > 1 || shrunk || fit.truncated {
                    let _ = write!(out, " {} {}px", p.layer.id, n(fit.font_size));
                    if shrunk {
                        let _ = write!(out, " {}", super::shrunk(max, fit));
                    }
                    let lines: Vec<String> = t
                        .drawn_lines(para, fit)
                        .iter()
                        .map(|l| format!("{l:?}"))
                        .collect();
                    let _ = writeln!(out, ": {}", lines.join(" / "));
                }
            }
            go(out, &p.children);
        }
    }
    let mut out = String::new();
    go(&mut out, &layout(&scene.for_size(size), size));
    out
}
