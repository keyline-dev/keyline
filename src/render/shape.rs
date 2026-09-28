//! Layer outlines as Skia shapes: rects (per-corner or capsule radii),
//! ellipses and arcs, polygons and stars, and SVG paths fitted to the box.

use skia_safe::{ClipOp, Path, PathBuilder, PathEffect, Point, RRect, rrect::Corner};

use crate::layout::Placed;
use crate::scene::{Arc, FillRule, FitPath, Kind};

/// A layer's outline.
pub(super) enum Shape {
    /// A rectangle, rounded or not.
    Rect(RRect),
    /// An ellipse inscribed in the box.
    Oval(skia_safe::Rect),
    /// Any path.
    Path(Path),
    /// A straight line, stroked only.
    Line(Point, Point),
}

impl Shape {
    /// The outline as a path.
    pub fn path(&self) -> Path {
        match self {
            Shape::Rect(rr) => Path::rrect(rr, None),
            Shape::Oval(r) => Path::oval(r, None),
            Shape::Path(p) => p.clone(),
            Shape::Line(a, b) => {
                let mut p = PathBuilder::new();
                p.move_to(*a).line_to(*b);
                p.detach()
            }
        }
    }

    /// Fills the shape with `paint`.
    pub fn fill(&self, canvas: &skia_safe::Canvas, paint: &skia_safe::Paint) {
        match self {
            Shape::Rect(rr) => {
                canvas.draw_rrect(rr, paint);
            }
            Shape::Oval(r) => {
                canvas.draw_oval(r, paint);
            }
            Shape::Path(p) => {
                canvas.draw_path(p, paint);
            }
            Shape::Line(..) => {}
        }
    }

    /// Clips to the shape, anti-aliased.
    pub fn clip(&self, canvas: &skia_safe::Canvas, op: ClipOp) {
        match self {
            Shape::Rect(rr) => {
                canvas.clip_rrect(rr, op, true);
            }
            _ => {
                canvas.clip_path(&self.path(), op, true);
            }
        }
    }

    /// The box the shape sits in.
    pub fn bounds(&self) -> skia_safe::Rect {
        match self {
            Shape::Rect(rr) => *rr.rect(),
            Shape::Oval(r) => *r,
            Shape::Path(p) => *p.bounds(),
            Shape::Line(a, b) => {
                skia_safe::Rect::new(a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y))
            }
        }
    }
}

/// The outline of layer `p`, or `None` for layers without one (text, icons,
/// spacers, firstFit).
pub(super) fn shape_of(p: &Placed) -> Option<Shape> {
    let l = p.layer;
    let r = super::paint::sk_rect(p.rect);
    let rounded = |corner_radius: f32| {
        let corners = match l.look.radius {
            Some(radius) => radius.corners(r.width(), r.height(), p.k),
            None => [corner_radius * p.k; 4],
        };
        rrect_corners(r, corners)
    };
    match &l.kind {
        Kind::Rect { corner_radius, .. } | Kind::Frame { corner_radius, .. } => {
            Some(Shape::Rect(rounded(*corner_radius)))
        }
        Kind::Image { .. } => Some(Shape::Rect(rounded(0.0))),
        Kind::Ellipse { arc: None, .. } => Some(Shape::Oval(r)),
        Kind::Ellipse { arc: Some(a), .. } => Some(Shape::Path(arc_path(r, a))),
        Kind::Polygon {
            sides,
            inner_radius,
            ..
        } => {
            let path = polygon_path(r, *sides, *inner_radius);
            // Rounded points: Skia's corner effect, baked into the outline.
            match l
                .look
                .radius
                .map(|rad| rad.corners(r.width(), r.height(), p.k)[0])
            {
                Some(round) if round > 0.0 => Some(Shape::Path(round_corners(&path, round))),
                _ => Some(Shape::Path(path)),
            }
        }
        Kind::Path {
            d,
            shape,
            fill_rule,
            fit_path,
            ..
        } => {
            let data = d
                .as_deref()
                .or_else(|| shape.as_deref().and_then(crate::shapes::path))?;
            let path = Path::from_svg(data)?;
            let fit = match fit_path {
                FitPath::Contain => skia_safe::matrix::ScaleToFit::Center,
                FitPath::Stretch => skia_safe::matrix::ScaleToFit::Fill,
            };
            let m = skia_safe::Matrix::rect_2_rect(path.bounds(), r, fit)?;
            let path = path.with_transform(&m);
            Some(Shape::Path(match fill_rule {
                FillRule::Nonzero => path,
                FillRule::Evenodd => path.with_fill_type(skia_safe::PathFillType::EvenOdd),
            }))
        }
        Kind::Line { .. } => Some(Shape::Line(
            Point::new(r.left, r.top),
            Point::new(r.right, r.bottom),
        )),
        _ => None,
    }
}

