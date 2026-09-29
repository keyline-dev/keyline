//! Inline markup in text: a small HTML subset, so agents style words without
//! counting characters. `<b>`, `<i>`, `<u>`, `<s>`, `<sup>`, `<sub>`,
//! `<br>`, and `<span color=… weight=… italic=… fontSize=… fontFamily=…
//! decoration=… highlight=…>`. Style-name tags (`<accent>`) are expanded
//! into spans when styles are resolved. A `<` that doesn't open a known tag
//! is text; `&lt;` `&gt;` `&amp;` `&quot;` are entities.

use serde_json::{Map, Value};

use crate::scene::Range;

/// Parses `text` into the displayed string and styled spans (character
/// offsets of the displayed string).
pub fn parse(text: &str) -> (String, Vec<Range>) {
    let mut out = String::new();
    let mut spans = Vec::new();
    // Open tags: name and the span they started, with its style.
    let mut open: Vec<(String, Range)> = Vec::new();
    let mut chars = 0usize;
    let mut rest = text;
    while let Some(i) = rest.find(['<', '&']) {
        push(&mut out, &mut chars, &rest[..i]);
        rest = &rest[i..];
        if rest.starts_with('&') {
            let entity = [
                ("&lt;", '<'),
                ("&gt;", '>'),
                ("&amp;", '&'),
                ("&quot;", '"'),
            ]
            .into_iter()
            .find(|(e, _)| rest.starts_with(e));
            match entity {
                Some((e, c)) => {
                    out.push(c);
                    chars += 1;
                    rest = &rest[e.len()..];
                }
                None => {
                    out.push('&');
                    chars += 1;
                    rest = &rest[1..];
                }
            }
            continue;
        }
        match tag(rest) {
            Some((len, Tag::Break)) => {
                out.push('\n');
                chars += 1;
                rest = &rest[len..];
            }
            Some((len, Tag::Open(name, style))) => {
                open.push((
                    name,
                    Range {
                        start: chars,
                        ..style
                    },
                ));
                rest = &rest[len..];
            }
            Some((len, Tag::Close(name))) => {
                if let Some(at) = open.iter().rposition(|(n, _)| *n == name) {
                    // Closing an outer tag closes the ones inside it too.
                    for (_, mut span) in open.drain(at..).rev() {
                        span.end = chars;
                        if span.end > span.start {
                            spans.push(span);
                        }
                    }
                }
                rest = &rest[len..];
            }
            None => {
                out.push('<');
                chars += 1;
                rest = &rest[1..];
            }
        }
    }
    push(&mut out, &mut chars, rest);
    for (_, mut span) in open.into_iter().rev() {
        span.end = chars;
        if span.end > span.start {
            spans.push(span);
        }
    }
    // Outer spans first, so inner (later-starting) ones win where they overlap.
    spans.sort_by_key(|s| (s.start, std::cmp::Reverse(s.end)));
    (out, spans)
}

fn push(out: &mut String, chars: &mut usize, s: &str) {
    out.push_str(s);
    *chars += s.chars().count();
}

enum Tag {
    Break,
    Open(String, Range),
    Close(String),
}

/// A known tag at the start of `s`, with its length in bytes.
fn tag(s: &str) -> Option<(usize, Tag)> {
    let end = s.find('>')?;
    let inner = s[1..end].trim();
    if let Some(name) = inner.strip_prefix('/') {
        let name = name.trim();
        return known(name).then(|| (end + 1, Tag::Close(name.to_owned())));
    }
    let inner = inner.trim_end_matches('/').trim();
    let (name, attrs) = inner.split_once(char::is_whitespace).unwrap_or((inner, ""));
    if name == "br" {
        return Some((end + 1, Tag::Break));
    }
    if !known(name) {
        return None;
    }
    let mut style = Range::default();
    match name {
        "b" | "strong" => style.weight = Some(700),
        "i" | "em" => style.italic = Some(true),
        "u" => style.decoration = Some(crate::scene::Decoration::Underline),
        "s" => style.decoration = Some(crate::scene::Decoration::Strike),
        "sup" => style.shift = Some(crate::scene::Shift::Sup),
        "sub" => style.shift = Some(crate::scene::Shift::Sub),
        _ => style = span_style(attrs).ok()?,
    }
    Some((end + 1, Tag::Open(name.to_owned(), style)))
}

fn known(name: &str) -> bool {
    matches!(
        name,
        "b" | "strong" | "i" | "em" | "u" | "s" | "sup" | "sub" | "span"
    )
}

