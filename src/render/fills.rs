//! Paints to Skia: colors, gradients (linear, radial, conic), images with
//! color adjustments, built-in patterns and film grain.

use anyhow::Result;
use skia_safe::{
    Matrix, PaintStyle, PictureRecorder, Point, Shader, TileMode, color_filters, gradient, shaders,
};

use super::Ctx;
use super::paint::{sk_blend, sk_color, sk_rect};
use crate::layout::Rect;
use crate::scene::{
    Adjust, Color, Gradient, GradientKind, NoisePaint, Paint, PatternKind, PatternPaint,
};

impl Ctx<'_> {
    /// The Skia paint for one fill over box `bx`; `k` scales px lengths.
    pub(super) fn fill(
        &mut self,
        paint: &Paint,
        bx: Rect,
        k: f32,
    ) -> Result<Option<skia_safe::Paint>> {
        let mut p = match paint {
            Paint::Solid(s) => solid(s.color),
            Paint::Gradient(g) => match gradient_shader(&g.gradient, sk_rect(bx)) {
                Some(sh) => shaded(sh),
                None => return Ok(None),
            },
            Paint::Image(i) => {
                let mut p = self.image_paint(
                    &i.image,
                    bx,
                    i.fit,
                    i.crop.as_ref(),
                    i.tile_scale * k,
                    i.focus,
                )?;
                if let Some(cf) = adjust_filter(&i.adjust) {
                    p.set_color_filter(cf);
                }
                p
            }
            Paint::Pattern(pt) => shaded(pattern_shader(pt, sk_rect(bx), k)),
            Paint::Noise(n) => noise_paint(n, k),
        };
        let c = paint.common();
        if c.opacity < 1.0 {
            p.set_alpha_f(p.alpha_f() * c.opacity.clamp(0.0, 1.0));
        }
        p.set_blend_mode(sk_blend(c.blend_mode));
        Ok(Some(p))
    }
}

fn solid(c: Color) -> skia_safe::Paint {
    let mut p = skia_safe::Paint::default();
    p.set_anti_alias(true);
    p.set_color(sk_color(c));
    p
}

fn shaded(sh: Shader) -> skia_safe::Paint {
    let mut p = skia_safe::Paint::default();
    p.set_anti_alias(true);
    p.set_shader(sh);
    p
}

/// A gradient's shader over box `r`.
pub(super) fn gradient_shader(g: &Gradient, r: skia_safe::Rect) -> Option<Shader> {
    let colors: Vec<skia_safe::Color4f> =
        g.stops.iter().map(|s| sk_color(s.color).into()).collect();
    let pos: Vec<f32> = g.stops.iter().map(|s| s.at).collect();
    let spec = gradient::Gradient::new(
        gradient::Colors::new(&colors, Some(&pos), TileMode::Clamp, None),
        gradient::Interpolation::default(),
    );
    let at = |[x, y]: [f32; 2]| Point::new(r.left + x * r.width(), r.top + y * r.height());
    match g.kind {
        GradientKind::Linear => {
            let (from, to) = g.line();
            gradient::shaders::linear_gradient((at(from), at(to)), &spec, None)
        }
        GradientKind::Radial => {
            // A unit circle stretched to the radius on each axis: an ellipse
            // that follows the box, as in design tools.
            let c = at(g.center);
            let m = Matrix::scale((
                g.radius[0] * r.width().max(1e-3),
                g.radius[1] * r.height().max(1e-3),
            ))
            .post_translate((c.x, c.y))
            .to_owned();
            gradient::shaders::radial_gradient(((0.0, 0.0), 1.0), &spec, &m)
        }
        GradientKind::Conic => {
            // Skia sweeps from 3 o'clock; start at 12, plus `angle`.
            let c = at(g.center);
            let m = Matrix::rotate_deg_pivot(-90.0 + g.angle.unwrap_or(0.0), c);
            gradient::shaders::sweep_gradient(c, (0.0, 360.0), &spec, &m)
        }
    }
}