/// A regular polygon inscribed in `r`, first corner at the top; with
/// `inner`, a star whose every other corner sits at that share of the radius.
fn polygon_path(r: skia_safe::Rect, sides: u32, inner: Option<f32>) -> Path {
    let n = sides.max(3) as usize;
    let (c, rx, ry) = (r.center(), r.width() / 2.0, r.height() / 2.0);
    let count = if inner.is_some() { 2 * n } else { n };
    let points: Vec<Point> = (0..count)
        .map(|i| {
            let a = (-90.0 + i as f32 * 360.0 / count as f32).to_radians();
            let s = if inner.is_some() && i % 2 == 1 {
                inner.unwrap_or(1.0)
            } else {
                1.0
            };
            Point::new(c.x + rx * s * a.cos(), c.y + ry * s * a.sin())
        })
        .collect();
    let mut b = PathBuilder::new();
    b.add_polygon(&points, true);
    b.detach()
}

/// Part of the ellipse in `r`: angles clockwise from the top; `inner` > 0
/// cuts a hole (a ring, or a ring segment).
fn arc_path(r: skia_safe::Rect, a: &Arc) -> Path {
    let sweep = (a.end - a.start).clamp(-360.0, 360.0);
    let start = a.start - 90.0;
    let inner = a.inner.clamp(0.0, 0.999);
    let hole = r.with_inset((
        r.width() * (1.0 - inner) / 2.0,
        r.height() * (1.0 - inner) / 2.0,
    ));
    let mut b = PathBuilder::new();
    if sweep.abs() >= 360.0 {
        b.add_oval(r, None, None);
        if inner > 0.0 {
            b.add_oval(hole, None, None);
            b.set_fill_type(skia_safe::PathFillType::EvenOdd);
        }
    } else if inner > 0.0 {
        b.arc_to(r, start, sweep, true);
        b.arc_to(hole, start + sweep, -sweep, false);
        b.close();
    } else {
        b.move_to(r.center());
        b.arc_to(r, start, sweep, false);
        b.close();
    }
    b.detach()
}

/// `path` with its corners rounded to `radius`.
fn round_corners(path: &Path, radius: f32) -> Path {
    let mut paint = skia_safe::Paint::default();
    paint.set_path_effect(PathEffect::corner_path(radius));
    let mut out = PathBuilder::new();
    if skia_safe::path_utils::fill_path_with_paint(path, &paint, &mut out, None, None) {
        out.detach()
    } else {
        path.clone()
    }
}

/// Each corner's radius: top-left, top-right, bottom-right, bottom-left.
pub(super) fn corner_radii(rr: &RRect) -> [f32; 4] {
    [
        Corner::UpperLeft,
        Corner::UpperRight,
        Corner::LowerRight,
        Corner::LowerLeft,
    ]
    .map(|c| rr.radii(c).x)
}

/// A rounded rect with each corner's radius (clamped by Skia to fit).
pub(super) fn rrect_corners(r: skia_safe::Rect, [tl, tr, br, bl]: [f32; 4]) -> RRect {
    if tl == tr && tr == br && br == bl {
        return RRect::new_rect_xy(r, tl, tl);
    }
    RRect::new_rect_radii(
        r,
        &[
            (tl, tl).into(),
            (tr, tr).into(),
            (br, br).into(),
            (bl, bl).into(),
        ],
    )
}
