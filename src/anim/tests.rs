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
        json!([{"id": "t", "type": "rect", "width": 100, "height": 100,
            "in": {"effect": "fade-up", "at": 1, "duration": 0.5, "ease": "none"},
            "out": {"effect": "fade", "duration": 1, "ease": "none"}}]),
        json!({}),
    );
    let at = |t: f32| find(&at_time(&s, t).layers, "t").clone();
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
        json!([{"id": "cta", "type": "rect", "color": "#000000", "width": 100, "height": 50,
            "animate": [{"scale": [1, 1.2, 1], "duration": 2, "ease": "none", "repeat": -1},
                        {"color": {"to": "#FF0000"}, "at": 1, "duration": 1, "ease": "none"}],
            "in": {"effect": "fade", "duration": 1, "ease": "none"}}]),
        json!({}),
    );
    let l = |t: f32| find(&at_time(&s, t).layers, "cta").clone();
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
        json!([{"id": "row", "type": "frame", "stack": {"dir": "row"}, "stagger": 0.5,
            "in": {"effect": "fade", "duration": 0.5, "ease": "none"},
            "children": [{"id": "a", "type": "rect", "width": 50, "height": 50},
                         {"id": "b", "type": "rect", "width": 50, "height": 50},
                         {"id": "c", "type": "rect", "width": 50, "height": 50}]}]),
        json!({}),
    );
    let t = at_time(&s, 0.75);
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
        json!([{"id": "cards", "type": "use", "component": "card", "stagger": 0.4,
            "in": {"effect": "pop", "duration": 0.4, "ease": "none"}, "each": [{}, {}, {}]}]),
        json!({"components": {"card": {"type": "rect", "width": 50, "height": 50}}}),
    );
    let t = at_time(&s, 0.4);
    let opacity = |id: &str| find(&t.layers, id).opacity;
    assert_eq!(
        (opacity("cards.0"), opacity("cards.1"), opacity("cards.2")),
        (1.0, 0.0, 0.0)
    );
    assert_eq!(opacity("cards.1"), 0.0);
    assert!(at_time(&s, 1.2).layers.iter().all(|l| l.opacity == 1.0));
}

#[test]
fn bad_timing_is_refused_in_one_line() {
    let mut v = json!({"width": 400, "height": 400, "duration": 0,
        "sizes": [{"id": "a", "width": 400, "height": 400}]});
    let s: Scene = serde_json::from_value(v.clone()).unwrap();
    assert!(s.validate().unwrap_err().contains("duration must be > 0"));
    v["duration"] = json!(2);
    v["layers"] = json!([{"type": "rect", "in": "slide-in"}]);
    let e = serde_json::from_value::<Scene>(v).unwrap_err();
    assert!(e.to_string().contains("unknown effect"), "{e}");
}
