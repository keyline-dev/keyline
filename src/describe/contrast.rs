//! The `warn contrast` advisory: WCAG contrast of text against what is
//! drawn behind it.

use skia_safe::Pixmap;

use crate::layout::{Placed, Rect};
use crate::scene::{Color, Kind};

/// WCAG contrast of the text's colors, as drawn (their alpha and `opacity`
/// blended over the backdrop), against the average luminance behind its
/// ink. Returns `(worst ratio, required)` when below the requirement: 3:1
/// for large text (≥ 24 px, or ≥ 18.66 px bold), else 4.5:1.
// ponytail: averages the backdrop; text over a busy photo can pass on
// average yet fail in places. Check per-glyph region if that bites.
pub(super) fn contrast(
    p: &Placed,
    ink: Rect,
    backdrop: &Pixmap,
    opacity: f32,
) -> Option<(f32, f32)> {
    let (_, fit) = p.text.as_ref()?;
    let Kind::Text {
        color,
        ranges,
        weight,
        fill,
        gradient,
        ..
    } = &p.layer.kind
    else {
        return None;
    };
    if fill.is_some() || gradient.is_some() {
        return None; // letters painted with an image or gradient: no single color to judge
    }
    const GRID: i32 = 16;
    let colors: Vec<Color> = std::iter::once(*color)
        .chain(ranges.iter().map(|r| r.color))
        .collect();
    let (bw, bh) = (backdrop.width() - 1, backdrop.height() - 1);
    let mut bg = 0.0;
    let mut fg = vec![0.0; colors.len()];
    for gx in 0..GRID {
        for gy in 0..GRID {
            let x = (ink.x + ink.w * (gx as f32 + 0.5) / GRID as f32) as i32;
            let y = (ink.y + ink.h * (gy as f32 + 0.5) / GRID as f32) as i32;
            let c = backdrop.get_color((x.clamp(0, bw), y.clamp(0, bh)));
            let under = [c.r(), c.g(), c.b()];
            bg += luminance(under);
            for (sum, text) in fg.iter_mut().zip(&colors) {
                *sum += luminance(over(*text, opacity, under));
            }
        }
    }
    let samples = (GRID * GRID) as f32;
    let worst = fg
        .iter()
        .map(|f| ratio(f / samples, bg / samples))
        .fold(f32::MAX, f32::min);
    let large = fit.font_size >= 24.0 || (fit.font_size >= 18.66 && *weight >= 700);
    let min = if large { 3.0 } else { 4.5 };
    (worst < min).then_some((worst, min))
}

/// `text` drawn at its alpha × `opacity` over `under`, as sRGB.
fn over(text: Color, opacity: f32, under: [u8; 3]) -> [u8; 3] {
    let [a, r, g, b] = text.0.to_be_bytes();
    let a = f32::from(a) / 255.0 * opacity;
    let mix = |t: u8, u: u8| (f32::from(t) * a + f32::from(u) * (1.0 - a)).round() as u8;
    [mix(r, under[0]), mix(g, under[1]), mix(b, under[2])]
}

/// WCAG relative luminance of an sRGB color.
fn luminance([r, g, b]: [u8; 3]) -> f32 {
    let lin = |v: u8| {
        let c = f32::from(v) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

fn ratio(a: f32, b: f32) -> f32 {
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
