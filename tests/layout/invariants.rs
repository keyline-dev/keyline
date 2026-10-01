//! Properties that hold for every layout, checked on a small set of typical
//! scenes at sizes of every shape: what a stack or grid draws stays inside
//! it unless it reports `!overflow`, a stack's children never overlap along
//! it, a reversed stack mirrors the plain one, and a free layer keeps the
//! distances its constraints pin.

use crate::*;
use keyline_mcp::layout::{Placed, Rect, layout};
use keyline_mcp::scene::{Dir, Dirs, Kind, Pin, Position, Scene, Size};
use serde_json::{Value, json};

const SIZES: [(f32, f32); 7] = [
    (1000.0, 500.0),
    (1080.0, 1080.0),
    (1080.0, 1920.0),
    (1200.0, 628.0),
    (728.0, 90.0),
    (160.0, 600.0),
    (300.0, 250.0),
];

/// Typical scenes on the 1000 × 500 master.
fn corpus() -> Vec<Value> {
    let tag = |id: &str, t: &str| json!({"id": id, "type": "text", "text": t, "fontSize": 18, "padding": [6, 12]});
    vec![
        // Free layers pinned every way, with px, % and fill sizes.
        json!([
            {"id": "right", "type": "rect", "x": 700, "y": 20, "width": 200, "height": 50, "constraints": {"horizontal": "right"}},
            {"id": "right-pct", "type": "rect", "x": 400, "y": 80, "width": "50%", "height": 50, "constraints": {"horizontal": "right"}},
            {"id": "center-pct", "type": "rect", "x": 100, "y": 140, "width": "30%", "height": 50, "constraints": {"horizontal": "center", "vertical": "center"}},
            {"id": "bottom-pct", "type": "rect", "x": 20, "y": 300, "width": 100, "height": "20%", "constraints": {"vertical": "bottom"}},
            {"id": "stretch", "type": "rect", "x": 50, "y": 400, "width": 900, "height": 40, "constraints": {"horizontal": "stretch"}},
            {"id": "caption", "type": "text", "x": 600, "y": 440, "text": "Pinned bottom right", "constraints": {"horizontal": "right", "vertical": "bottom"}}
        ]),
        // A card: a padded column with a headline, copy and an icon row.
        json!([{"id": "card", "type": "frame", "x": 40, "y": 40, "width": 420, "flexDirection": "column", "gap": 12, "padding": 24, "children": [
            {"id": "h", "type": "text", "fontSize": 40, "fontWeight": 800, "text": "Cold brew season"},
            {"id": "p", "type": "text", "fontSize": 18, "text": "Slow-steeped for sixteen hours, served over ice, all summer long."},
            {"id": "meta", "type": "frame", "flexDirection": "row", "gap": 8, "alignItems": "center", "children": [
                {"id": "dot", "type": "ellipse", "width": 12, "height": 12},
                {"id": "where", "type": "text", "fontSize": 14, "text": "Every store from June"}]}]}]),
        // A toolbar: a fill title between buttons, and a wrapping tag list.
        json!([
            {"id": "bar", "type": "frame", "width": "fill", "height": 64, "flexDirection": "row", "gap": 16, "padding": [0, 24], "alignItems": "center", "children": [
                {"id": "logo", "type": "rect", "width": 40, "height": 40},
                {"id": "title", "type": "text", "width": "fill", "fontSize": 20, "text": "Quarterly report"},
                tag("btn1", "Share"), tag("btn2", "Export")]},
            {"id": "tags", "type": "frame", "y": 100, "width": "60%", "flexDirection": "row", "flexWrap": "wrap", "gap": [8, 8], "children": [
                tag("t1", "design"), tag("t2", "layout"), tag("t3", "typography"), tag("t4", "colour"), tag("t5", "grids")]}
        ]),
        // A grid of cards with areas.
        json!([{"id": "grid", "type": "frame", "width": "fill", "height": "fill", "padding": 20, "gap": 16,
            "gridTemplateColumns": "2fr 1fr", "gridTemplateRows": "1fr auto", "gridTemplateAreas": ["hero side", "cta side"], "children": [
            {"id": "hero", "type": "rect", "gridArea": "hero"},
            {"id": "side", "type": "rect", "gridArea": "side"},
            {"id": "cta", "type": "text", "gridArea": "cta", "fontSize": 24, "text": "Book a table"}]}]),
    ]
}

