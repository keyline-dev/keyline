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
        let sized = scene.for_size(&size.id);
        let placed = layout(&sized, size);
        let mut min: Option<(f32, &str)> = None;
        walk(&placed, &mut |p| match (&p.text, &p.layer.kind) {
            (Some((_, fit)), _) if min.is_none_or(|(px, _)| fit.font_size < px) => {
                min = Some((fit.font_size, &p.layer.id));
            }
            (
                _,
                Kind::Image {
                    asset,
                    fit,
                    crop,
                    tile_scale,
                    ..
                },
            ) => {
                if let Some(a) = scene.assets.get(asset).filter(|a| !a.svg) {
                    let up = image_scale(
                        p.rect,
                        a.width,
                        a.height,
                        *fit,
                        crop.as_ref(),
                        tile_scale * p.k,
                    );
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
                        let _ = write!(out, " (max {})", n(max));
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
    go(&mut out, &layout(&scene.for_size(&size.id), size));
    out
}
