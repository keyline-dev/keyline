//! Tests of tokens, styles on any layer, and components through edits.

use super::*;
use crate::scene::{Color, Kind};
use serde_json::json;

fn scene() -> Scene {
    serde_json::from_value(
        json!({"width": 400, "height": 400, "sizes": [{"id": "a", "width": 400, "height": 400}]}),
    )
    .unwrap()
}

fn with(tokens: Value, components: Value) -> Shared {
    let map = |v: Value| match v {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    Shared {
        tokens: map(tokens),
        components: map(components),
        ..Shared::default()
    }
}

fn rect_color(s: &Scene, id: &str) -> Option<Color> {
    let mut out = None;
    s.walk(&mut |l| {
        if l.id == id
            && let Some(crate::scene::OneOrMany::One(crate::scene::Paint::Solid(p))) = &l.look.fills
        {
            out = Some(p.color);
        }
    });
    out
}

#[test]
fn changing_a_token_updates_every_layer_bound_to_it() {
    let mut s = scene();
    add_layers(&mut s, with(json!({"brand": "#D0202E"}), json!({})), vec![
        json!({"id": "a", "type": "rect", "fill": "{{brand}}"}),
        json!({"id": "b", "type": "frame", "children": [{"id": "c", "type": "rect", "fill": "{{brand}}"}]}),
    ])
    .unwrap();
    assert_eq!(rect_color(&s, "a"), Color::parse("#D0202E"));
    update_layers(&mut s, with(json!({"brand": "#1B2A5C"}), json!({})), &[]).unwrap();
    assert_eq!(rect_color(&s, "a"), Color::parse("#1B2A5C"));
    assert_eq!(rect_color(&s, "c"), Color::parse("#1B2A5C"));
}

#[test]
fn an_explicit_value_replaces_the_token_binding() {
    let mut s = scene();
    add_layers(
        &mut s,
        with(json!({"brand": "#D0202E"}), json!({})),
        vec![json!({"id": "a", "type": "rect", "fill": "{{brand}}"})],
    )
    .unwrap();
    let op: Op =
        serde_json::from_value(json!({"target": {"id": "a"}, "set": {"fill": "#000000"}})).unwrap();
    update_layers(&mut s, Shared::default(), &[op]).unwrap();
    update_layers(&mut s, with(json!({"brand": "#00FF00"}), json!({})), &[]).unwrap();
    assert_eq!(rect_color(&s, "a"), Color::parse("#000000"));
}

#[test]
fn styles_apply_to_any_layer_and_a_later_style_wins() {
    let mut s = scene();
    let styles =
        json!({"card": {"fill": "#FFFFFF", "borderRadius": 16}, "danger": {"fill": "#FF0000"}});
    add_layers(
        &mut s,
        Shared {
            styles: styles.as_object().cloned().unwrap(),
            ..Shared::default()
        },
        vec![
            json!({"id": "a", "type": "rect", "style": ["card", "danger"]}),
            json!({"id": "b", "type": "rect", "style": "card", "fill": "#000000"}),
        ],
    )
    .unwrap();
    let r = s.resolved();
    assert_eq!(rect_color(&r, "a"), Color::parse("#FF0000"));
    assert_eq!(
        rect_color(&r, "b"),
        Color::parse("#000000"),
        "the layer's own field wins"
    );
    let e = add_layers(
        &mut s,
        Shared::default(),
        vec![json!({"type": "rect", "style": "nope"})],
    )
    .unwrap_err();
    assert!(
        e.contains("unknown style nope; styles: card, danger"),
        "{e}"
    );
}

#[test]
fn editing_a_component_changes_every_instance_and_detach_frees_them() {
    let mut s = scene();
    let card = json!({"card": {"type": "frame", "children": [{"type": "rect", "role": "dot", "fill": "#FF0000"}]}});
    add_layers(
        &mut s,
        with(json!({}), card),
        vec![json!({"id": "cards", "type": "use", "component": "card", "each": [{}, {}]})],
    )
    .unwrap();
    let r = s.resolved();
    assert_eq!(rect_color(&r, "cards.1.dot"), Color::parse("#FF0000"));
    let op: Op = serde_json::from_value(
        json!({"target": {"component": "card", "role": "dot"}, "set": {"fill": "#0000FF"}}),
    )
    .unwrap();
    update_layers(&mut s, Shared::default(), &[op]).unwrap();
    let r = s.resolved();
    assert_eq!(rect_color(&r, "cards.0.dot"), Color::parse("#0000FF"));
    assert_eq!(rect_color(&r, "cards.1.dot"), Color::parse("#0000FF"));
    let op: Op =
        serde_json::from_value(json!({"target": {"id": "cards"}, "detach": true})).unwrap();
    update_layers(&mut s, Shared::default(), &[op]).unwrap();
    assert_eq!(
        rect_color(&s, "cards.1.dot"),
        Color::parse("#0000FF"),
        "plain layers now"
    );
    let op: Op = serde_json::from_value(
        json!({"target": {"component": "card", "role": "dot"}, "set": {"color": "#00FF00"}}),
    )
    .unwrap();
    update_layers(&mut s, Shared::default(), &[op]).unwrap();
    assert_eq!(
        rect_color(&s, "cards.1.dot"),
        Color::parse("#0000FF"),
        "detached instances keep their look"
    );
}

#[test]
fn unknown_tokens_and_components_say_what_exists() {
    let mut s = scene();
    let e = add_layers(
        &mut s,
        with(json!({"red": "#F00"}), json!({})),
        vec![json!({"type": "rect", "fill": "{{blue}}"})],
    )
    .unwrap_err();
    assert!(e.ends_with("unknown token {{blue}}; tokens: red"), "{e}");
    let e = add_layers(
        &mut s,
        Shared::default(),
        vec![json!({"id": "u", "type": "use", "component": "ghost"})],
    )
    .unwrap_err();
    assert!(e.contains("unknown component ghost"), "{e}");
}

#[test]
fn deleting_by_role_removes_inner_layers_or_the_whole_component() {
    let mut s = scene();
    let card = json!({"card": {"type": "frame", "role": "box", "children": [
        {"type": "rect", "role": "dot"}, {"type": "rect", "role": "bar"}]}});
    add_layers(&mut s, with(json!({}), card), vec![]).unwrap();
    let delete = |role: &str| -> Op {
        serde_json::from_value(
            json!({"target": {"component": "card", "role": role}, "delete": true}),
        )
        .unwrap()
    };
    update_layers(&mut s, Shared::default(), &[delete("dot")]).unwrap();
    assert_eq!(
        s.components["card"]["children"].as_array().unwrap().len(),
        1
    );
    // The root's own role: the component goes, not a null left behind.
    update_layers(&mut s, Shared::default(), &[delete("box")]).unwrap();
    assert!(s.components.is_empty());
    let e = update_layers(&mut s, Shared::default(), &[delete("box")]).unwrap_err();
    assert!(e.ends_with("no component card; components: "), "{e}");
}

#[test]
fn an_own_field_set_to_its_default_still_beats_the_style() {
    let mut s = scene();
    let styles = json!({"soft": {"opacity": 0.5, "color": "#FF0000"}});
    add_layers(
        &mut s,
        Shared {
            styles: styles.as_object().cloned().unwrap(),
            ..Shared::default()
        },
        vec![
            json!({"id": "t", "type": "text", "text": "Hi", "style": "soft",
            "opacity": 1, "color": "#000000"}),
        ],
    )
    .unwrap();
    let look = |s: &Scene| {
        let r = s.resolved();
        let l = r.layers[0].clone();
        let Kind::Text { color, .. } = l.kind else {
            panic!("not text")
        };
        (l.opacity, color)
    };
    assert_eq!(look(&s), (1.0, Color::parse("#000000").unwrap()));

    // An unrelated edit keeps them; resetting one hands it back to the style.
    let set = |v: Value| Op {
        target: serde_json::from_value(json!({"id": "t"})).unwrap(),
        set: v.as_object().cloned(),
        delete: false,
        detach: false,
    };
    update_layers(&mut s, Shared::default(), &[set(json!({"text": "Hello"}))]).unwrap();
    assert_eq!(look(&s), (1.0, Color::parse("#000000").unwrap()));
    update_layers(&mut s, Shared::default(), &[set(json!({"opacity": null}))]).unwrap();
    assert_eq!(look(&s), (0.5, Color::parse("#000000").unwrap()));
}

#[test]
fn a_bad_at_key_inside_a_component_is_an_error() {
    let mut s = scene();
    let e = add_layers(
        &mut s,
        with(
            json!({}),
            json!({"card": {"type": "rect", "media": {"skyy": {"hidden": true}}}}),
        ),
        vec![json!({"id": "c", "type": "use", "component": "card"})],
    )
    .unwrap_err();
    assert!(e.contains("no size or aspect class skyy"), "{e}");
}

#[test]
fn a_token_of_the_wrong_type_is_named() {
    let mut s = scene();
    let e = add_layers(
        &mut s,
        with(json!({"big": "huge"}), json!({})),
        vec![json!({"type": "text", "text": "Hi", "fontSize": "{{big}}"})],
    )
    .unwrap_err();
    assert!(e.contains("token {{big}} doesn't suit fontSize"), "{e}");
}

#[test]
fn changing_a_token_to_the_wrong_type_is_named() {
    let mut s = scene();
    add_layers(
        &mut s,
        with(json!({"big": 40}), json!({})),
        vec![json!({"id": "t", "type": "text", "text": "Hi", "fontSize": "{{big}}"})],
    )
    .unwrap();
    let e = update_layers(&mut s, with(json!({"big": "huge"}), json!({})), &[]).unwrap_err();
    assert!(e.contains("t: token {{big}} doesn't suit it"), "{e}");
}

fn text_of(s: &Scene, id: &str) -> String {
    let mut out = String::new();
    s.walk(&mut |l| {
        if l.id == id
            && let Kind::Text { text, .. } = &l.kind
        {
            out.clone_from(text);
        }
    });
    out
}

fn set(id: &str, fields: &Value) -> Op {
    serde_json::from_value(json!({"target": {"id": id}, "set": fields})).unwrap()
}

#[test]
fn tokens_inside_a_sentence_follow_their_changes_until_the_text_is_rewritten() {
    let mut s = scene();
    add_layers(
        &mut s,
        with(json!({"name": "Dana", "age": 7}), json!({})),
        vec![json!({"id": "t", "type": "text", "text": "Meet {{name}}, {{age}} months old"})],
    )
    .unwrap();
    assert_eq!(text_of(&s, "t"), "Meet Dana, 7 months old");
    update_layers(&mut s, with(json!({"age": 8}), json!({})), &[]).unwrap();
    assert_eq!(text_of(&s, "t"), "Meet Dana, 8 months old");
    // Another field's edit keeps the binding; the text's own edit ends it.
    update_layers(
        &mut s,
        Shared::default(),
        &[set("t", &json!({"fontSize": 30}))],
    )
    .unwrap();
    update_layers(&mut s, with(json!({"name": "Omar"}), json!({})), &[]).unwrap();
    assert_eq!(text_of(&s, "t"), "Meet Omar, 8 months old");
    update_layers(
        &mut s,
        Shared::default(),
        &[set("t", &json!({"text": "Hello"}))],
    )
    .unwrap();
    update_layers(&mut s, with(json!({"name": "Ruth"}), json!({})), &[]).unwrap();
    assert_eq!(text_of(&s, "t"), "Hello");
}

#[test]
fn a_row_of_tokens_changes_a_sentence_and_an_image() {
    let mut s = scene();
    for id in ["cat", "dog"] {
        s.assets.insert(
            id.into(),
            serde_json::from_value(json!({"sha256": id, "width": 10, "height": 10})).unwrap(),
        );
    }
    add_layers(
        &mut s,
        with(json!({"name": "Mia", "photo": "cat"}), json!({})),
        vec![
            json!({"id": "t", "type": "text", "text": "Meet {{name}}"}),
            json!({"id": "p", "type": "image", "asset": "{{photo}}"}),
        ],
    )
    .unwrap();
    // What `rows` does for each row: the row's tokens over the scene's.
    let mut row = s.clone();
    update_layers(
        &mut row,
        with(json!({"name": "Rex", "photo": "dog"}), json!({})),
        &[],
    )
    .unwrap();
    assert_eq!(text_of(&row, "t"), "Meet Rex");
    let mut asset = String::new();
    row.walk(&mut |l| {
        if let Kind::Image { asset: a, .. } = &l.kind {
            asset.clone_from(a);
        }
    });
    assert_eq!(asset, "dog");
    assert_eq!(
        text_of(&s, "t"),
        "Meet Mia",
        "the scene itself is unchanged"
    );
}

#[test]
fn a_dollar_token_as_a_whole_value_binds_and_in_text_gets_a_hint() {
    let mut s = scene();
    add_layers(
        &mut s,
        with(json!({"brand": "#D0202E", "price": 29}), json!({})),
        vec![
            json!({"id": "a", "type": "rect", "fill": "$brand"}),
            json!({"id": "t", "type": "text", "text": "Only $price today, was $35"}),
        ],
    )
    .unwrap();
    assert_eq!(rect_color(&s, "a"), Color::parse("#D0202E"));
    update_layers(&mut s, with(json!({"brand": "#1B2A5C"}), json!({})), &[]).unwrap();
    assert_eq!(
        rect_color(&s, "a"),
        Color::parse("#1B2A5C"),
        "bound, as {{brand}}"
    );
    assert_eq!(
        text_of(&s, "t"),
        "Only $price today, was $35",
        "text isn't rewritten"
    );
    assert_eq!(
        dollar_hints(&s),
        ["hint: did you mean {{price}}? (t says $price)"]
    );
}

#[test]
fn inside_a_component_a_prop_wins_over_a_token_of_the_same_name() {
    let mut s = scene();
    let card = json!({"card": {"type": "text", "text": "{{name}} from {{city}}"}});
    add_layers(
        &mut s,
        with(json!({"name": "token", "city": "Haifa"}), card),
        vec![json!({"id": "u", "type": "use", "component": "card", "props": {"name": "Dana"}})],
    )
    .unwrap();
    let drawn = s.resolved();
    assert_eq!(text_of(&drawn, "u"), "Dana from Haifa");
    // A missing prop that no token covers is named as such.
    let badge = json!({"badge": {"type": "text", "text": "{{who}} votes"}});
    let e = add_layers(
        &mut s,
        with(json!({}), badge),
        vec![json!({"id": "v", "type": "use", "component": "badge", "props": {"city": "Acre"}})],
    )
    .unwrap_err();
    assert!(
        e.ends_with("v: {{who}} is neither a prop nor a token; props: city; tokens: city, name"),
        "{e}"
    );
    let e = add_layers(&mut s, with(json!({"n": 1}), json!({})), vec![]).unwrap_err();
    assert!(e.starts_with("{{n}} is the counting number"), "{e}");
}