/// One 4 × 5 row-major color matrix for every adjustment, applied in the
/// order CSS filters list them; `None` when nothing is adjusted.
pub(super) fn adjust_filter(a: &Adjust) -> Option<skia_safe::ColorFilter> {
    if a.is_none() {
        return None;
    }
    let mut m = IDENTITY;
    let (lr, lg, lb) = (0.2126, 0.7152, 0.0722);
    if a.brightness != 0.0 {
        let b = a.brightness;
        m = then(
            m,
            [
                1.0, 0.0, 0.0, 0.0, b, 0.0, 1.0, 0.0, 0.0, b, 0.0, 0.0, 1.0, 0.0, b, 0.0, 0.0, 0.0,
                1.0, 0.0,
            ],
        );
    }
    if a.contrast != 0.0 {
        let c = 1.0 + a.contrast;
        let t = 0.5 * (1.0 - c);
        m = then(
            m,
            [
                c, 0.0, 0.0, 0.0, t, 0.0, c, 0.0, 0.0, t, 0.0, 0.0, c, 0.0, t, 0.0, 0.0, 0.0, 1.0,
                0.0,
            ],
        );
    }
    let saturation = |s: f32| {
        [
            lr + (1.0 - lr) * s,
            lg * (1.0 - s),
            lb * (1.0 - s),
            0.0,
            0.0,
            lr * (1.0 - s),
            lg + (1.0 - lg) * s,
            lb * (1.0 - s),
            0.0,
            0.0,
            lr * (1.0 - s),
            lg * (1.0 - s),
            lb + (1.0 - lb) * s,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            0.0,
        ]
    };
    if a.saturate != 0.0 {
        m = then(m, saturation(1.0 + a.saturate));
    }
    if a.grayscale != 0.0 {
        m = then(m, saturation(1.0 - a.grayscale.clamp(0.0, 1.0)));
    }
    if a.sepia != 0.0 {
        let s = a.sepia.clamp(0.0, 1.0);
        let sep = [
            0.393, 0.769, 0.189, 0.0, 0.0, 0.349, 0.686, 0.168, 0.0, 0.0, 0.272, 0.534, 0.131, 0.0,
            0.0, 0.0, 0.0, 0.0, 1.0, 0.0,
        ];
        m = then(m, lerp(IDENTITY, sep, s));
    }
    if a.hue != 0.0 {
        m = then(m, hue(a.hue));
    }
    if let Some([dark, light]) = a.duotone {
        // Brightness picks a color between the two: a straight line in RGB.
        let ch = |c: Color, shift: u32| ((c.0 >> shift) & 0xFF) as f32 / 255.0;
        let row = |shift| {
            let (d, l) = (ch(dark, shift), ch(light, shift));
            [(l - d) * lr, (l - d) * lg, (l - d) * lb, 0.0, d]
        };
        let (r, g, b) = (row(16), row(8), row(0));
        m = then(
            m,
            [
                r[0], r[1], r[2], r[3], r[4], g[0], g[1], g[2], g[3], g[4], b[0], b[1], b[2], b[3],
                b[4], 0.0, 0.0, 0.0, 1.0, 0.0,
            ],
        );
    }
    if let Some(t) = a.tint {
        let ch = |shift: u32| ((t.0 >> shift) & 0xFF) as f32 / 255.0;
        m = then(
            m,
            [
                0.0,
                0.0,
                0.0,
                0.0,
                ch(16),
                0.0,
                0.0,
                0.0,
                0.0,
                ch(8),
                0.0,
                0.0,
                0.0,
                0.0,
                ch(0),
                0.0,
                0.0,
                0.0,
                1.0,
                0.0,
            ],
        );
    }
    Some(color_filters::matrix_row_major(&m, None))
}

type ColorMatrix = [f32; 20];

const IDENTITY: ColorMatrix = [
    1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    0.0,
];

/// `a` then `b`: the matrix that applies `a` first.
fn then(a: ColorMatrix, b: ColorMatrix) -> ColorMatrix {
    let mut out = [0.0; 20];
    for row in 0..4 {
        for col in 0..5 {
            let mut v: f32 = (0..4).map(|i| b[row * 5 + i] * a[i * 5 + col]).sum();
            if col == 4 {
                v += b[row * 5 + 4];
            }
            out[row * 5 + col] = v;
        }
    }
    out
}