fn scene_of(layers: &Value) -> Scene {
    scene(layers.clone())
}

fn size_of((w, h): (f32, f32)) -> Size {
    size(&format!("{w}x{h}"), w, h, 1.0)
}

/// Runs `f` on every placed layer of `scene` at `size`.
fn each_placed(scene: &Scene, size: &Size, f: &mut impl FnMut(&Placed)) {
    fn walk(placed: &[Placed], f: &mut impl FnMut(&Placed)) {
        for p in placed {
            f(p);
            walk(&p.children, f);
        }
    }
    let resolved = scene.resolved();
    let sized = resolved.for_size(size);
    walk(&layout(&sized, size), f);
}

/// The children a stack or grid places itself (not absolute ones).
fn flow<'a>(p: &'a Placed<'a>) -> impl Iterator<Item = &'a Placed<'a>> {
    p.children
        .iter()
        .filter(|c| c.layer.position != Position::Absolute)
}

/// The stack's direction, when `p` is a stack.
fn stack_dir(p: &Placed) -> Option<(Dir, bool)> {
    match &p.layer.kind {
        Kind::Frame { layout, .. } => layout.stack.as_ref().map(|s| {
            let dir = match &s.dir {
                Dirs::One(d) => *d,
                Dirs::FirstFit(ds) => p
                    .chosen
                    .as_deref()
                    .and_then(|c| ds.iter().copied().find(|d| d.name() == c))
                    .unwrap_or(Dir::Row),
            };
            (dir, s.wrap)
        }),
        _ => None,
    }
}

fn is_grid(p: &Placed) -> bool {
    matches!(&p.layer.kind, Kind::Frame { layout, .. } if layout.grid.is_some())
}

fn inside(child: Rect, parent: Rect) -> bool {
    child.x >= parent.x - 0.5
        && child.y >= parent.y - 0.5
        && child.right() <= parent.right() + 0.5
        && child.bottom() <= parent.bottom() + 0.5
}

