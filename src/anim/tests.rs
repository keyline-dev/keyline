//! Tests of applying animation to a scene at a moment.

use super::at_time;
use crate::scene::{Color, Kind, Layer, Scene};
use serde_json::{Value, json};

fn scene(layers: Value, extra: Value) -> Scene {
    let mut v = json!({"width": 400, "height": 400, "duration": 4,
        "sizes": [{"id": "a", "width": 400, "height": 400}]});
    v["layers"] = layers;
    if let Value::Object(m) = extra {
        for (k, x) in m {
            v[k] = x;
        }
    }
    let s: Scene = serde_json::from_value(v).unwrap();
    s.validate().unwrap();
    s.resolved().into_owned()
}

fn find<'a>(layers: &'a [Layer], id: &str) -> &'a Layer {
    fn go<'a>(layers: &'a [Layer], id: &str) -> Option<&'a Layer> {
        layers.iter().find_map(|l| {
            if l.id == id {
                Some(l)
            } else {
                l.kind.children().and_then(|c| go(c, id))
            }
        })
    }
    go(layers, id).unwrap_or_else(|| panic!("no layer {id}"))
}

#[test]
fn a_layer_enters_and_leaves_on_the_timeline() {
    let s = scene(
        json!([{"id": "t", "type": "rect", "width": 100, "height": 100, "enter": {"effect": "fade-up", "delay": 1, "duration": 0.5, "ease": "none"}, "exit": {"effect": "fade", "duration": 1, "ease": "none"}}]),
        json!({}),
    );
    let at = |t: f32| find(&at_time(&s, t, &s.sizes[0]).layers, "t").clone();
    assert_eq!(at(0.5).opacity, 0.0, "not in yet");
    assert_eq!(at(0.5).look.offset, [0.0, 40.0]);
    assert!((at(1.25).opacity - 0.5).abs() < 1e-5);
    assert_eq!(at(2.0).opacity, 1.0, "at rest in between");
    assert_eq!(at(2.0).look.offset, [0.0, 0.0]);
    assert!(
        (at(3.5).opacity - 0.5).abs() < 1e-5,
        "leaving over the last second"
    );
    assert_eq!(at(4.0).opacity, 0.0);
    // The scene itself is untouched: it's the rest state.
    assert_eq!(find(&s.layers, "t").opacity, 1.0);
}

#[test]
fn tracks_set_values_and_effects_apply_on_top() {
    let s = scene(
        json!([{"id": "cta", "type": "rect", "width": 100, "height": 50, "animate": [{"scale": [1, 1.2, 1], "duration": 2, "ease": "none", "repeat": -1}, {"color": {"to": "#FF0000"}, "delay": 1, "duration": 1, "ease": "none"}], "enter": {"effect": "fade", "duration": 1, "ease": "none"}, "fill": "#000000"}]),
        json!({}),
    );
    let l = |t: f32| find(&at_time(&s, t, &s.sizes[0]).layers, "cta").clone();
    assert!((l(1.0).look.scale - 1.2).abs() < 1e-5);
    assert!((l(0.5).look.scale - 1.1).abs() < 1e-5);
    assert!(
        (l(0.5).opacity - 0.5).abs() < 1e-5,
        "the entrance fades on top"
    );
    assert!((l(3.0).look.scale - 1.2).abs() < 1e-5, "repeats");
    let color = |l: &Layer| match &l.kind {
        Kind::Rect { color, .. } => *color,
        _ => None,
    };
    assert_eq!(color(&l(0.5)), Some(Color(0xFF00_0000)));
    assert_eq!(color(&l(2.0)), Some(Color(0xFFFF_0000)));
}

