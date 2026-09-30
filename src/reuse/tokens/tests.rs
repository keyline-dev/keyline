//! Tests of token substitution and binding.

use super::{bind, placeholders, reference, set_at};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn tokens(v: Value) -> BTreeMap<String, Value> {
    serde_json::from_value(v).unwrap()
}

#[test]
fn a_placeholder_is_a_name_in_double_braces() {
    assert_eq!(reference("{{brand}}"), Some("brand"));
    assert_eq!(reference("{{ color.brand }}"), Some("color.brand"));
    assert_eq!(reference("Meet {{name}}"), None, "not the whole value");
    assert_eq!(reference("{{29}}"), None);
    assert_eq!(reference("{{n}}"), None, "the counting number");
    let names: Vec<&str> = placeholders("{{a}} and {{b-2}} {{}} {{ }} {curly} {{x")
        .iter()
        .map(|p| p.1)
        .collect();
    assert_eq!(names, ["a", "b-2"]);
}

#[test]
fn whole_values_keep_their_type_and_sentences_splice() {
    let t = tokens(json!({"red": "#D0202E", "pad": 24, "name": "Dana", "age": 7}));
    let mut v = json!({"type": "frame", "fill": "{{red}}", "stack": {"padding": "{{pad}}"},
        "children": [{"type": "text", "text": "Meet {{name}}, {{age}} months old", "color": "{{red}}"}]});
    bind(&mut v, &t).unwrap();
    assert_eq!(
        (&v["fill"], &v["stack"]["padding"]),
        (&json!("#D0202E"), &json!(24))
    );
    assert_eq!(
        v["$tokens"],
        json!({"/fill": "red", "/stack/padding": "pad"})
    );
    let child = &v["children"][0];
    assert_eq!(child["text"], "Meet Dana, 7 months old");
    assert_eq!(
        child["$tokens"],
        json!({"/color": "red", "/text": "Meet {{name}}, {{age}} months old"}),
        "a sentence keeps its template"
    );
}

#[test]
fn markup_attributes_take_tokens() {
    let t = tokens(json!({"red": "#D0202E", "big": 60}));
    let mut v = json!({"type": "text", "text": r#"Pay <span style="color:{{red}};font-size:{{big}}">now</span>"#});
    bind(&mut v, &t).unwrap();
    assert_eq!(
        v["text"],
        r##"Pay <span style="color:#D0202E;font-size:60">now</span>"##
    );
}

#[test]
fn text_without_a_placeholder_is_never_touched() {
    let t = tokens(json!({"price": 29}));
    for text in [
        "$29",
        "{curly}",
        "{{ }}",
        "Only {{",
        "{{n}}+ adopted",
        "$price",
    ] {
        let mut v = json!({"type": "text", "text": text});
        bind(&mut v, &t).unwrap();
        assert_eq!(v["text"], text);
        assert!(v.get("$tokens").is_none(), "{text}");
    }
}

#[test]
fn an_unknown_name_lists_the_tokens() {
    let t = tokens(json!({"red": "#D0202E", "pad": 24}));
    let e = bind(&mut json!({"type": "text", "text": "Hi {{blue}}"}), &t).unwrap_err();
    assert_eq!(e, "unknown token {{blue}}; tokens: pad, red");
    let e = bind(
        &mut json!({"type": "rect", "fill": "{{blue}}"}),
        &BTreeMap::new(),
    )
    .unwrap_err();
    assert_eq!(e, "unknown token {{blue}}; add it under tokens");
    let e = bind(
        &mut json!({"type": "text", "text": "a {{list}}"}),
        &tokens(json!({"list": [1]})),
    )
    .unwrap_err();
    assert!(e.contains("use it as a whole value"), "{e}");
}

#[test]
fn media_changes_bind_too() {
    let t = tokens(json!({"headline": "Summer sale"}));
    let mut v = json!({"type": "text", "text": "{{headline}}", "media": {"sky": {"text": "{{headline}}!"}}});
    bind(&mut v, &t).unwrap();
    assert_eq!(
        (&v["text"], &v["media"]["sky"]["text"]),
        (&json!("Summer sale"), &json!("Summer sale!"))
    );
}

#[test]
fn a_default_first_value_still_follows_its_token() {
    let mut v = json!({"type": "text"});
    set_at(&mut v, "/color", json!("#D0202E"));
    set_at(&mut v, "/stack/padding", json!(8));
    assert_eq!(
        v,
        json!({"type": "text", "color": "#D0202E", "stack": {"padding": 8}})
    );
}