#[test]
fn stacks_and_grids_keep_their_children_inside_unless_they_report_overflow() {
    let mut bad = Vec::new();
    for layers in corpus() {
        let s = scene_of(&layers);
        for wh in SIZES {
            each_placed(&s, &size_of(wh), &mut |p| {
                if (stack_dir(p).is_some() || is_grid(p)) && p.overflow.is_none() {
                    for c in flow(p).filter(|c| !inside(c.rect, p.rect)) {
                        bad.push(format!(
                            "{wh:?} {} outside {}: {:?} in {:?}",
                            c.layer.id, p.layer.id, c.rect, p.rect
                        ));
                    }
                }
            });
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn a_stack_never_overlaps_its_children_along_it() {
    let mut bad = Vec::new();
    for layers in corpus() {
        let s = scene_of(&layers);
        for wh in SIZES {
            each_placed(&s, &size_of(wh), &mut |p| {
                let Some((dir, false)) = stack_dir(p) else {
                    return;
                };
                let span = |r: &Rect| {
                    if dir.is_row() {
                        (r.x, r.right())
                    } else {
                        (r.y, r.bottom())
                    }
                };
                let mut spans: Vec<(f32, f32, &str)> = flow(p)
                    .map(|c| {
                        let (a, b) = span(&c.rect);
                        (a, b, c.layer.id.as_str())
                    })
                    .collect();
                spans.sort_by(|a, b| a.0.total_cmp(&b.0));
                for w in spans.windows(2) {
                    if w[1].0 < w[0].1 - 0.5 {
                        bad.push(format!(
                            "{wh:?} {} overlaps {} in {}",
                            w[1].2, w[0].2, p.layer.id
                        ));
                    }
                }
            });
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn a_reversed_stack_mirrors_the_plain_one() {
    let children = json!([
        {"id": "a", "type": "rect", "width": 60, "height": 30},
        {"id": "b", "type": "text", "text": "Middle child"},
        {"id": "c", "type": "rect", "width": "fill", "height": 20, "maxWidth": 80}
    ]);
    let mut bad = Vec::new();
    for justify in [
        "flex-start",
        "center",
        "flex-end",
        "space-between",
        "space-around",
    ] {
        for (plain, reversed) in [("row", "row-reverse"), ("column", "column-reverse")] {
            let frame = |dir: &str| {
                scene(
                    json!([{"id": "f", "type": "frame", "x": 10, "y": 20, "width": 400, "height": 300,
                    "padding": 15, "gap": 10, "flexDirection": dir, "justifyContent": justify,
                    "alignItems": "flex-start", "children": children.clone()}]),
                )
            };
            let master = size("master", 1000.0, 500.0, 1.0);
            let (p, r) = (
                boxes(&frame(plain), &master),
                boxes(&frame(reversed), &master),
            );
            let f = p["f"];
            for id in ["a", "b", "c"] {
                let (pb, rb) = (p[id], r[id]);
                // Mirrored along the stack, unchanged across it.
                let want = if plain == "row" {
                    (2.0 * f.x + f.w - pb.right(), pb.y)
                } else {
                    (pb.x, 2.0 * f.y + f.h - pb.bottom())
                };
                let same_size = (pb.w - rb.w).abs() < 0.5 && (pb.h - rb.h).abs() < 0.5;
                if (rb.x - want.0).abs() > 0.5 || (rb.y - want.1).abs() > 0.5 || !same_size {
                    bad.push(format!(
                        "{reversed} {justify} {id}: got {rb:?}, mirror of {pb:?}"
                    ));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn free_layers_keep_the_distances_their_constraints_pin() {
    let master = size("master", 1000.0, 500.0, 1.0);
    let mut bad = Vec::new();
    for layers in corpus() {
        let s = scene_of(&layers);
        let at_master = boxes(&s, &master);
        let top: Vec<_> = s.layers.iter().filter(|l| l.place.is_none()).collect();
        for wh in SIZES {
            let b = boxes(&s, &size_of(wh));
            for l in &top {
                let (Some(old), Some(new)) = (at_master.get(&l.id), b.get(&l.id)) else {
                    continue;
                };
                let axes = [
                    (
                        Pin::from(l.constraints.h),
                        old.x,
                        old.right(),
                        1000.0,
                        new.x,
                        new.right(),
                        wh.0,
                    ),
                    (
                        Pin::from(l.constraints.v),
                        old.y,
                        old.bottom(),
                        500.0,
                        new.y,
                        new.bottom(),
                        wh.1,
                    ),
                ];
                for (axis, (pin, o0, o1, ow, n0, n1, nw)) in axes.into_iter().enumerate() {
                    let kept = match pin {
                        Pin::Start => (n0 - o0).abs() < 0.5,
                        Pin::End => ((nw - n1) - (ow - o1)).abs() < 0.5,
                        Pin::Center => (((n0 + n1) - nw) - ((o0 + o1) - ow)).abs() < 1.0,
                        Pin::Stretch => {
                            (n0 - o0).abs() < 0.5 && ((nw - n1) - (ow - o1)).abs() < 0.5
                        }
                        Pin::Scale => true,
                    };
                    if !kept {
                        let a = ["x", "y"][axis];
                        bad.push(format!(
                            "{wh:?} {} {pin:?} on {a}: master {old:?}, got {new:?}",
                            l.id
                        ));
                    }
                }
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}
