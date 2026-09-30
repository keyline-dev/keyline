//! Tests of batched adds and updates: ids, targets, styles and atomicity.

use super::*;
use crate::scene::Kind;

fn shared(styles: Map<String, Value>) -> Shared {
    Shared {
        styles,
        ..Shared::default()
    }
}
use serde_json::json;

fn scene() -> Scene {
    serde_json::from_value(json!({
        "width": 100, "height": 100, "sizes": [{"id": "a", "width": 100, "height": 100}],
        "layers": [
            {"id": "bar", "type": "frame", "width": 100, "height": 20, "children": [
                {"id": "label", "role": "cta", "type": "text", "text": "Go"}
            ]},
            {"id": "t2", "role": "cta", "type": "text", "text": "Now"}
        ]
    }))
    .unwrap()
}

fn op(v: Value) -> Op {
    serde_json::from_value(v).unwrap()
}

#[test]
fn a_style_target_creates_changes_and_deletes_styles() {
    let mut s = scene();
    let op = |v: Value| serde_json::from_value::<Op>(v).unwrap();
    let ids = update_layers(
        &mut s,
        Shared::default(),
        &[op(
            json!({"target": {"style": "h"}, "set": {"fontSize": 40, "fontWeight": 800}}),
        )],
    )
    .unwrap();
    assert_eq!(ids, ["h"]);
    update_layers(
        &mut s,
        Shared::default(),
        &[op(
            json!({"target": {"style": "h"}, "set": {"fontWeight": null}}),
        )],
    )
    .unwrap();
    assert_eq!(s.styles["h"], *json!({"fontSize": 40}).as_object().unwrap());
    update_layers(
        &mut s,
        Shared::default(),
        &[op(json!({"target": {"style": "h"}, "delete": true}))],
    )
    .unwrap();
    assert!(s.styles.is_empty());
    let err = update_layers(
        &mut s,
        Shared::default(),
        &[op(json!({"target": {"style": "h"}, "delete": true}))],
    );
    assert!(err.unwrap_err().contains("no style h"));
}

#[test]
fn add_generates_ids_and_nests_under_parent() {
    let mut s = scene();
    let ids = add_layers(
        &mut s,
        shared(Map::new()),
        vec![
            json!({"type": "rect", "fill": "#FF0000"}),
            json!({"type": "rect", "parent": "bar"}),
            json!({"type": "frame", "children": [{"type": "text", "text": "x"}]}),
        ],
    )
    .unwrap();
    assert_eq!(ids, ["rect1", "rect2", "frame1"]);
    assert_eq!(s.version, 1);
    let Kind::Frame { children, .. } = &s.layers[0].kind else {
        panic!()
    };
    assert_eq!(children[1].id, "rect2");
    let Kind::Frame { children, .. } = &s.layers[3].kind else {
        panic!()
    };
    assert_eq!(children[0].id, "text1");
}

#[test]
fn generated_ids_avoid_ids_given_later_in_the_batch() {
    let mut s = scene();
    let ids = add_layers(
        &mut s,
        shared(Map::new()),
        vec![
            json!({"type": "rect"}),
            json!({"id": "rect1", "type": "rect"}),
        ],
    )
    .unwrap();
    assert_eq!(ids, ["rect2", "rect1"]);
}

#[test]
fn add_is_atomic() {
    let mut s = scene();
    let err = add_layers(
        &mut s,
        shared(Map::new()),
        vec![
            json!({"type": "rect"}),
            json!({"type": "text", "text": "x", "fontsize": 3}),
        ],
    )
    .unwrap_err();
    assert!(
        err.starts_with("layers[1]") && err.contains("fontsize"),
        "{err}"
    );
    assert_eq!(s, scene());
    assert!(
        add_layers(
            &mut s,
            shared(Map::new()),
            vec![json!({"type": "rect", "parent": "t2"})]
        )
        .is_err()
    );
    assert!(
        add_layers(
            &mut s,
            shared(Map::new()),
            vec![json!({"id": "bar", "type": "rect"})]
        )
        .unwrap_err()
        .contains("duplicate")
    );
}

#[test]
fn update_by_role_hits_every_match_and_merges() {
    let mut s = scene();
    let changed = update_layers(&mut s, Shared::default(),
        &[op(json!({"target": {"role": "cta"}, "set": {"color": "#FFFFFF", "constraints": {"horizontal": "center"}}}))],
    )
    .unwrap();
    assert_eq!(changed, ["label", "t2"]);
    update_layers(
        &mut s,
        Shared::default(),
        &[op(
            json!({"target": {"id": "t2"}, "set": {"constraints": {"vertical": "bottom"}}}),
        )],
    )
    .unwrap();
    let t2 = serde_json::to_value(&s.layers[1]).unwrap();
    assert_eq!(
        t2["constraints"],
        json!({"horizontal": "center", "vertical": "bottom"})
    );
    assert_eq!(t2["color"], "#FFFFFF");
}

#[test]
fn null_resets_and_delete_removes() {
    let mut s = scene();
    update_layers(
        &mut s,
        Shared::default(),
        &[op(json!({"target": {"id": "t2"}, "set": {"role": null}}))],
    )
    .unwrap();
    assert_eq!(s.layers[1].role, None);
    update_layers(
        &mut s,
        Shared::default(),
        &[op(json!({"target": {"id": "label"}, "delete": true}))],
    )
    .unwrap();
    let Kind::Frame { children, .. } = &s.layers[0].kind else {
        panic!()
    };
    assert!(children.is_empty());
}