#[test]
fn a_staggering_frame_hands_its_entrance_to_its_children() {
    let s = scene(
        json!([{"id": "row", "type": "frame", "flexDirection": "row", "alignItems": "flex-start", "stagger": 0.5, "enter": {"effect": "fade", "duration": 0.5, "ease": "none"}, "children": [{"id": "a", "type": "rect", "width": 50, "height": 50}, {"id": "b", "type": "rect", "width": 50, "height": 50}, {"id": "c", "type": "rect", "width": 50, "height": 50}]}]),
        json!({}),
    );
    let t = at_time(&s, 0.75, &s.sizes[0]);
    assert_eq!(
        find(&t.layers, "row").opacity,
        1.0,
        "the frame itself stays"
    );
    assert_eq!(find(&t.layers, "a").opacity, 1.0, "the first is in");
    assert!(
        (find(&t.layers, "b").opacity - 0.5).abs() < 1e-5,
        "the second halfway"
    );
    assert_eq!(find(&t.layers, "c").opacity, 0.0, "the third not yet");
}

#[test]
fn use_instances_enter_one_after_another() {
    let s = scene(
        json!([{"id": "cards", "type": "use", "component": "card", "stagger": 0.4, "enter": {"effect": "pop", "duration": 0.4, "ease": "none"}, "each": [{}, {}, {}]}]),
        json!({"components": {"card": {"type": "rect", "width": 50, "height": 50}}}),
    );
    let t = at_time(&s, 0.4, &s.sizes[0]);
    let opacity = |id: &str| find(&t.layers, id).opacity;
    assert_eq!(
        (opacity("cards.0"), opacity("cards.1"), opacity("cards.2")),
        (1.0, 0.0, 0.0)
    );
    assert_eq!(opacity("cards.1"), 0.0);
    assert!(
        at_time(&s, 1.2, &s.sizes[0])
            .layers
            .iter()
            .all(|l| l.opacity == 1.0)
    );
}

#[test]
fn bad_timing_is_refused_in_one_line() {
    let mut v = json!({"width": 400, "height": 400, "duration": 0,
        "sizes": [{"id": "a", "width": 400, "height": 400}]});
    let s: Scene = serde_json::from_value(v.clone()).unwrap();
    assert!(s.validate().unwrap_err().contains("duration must be > 0"));
    v["duration"] = json!(2);
    v["layers"] = json!([{"type": "rect", "enter": "slide-in"}]);
    let e = serde_json::from_value::<Scene>(v).unwrap_err();
    assert!(e.to_string().contains("unknown effect"), "{e}");
}

#[test]
fn draw_and_count_tracks_set_the_moment_and_the_count_box_holds_still() {
    let s = scene(
        json!([{"id": "n", "type": "text", "text": "{{n}} users", "fontSize": 20,
            "animate": {"count": [0, 12500], "separator": ",", "ease": "none", "draw": [0, 1]}}]),
        json!({}),
    );
    let at = |t: f32| at_time(&s, t, &s.sizes[0]);
    let (start, mid) = (at(0.0), at(0.5));
    assert_eq!(
        (start.layers[0].time.count, mid.layers[0].time.count),
        (Some(0.0), Some(6250.0))
    );
    assert_eq!(mid.layers[0].time.drawn, Some(0.5));
    assert_eq!(s.layers[0].time.count, None, "at rest: none set");
    let width = |sc: &Scene| crate::layout::layout(sc, &sc.sizes[0])[0].rect.w;
    assert_eq!(
        width(&start),
        width(&at(1.0)),
        "measured with 12,500 throughout"
    );
    assert_eq!(width(&start), width(&s));
}

#[test]
fn draw_and_count_are_refused_where_they_would_do_nothing() {
    let s: Scene = serde_json::from_value(json!({"width": 100, "height": 100, "duration": 1,
        "sizes": [{"id": "a", "width": 100, "height": 100}],
        "layers": [{"id": "t", "type": "text", "text": "{{n}}", "split": "chars",
            "animate": {"count": [0, 9]}}]}))
    .unwrap();
    let e = s.validate().unwrap_err();
    assert!(
        e.starts_with("t: draw and count don't work on split text"),
        "{e}"
    );
    let s: Scene = serde_json::from_value(json!({"width": 100, "height": 100, "duration": 1,
        "sizes": [{"id": "a", "width": 100, "height": 100}],
        "layers": [{"id": "t", "type": "text", "text": "9 teams", "animate": {"count": [0, 9]}}]}))
    .unwrap();
    assert_eq!(
        s.validate().unwrap_err(),
        "t: count needs text with {{n}} where the number goes"
    );
}
