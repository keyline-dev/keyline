//! How problem lines reach the reply: which lines show, advisories grouped
//! across sizes, and unnamed texts named by their words.

use std::fmt::Write;

use crate::scene::Kind;

/// The lines with a problem, and above them a `firstFit` that skipped
/// an option, since its choice may be why (`→ short (long: headline cut
/// at maxLines 3)`).
pub(super) fn shown(lines: &[String]) -> impl Iterator<Item = &String> {
    let problem = |l: &str| l.contains(" !") || l.contains(" warn ");
    let indent = |l: &str| l.len() - l.trim_start().len();
    lines.iter().enumerate().filter_map(move |(i, l)| {
        let chose = l.contains(" firstFit ") && l.contains(" → ") && l.ends_with(')');
        let below = || {
            lines[i + 1..]
                .iter()
                .take_while(|c| indent(c) > indent(l))
                .any(|c| problem(c))
        };
        (problem(l) || (chose && below())).then_some(l)
    })
}

/// Problem lines, `(size, line)`, as the reply shows them: each with its
/// size in front, except an advisory alone on its line that the same
/// layer has at several sizes, and whose fix is the same at each
/// (contrast, a clipped shadow; not a crop, which may need a taller box
/// here and a wider one there). That becomes one line, after the others,
/// naming the sizes (`all` for every one of the `total`) and keeping the
/// worst value, without the box that differs per size:
/// `all text2 "TODAY" warn contrast 2.9:1 (WCAG 4.5)`.
pub(super) fn grouped(problems: &[(&str, String)], total: usize) -> String {
    // The layer (and its quoted text), and the advisory from `warn` on.
    let split = |l: &str| -> Option<(String, String)> {
        if l.contains(" !") {
            return None;
        }
        let at = l.find(" warn ")?;
        let mut words = l.split(' ');
        let id = words.next()?;
        let name = l
            .split_once(" \"")
            .filter(|(head, _)| !head.contains(' '))
            .and_then(|(_, rest)| rest.split_once('"'))
            .map_or_else(String::new, |(q, _)| format!(" \"{q}\""));
        // A crop's fix differs per size (taller here, wider there): kept apart.
        if l[at..].starts_with(" warn crop ") {
            return None;
        }
        Some((format!("{id}{name}"), l[at + 1..].to_owned()))
    };
    // How bad an advisory is, to keep the worst: the lowest contrast ratio.
    let badness = |warn: &str| -> f32 {
        let num = |s: &str| {
            s.split(|c: char| !c.is_ascii_digit() && c != '.')
                .next()
                .and_then(|n| n.parse::<f32>().ok())
                .unwrap_or(0.0)
        };
        warn.strip_prefix("warn contrast ").map_or(0.0, |r| -num(r))
    };
    // Which advisory: `warn contrast`, `warn shadow`.
    let advisory = |warn: &str| warn.split(' ').take(2).collect::<Vec<_>>().join(" ");
    let mut groups: Vec<(String, String, Vec<&str>, String, f32)> = Vec::new();
    for (size, line) in problems {
        let Some((who, warn)) = split(line) else {
            continue;
        };
        let kind = advisory(&warn);
        let bad = badness(&warn);
        match groups.iter_mut().find(|g| g.0 == who && g.1 == kind) {
            Some(g) => {
                g.2.push(size);
                if bad > g.4 {
                    (g.3, g.4) = (warn, bad);
                }
            }
            None => groups.push((who, kind, vec![size], warn, bad)),
        }
    }
    let many = |who: &str, kind: &str| {
        groups
            .iter()
            .any(|g| g.0 == who && g.1 == kind && g.2.len() > 1)
    };
    let mut out = String::new();
    for (size, line) in problems {
        match split(line) {
            Some((who, warn)) if many(&who, &advisory(&warn)) => {}
            _ => {
                let _ = writeln!(out, "{size} {line}");
            }
        }
    }
    for (who, _, sizes, warn, _) in groups.iter().filter(|g| g.2.len() > 1) {
        let sizes = if sizes.len() == total {
            "all".to_owned()
        } else {
            sizes.join(",")
        };
        let _ = writeln!(out, "{sizes} {who} {warn}");
    }
    out
}

/// A text layer's words, quoted, when its id was made up by the server
/// (`text7`, or `card.text2` inside a component), so a problem line says
/// which text it is: `"TODAY"`. Up to 16 characters.
pub(super) fn named(l: &crate::scene::Layer) -> Option<String> {
    let Kind::Text { text, .. } = &l.kind else {
        return None;
    };
    let last = l.id.rsplit('.').next()?;
    let digits = last.strip_prefix("text")?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (plain, _) = crate::text::markup::parse(text);
    let plain = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    let short: String = plain.chars().take(16).collect();
    let more = if short.len() < plain.len() { "…" } else { "" };
    Some(format!("\"{short}{more}\""))
}