fn lerp(a: ColorMatrix, b: ColorMatrix, t: f32) -> ColorMatrix {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

/// The CSS `hue-rotate` matrix.
fn hue(deg: f32) -> ColorMatrix {
    let (s, c) = deg.to_radians().sin_cos();
    [
        0.213 + c * 0.787 - s * 0.213,
        0.715 - c * 0.715 - s * 0.715,
        0.072 - c * 0.072 + s * 0.928,
        0.0,
        0.0,
        0.213 - c * 0.213 + s * 0.143,
        0.715 + c * 0.285 + s * 0.140,
        0.072 - c * 0.072 - s * 0.283,
        0.0,
        0.0,
        0.213 - c * 0.213 - s * 0.787,
        0.715 - c * 0.715 + s * 0.715,
        0.072 + c * 0.928 + s * 0.072,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
    ]
}

/// A repeating pattern: one tile recorded once, repeated as a shader.
fn pattern_shader(pt: &PatternPaint, r: skia_safe::Rect, k: f32) -> Shader {
    if pt.pattern == PatternKind::Rays {
        return rays(pt, r);
    }
    let s = (pt.size * k).max(1.0);
    let tile = skia_safe::Rect::from_wh(s, s);
    let mut rec = PictureRecorder::new();
    let canvas = rec.begin_recording(tile, false);
    let mut p = solid(pt.color);
    let line = (s / 12.0).max(k);
    match pt.pattern {
        PatternKind::Dots => {
            canvas.draw_circle((s / 2.0, s / 2.0), s * 0.18, &p);
        }
        PatternKind::Stripes => {
            canvas.draw_rect(skia_safe::Rect::from_wh(s / 2.0, s), &p);
        }
        PatternKind::Grid => {
            canvas.draw_rect(skia_safe::Rect::from_wh(s, line), &p);
            canvas.draw_rect(skia_safe::Rect::from_wh(line, s), &p);
        }
        PatternKind::Checker => {
            canvas.draw_rect(skia_safe::Rect::from_wh(s / 2.0, s / 2.0), &p);
            canvas.draw_rect(
                skia_safe::Rect::from_xywh(s / 2.0, s / 2.0, s / 2.0, s / 2.0),
                &p,
            );
        }
        PatternKind::Zigzag => {
            p.set_style(PaintStyle::Stroke);
            p.set_stroke_width(line);
            let mut path = skia_safe::PathBuilder::new();
            path.move_to((0.0, s * 0.75))
                .line_to((s / 2.0, s * 0.25))
                .line_to((s, s * 0.75));
            canvas.draw_path(&path.detach(), &p);
        }
        PatternKind::Rays => {}
    }
    let Some(picture) = rec.finish_recording_as_picture(None) else {
        return shaders::color(skia_safe::Color::TRANSPARENT);
    };
    let m = Matrix::rotate_deg_pivot(pt.angle, Point::new(r.left, r.top))
        .pre_translate((r.left, r.top))
        .to_owned();
    picture.to_shader(
        (TileMode::Repeat, TileMode::Repeat),
        skia_safe::FilterMode::Linear,
        &m,
        &tile,
    )
}

/// Sunburst rays: `size` rays from the center, color and clear alternating.
fn rays(pt: &PatternPaint, r: skia_safe::Rect) -> Shader {
    let n = pt.size.round().clamp(2.0, 180.0) as usize;
    let clear = skia_safe::Color4f::from(skia_safe::Color::TRANSPARENT);
    let on: skia_safe::Color4f = sk_color(pt.color).into();
    let mut colors = Vec::with_capacity(4 * n);
    let mut pos = Vec::with_capacity(4 * n);
    for i in 0..n {
        let (a, b, c) = (
            i as f32 / n as f32,
            (i as f32 + 0.5) / n as f32,
            (i as f32 + 1.0) / n as f32,
        );
        colors.extend([on, on, clear, clear]);
        pos.extend([a, b, b, c]);
    }
    let spec = gradient::Gradient::new(
        gradient::Colors::new(&colors, Some(&pos), TileMode::Clamp, None),
        gradient::Interpolation::default(),
    );
    let c = r.center();
    let m = Matrix::rotate_deg_pivot(pt.angle - 90.0, c);
    gradient::shaders::sweep_gradient(c, (0.0, 360.0), &spec, &m)
        .unwrap_or_else(|| shaders::color(skia_safe::Color::TRANSPARENT))
}

/// Gray grain at `noise` strength; same `seed`, same grain.
fn noise_paint(n: &NoisePaint, k: f32) -> skia_safe::Paint {
    let f = 0.9 / (n.size * k).max(0.25);
    let sh = shaders::fractal_noise((f, f), 2, n.seed as f32, None)
        .unwrap_or_else(|| shaders::color(skia_safe::Color::TRANSPARENT));
    let mut p = shaded(sh);
    // Gray from the noise's own brightness, at a fixed alpha.
    let a = n.noise.clamp(0.0, 1.0);
    let g = 1.0 / 3.0;
    p.set_color_filter(color_filters::matrix_row_major(
        &[
            g, g, g, 0.0, 0.0, g, g, g, 0.0, 0.0, g, g, g, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, a,
        ],
        None,
    ));
    p
}

#[cfg(test)]
mod tests {
    use super::{IDENTITY, hue, then};

    #[test]
    fn matrices_compose_in_order() {
        let brighten = [
            1.0, 0.0, 0.0, 0.0, 0.1, 0.0, 1.0, 0.0, 0.0, 0.1, 0.0, 0.0, 1.0, 0.0, 0.1, 0.0, 0.0,
            0.0, 1.0, 0.0,
        ];
        let double = [
            2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0,
        ];
        // Brighten then double: (x + 0.1) × 2 = 2x + 0.2.
        let m = then(brighten, double);
        assert_eq!((m[0], m[4]), (2.0, 0.2));
        assert_eq!(then(IDENTITY, double), double);
        let h = hue(0.0);
        assert!(h.iter().zip(IDENTITY).all(|(a, b)| (a - b).abs() < 1e-3));
    }
}