/// Checks every `<span>` in `text`, so one with bad attributes is an error
/// rather than drawn as literal text.
///
/// # Errors
/// The first bad span, with what's wrong.
pub fn check(text: &str) -> Result<(), String> {
    let mut rest = text;
    while let Some(i) = rest.find("<span") {
        rest = &rest[i + 5..];
        // `<spanish>` is text, not a span.
        if !rest.starts_with(|c: char| c.is_whitespace() || c == '>' || c == '/') {
            continue;
        }
        let Some(end) = rest.find('>') else { break };
        let attrs = rest[..end].trim().trim_end_matches('/').trim();
        span_style(attrs).map_err(|e| format!("<span {attrs}>: {e}"))?;
        rest = &rest[end..];
    }
    Ok(())
}

/// A `<span>`'s attributes as a styled range: its `style`, CSS
/// declarations such as `color:#D0202E;font-weight:800`.
fn span_style(attrs: &str) -> Result<Range, String> {
    let mut obj = Map::new();
    obj.insert("start".into(), 0.into());
    obj.insert("end".into(), 0.into());
    let mut rest = attrs.trim();
    while !rest.is_empty() {
        let (key, after) = rest
            .split_once('=')
            .ok_or_else(|| format!("{rest}: attributes are name=\"value\""))?;
        let after = after.trim_start();
        let (value, next) = match after.chars().next().ok_or("an attribute has no value")? {
            q @ ('"' | '\'') => {
                let close = after[1..]
                    .find(q)
                    .ok_or("an attribute's quote isn't closed")?
                    + 1;
                (&after[1..close], &after[close + 1..])
            }
            _ => after.split_once(char::is_whitespace).unwrap_or((after, "")),
        };
        if key.trim() != "style" {
            return Err(format!(
                "a span takes style=\"…\" (CSS: color, font-weight, font-style, font-size, font-family, text-decoration, background-color), not {}",
                key.trim()
            ));
        }
        for decl in value.split(';').map(str::trim).filter(|d| !d.is_empty()) {
            let (prop, v) = decl
                .split_once(':')
                .ok_or_else(|| format!("{decl}: CSS declarations are property: value"))?;
            let (field, v) = css_declaration(prop.trim(), v.trim())?;
            obj.insert(field.into(), v);
        }
        rest = next.trim_start();
    }
    serde_json::from_value(Value::Object(obj)).map_err(|e| e.to_string())
}

/// One CSS declaration as a range field and its value.
fn css_declaration(prop: &str, v: &str) -> Result<(&'static str, Value), String> {
    let px = |v: &str| {
        v.strip_suffix("px")
            .unwrap_or(v)
            .trim()
            .parse::<f64>()
            .map(Value::from)
            .map_err(|_| format!("{prop}: {v} isn't a size"))
    };
    Ok(match prop {
        "color" => ("color", v.into()),
        "font-weight" => (
            "fontWeight",
            match v {
                "bold" => 700.into(),
                "normal" => 400.into(),
                n => n
                    .parse::<u64>()
                    .map(Value::from)
                    .map_err(|_| format!("font-weight: {n} isn't 100–900"))?,
            },
        ),
        "font-style" => ("fontStyle", v.into()),
        "font-size" => ("fontSize", px(v)?),
        "font-family" => (
            "fontFamily",
            v.split(',')
                .next()
                .unwrap_or(v)
                .trim()
                .trim_matches(['"', '\''])
                .into(),
        ),
        "text-decoration" | "text-decoration-line" => ("textDecoration", v.into()),
        "background-color" | "background" => ("highlight", v.into()),
        other => {
            return Err(format!(
                "a span's style takes color, font-weight, font-style, font-size, font-family, text-decoration or background-color, not {other}"
            ));
        }
    })
}

/// Rewrites style-name tags (`<accent>…</accent>`) into spans with the
/// style's text fields, so text layout needs no style table.
pub fn expand_styles(
    text: &str,
    styles: &std::collections::BTreeMap<String, Map<String, Value>>,
) -> String {
    if !text.contains('<') {
        return text.to_owned();
    }
    let mut out = text.to_owned();
    for (name, fields) in styles {
        let (open, close) = (format!("<{name}>"), format!("</{name}>"));
        if !out.contains(&open) {
            continue;
        }
        let decls: Vec<String> = fields
            .iter()
            .filter_map(|(k, v)| {
                let (_, css) = SPAN_FIELDS.iter().find(|(f, _)| f == k)?;
                let v = match (k.as_str(), v) {
                    ("fontSize", Value::Number(n)) => format!("{n}px"),
                    (_, Value::String(s)) => s.clone(),
                    (_, Value::Number(n)) => n.to_string(),
                    _ => return None,
                };
                Some(format!("{css}:{v}"))
            })
            .collect();
        out = out
            .replace(&open, &format!("<span style=\"{}\">", decls.join(";")))
            .replace(&close, "</span>");
    }
    out
}

