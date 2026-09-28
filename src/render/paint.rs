//! Conversions from scene types to Skia: blend modes, colors, rects.

use crate::layout::Rect;
use crate::scene::{BlendMode, Color};

pub(super) fn sk_blend(m: BlendMode) -> skia_safe::BlendMode {
    use skia_safe::BlendMode as B;
    match m {
        BlendMode::Normal => B::SrcOver,
        BlendMode::Multiply => B::Multiply,
        BlendMode::Screen => B::Screen,
        BlendMode::Overlay => B::Overlay,
        BlendMode::Darken => B::Darken,
        BlendMode::Lighten => B::Lighten,
        BlendMode::ColorDodge => B::ColorDodge,
        BlendMode::ColorBurn => B::ColorBurn,
        BlendMode::HardLight => B::HardLight,
        BlendMode::SoftLight => B::SoftLight,
        BlendMode::Difference => B::Difference,
        BlendMode::Exclusion => B::Exclusion,
        BlendMode::Hue => B::Hue,
        BlendMode::Saturation => B::Saturation,
        BlendMode::Color => B::Color,
        BlendMode::Luminosity => B::Luminosity,
    }
}

pub(super) fn sk_color(c: Color) -> skia_safe::Color {
    skia_safe::Color::new(c.0)
}

pub(super) fn sk_rect(r: Rect) -> skia_safe::Rect {
    skia_safe::Rect::from_xywh(r.x, r.y, r.w, r.h)
}