#[test]
fn update_errors_leave_scene_untouched() {
    let mut s = scene();
    for bad in [
        json!({"target": {"id": "nope"}, "set": {"x": 1}}),
        json!({"target": {"id": "t2"}, "set": {"type": "rect"}}),
        json!({"target": {"id": "t2"}, "set": {"fontWeight": 450}}),
        json!({"target": {"id": "t2"}}),
    ] {
        assert!(
            update_layers(
                &mut s,
                Shared::default(),
                &[
                    op(json!({"target": {"id": "t2"}, "set": {"x": 5}})),
                    op(bad.clone())
                ]
            )
            .is_err(),
            "{bad}"
        );
        assert_eq!(s, scene());
    }
}

#[test]
fn common_guesses_are_accepted_on_shapes() {
    let mut s = scene();
    add_layers(
        &mut s,
        shared(Map::new()),
        vec![
            json!({"id": "r", "type": "rect", "fill": "#FFFFFF", "shadow": {"y": 4, "color": "#0004"}}),
            json!({"id": "t", "type": "text", "text": "x", "fill": {"image": "missing"}}),
        ],
    )
    .unwrap_err();
    // The text's `fill` is its own field (an image), so the missing asset fails;
    // the rect's `fill` and `shadow` became `fills` and `shadows`.
    add_layers(&mut s, shared(Map::new()), vec![json!({"id": "r", "type": "rect", "fill": "#FFFFFF", "shadow": {"y": 4, "color": "#0004"}})]).unwrap();
    let r = s.layers.iter().find(|l| l.id == "r").unwrap();
    assert!(r.look.fills.is_some() && r.look.shadows.is_some());
}

#[test]
fn generic_shapes_and_sized_asset_ids_are_understood() {
    let mut s = scene();
    s.assets.insert(
        "photo".into(),
        serde_json::from_value(json!({"sha256": "x", "width": 10, "height": 10})).unwrap(),
    );
    add_layers(
        &mut s,
        shared(Map::new()),
        vec![
            json!({"id": "a", "type": "shape", "fill": "#000"}),
            json!({"id": "b", "type": "shape", "shape": "heart"}),
            json!({"id": "c", "type": "image", "asset": "photo 864×530"}),
        ],
    )
    .unwrap();
    let kind = |id: &str| s.layers.iter().find(|l| l.id == id).unwrap().kind.clone();
    assert_eq!(kind("a").name(), "rect");
    assert_eq!(kind("b").name(), "path");
    assert!(matches!(kind("c"), Kind::Image { asset, .. } if asset == "photo"));
    let e = add_layers(
        &mut s,
        shared(Map::new()),
        vec![json!({"type": "image", "asset": "logo"})],
    )
    .unwrap_err();
    assert!(e.ends_with("unknown asset logo; assets: photo"), "{e}");
}

#[test]
fn the_scene_itself_can_change_after_creation() {
    let mut s = scene();
    let op: Op = serde_json::from_value(json!({"target": {"scene": true},
        "set": {"background": "#000000", "duration": 3, "sizes": ["300x600"]}}))
    .unwrap();
    let changed = update_layers(&mut s, Shared::default(), &[op]).unwrap();
    assert_eq!(changed, ["scene"]);
    assert_eq!(
        (s.background.to_string(), s.duration),
        ("#000000".into(), Some(3.0))
    );
    assert_eq!(s.sizes[0].id, "300x600");
    let op: Op =
        serde_json::from_value(json!({"target": {"scene": true}, "set": {"fill": "#FFF"}}))
            .unwrap();
    let e = update_layers(&mut s, Shared::default(), &[op]).unwrap_err();
    assert!(e.contains("the scene takes background"), "{e}");
}

#[test]
fn a_soundtrack_is_set_by_its_asset_alone_and_reset_by_null() {
    let mut s = scene();
    s.assets.insert(
        "song".into(),
        serde_json::from_value(json!({"sha256": "m", "width": 0, "height": 0,
            "clip": {"duration": 30, "fps": 0, "audio": true}}))
        .unwrap(),
    );
    let set = |v: Value| -> Op {
        serde_json::from_value(json!({"target": {"scene": true}, "set": v})).unwrap()
    };
    update_layers(&mut s, Shared::default(), &[set(json!({"music": "song"}))]).unwrap();
    assert_eq!(s.audio.as_ref().map(|a| a.asset.as_str()), Some("song"));
    let e =
        update_layers(&mut s, Shared::default(), &[set(json!({"audio": "photo"}))]).unwrap_err();
    assert!(e.contains("audio: photo isn't a sound"), "{e}");
    update_layers(&mut s, Shared::default(), &[set(json!({"audio": null}))]).unwrap();
    assert_eq!(s.audio, None);
}

#[test]
fn a_style_s_type_steers_the_guesses_and_is_dropped() {
    let mut s = scene();
    let styles =
        json!({"rule": {"type": "line", "color": "#f00"}, "h": {"type": "text", "fontSize": 40}});
    update_layers(&mut s, shared(styles.as_object().unwrap().clone()), &[]).unwrap();
    assert_eq!(s.styles["h"], *json!({"fontSize": 40}).as_object().unwrap());
    assert_eq!(
        s.styles["rule"],
        *json!({"stroke": {"color": "#f00"}}).as_object().unwrap()
    );
}