/// Style fields a span can take, with their CSS properties.
const SPAN_FIELDS: &[(&str, &str)] = &[
    ("color", "color"),
    ("fontWeight", "font-weight"),
    ("fontStyle", "font-style"),
    ("fontSize", "font-size"),
    ("fontFamily", "font-family"),
    ("textDecoration", "text-decoration"),
    ("highlight", "background-color"),
];

#[cfg(test)]
mod tests {
    use super::{expand_styles, parse};
    use crate::scene::{Decoration, Shift};

    #[test]
    fn bad_spans_are_errors_and_other_angle_text_is_fine() {
        use super::check;
        check(r##"Proven <span style="color:#D0202E;font-weight:800">RESULTS</span>"##).unwrap();
        check("a <spanish> b < c <span>x</span>").unwrap();
        let e = check(r#"Proven <span style="color:$red">RESULTS</span>"#).unwrap_err();
        assert!(e.starts_with(r#"<span style="color:$red">: "#), "{e}");
        let e = check(r#"<span color="red">x</span>"#).unwrap_err();
        assert!(e.contains("a span takes style="), "{e}");
        let e = check(r#"<span style="font-weight">x</span>"#).unwrap_err();
        assert!(e.contains("property: value"), "{e}");
    }

    #[test]
    fn tags_become_spans_over_the_displayed_text() {
        let (text, spans) = parse("Proven <b>RESULTS</b> for <i>you</i>");
        assert_eq!(text, "Proven RESULTS for you");
        assert_eq!(
            (spans[0].start, spans[0].end, spans[0].weight),
            (7, 14, Some(700))
        );
        assert_eq!(
            (spans[1].start, spans[1].end, spans[1].italic),
            (19, 22, Some(true))
        );
    }

    #[test]
    fn spans_take_css_in_their_style() {
        let (text, spans) = parse(
            r##"<s>$49</s> <span style="color: #D0202E; font-weight: bold; background-color: #FFE600">$29</span><sup>99</sup>"##,
        );
        assert_eq!(text, "$49 $2999");
        assert_eq!(spans[0].decoration, Some(Decoration::Strike));
        assert_eq!(
            (spans[1].weight, spans[1].color.map(|c| c.to_string())),
            (Some(700), Some("#D0202E".into()))
        );
        assert!(spans[1].highlight.is_some());
        assert_eq!((spans[2].start, spans[2].shift), (7, Some(Shift::Sup)));
        let (_, spans) = parse("<strong>a</strong><em>b</em>");
        assert_eq!((spans[0].weight, spans[1].italic), (Some(700), Some(true)));
    }

    #[test]
    fn stray_brackets_and_entities_stay_text() {
        let (text, spans) = parse("a < b, x<y, 1 &lt; 2 &amp; <br>next <unknown>tag</unknown>");
        assert_eq!(text, "a < b, x<y, 1 < 2 & \nnext <unknown>tag</unknown>");
        assert!(spans.is_empty());
    }

    #[test]
    fn unclosed_tags_run_to_the_end_and_nesting_works() {
        let (text, spans) = parse("<b>bold <i>both</b> plain");
        assert_eq!(text, "bold both plain");
        assert_eq!(
            spans.iter().map(|s| (s.start, s.end)).collect::<Vec<_>>(),
            [(0, 9), (5, 9)]
        );
    }

    #[test]
    fn style_tags_expand_into_spans() {
        let styles = serde_json::from_value(
            serde_json::json!({"accent": {"color": "#D0202E", "fontWeight": 800, "letterSpacing": 2}}),
        )
        .unwrap();
        let out = expand_styles("Proven <accent>RESULTS</accent>", &styles);
        assert_eq!(
            out,
            r##"Proven <span style="color:#D0202E;font-weight:800">RESULTS</span>"##
        );
        let (text, spans) = parse(&out);
        assert_eq!(
            (text.as_str(), spans[0].weight),
            ("Proven RESULTS", Some(800))
        );
    }
}
