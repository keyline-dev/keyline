//! Built-in named shapes: SVG paths in a 100 × 100 box, fitted to a layer's
//! box by `path` layers (`"shape": "ribbon"`) and shape masks. Shapes that
//! aren't parametric are cheaper for an agent to name than to draw. Drawn
//! for keyline; no third-party artwork.

/// `(name, SVG path)` for every named shape.
pub const SHAPES: &[(&str, &str)] = &[
    ("ribbon", "M0 20 H100 L90 50 L100 80 H0 L10 50 Z"),
    (
        "ribbon-banner",
        "M15 25 H85 V65 H15 Z M15 35 H0 L8 50 L0 65 H15 Z M85 35 H100 L92 50 L100 65 H85 Z",
    ),
    (
        "bubble",
        "M10 0 H90 Q100 0 100 10 V60 Q100 70 90 70 H40 L20 90 L25 70 H10 Q0 70 0 60 V10 Q0 0 10 0 Z",
    ),
    (
        "bubble-round",
        "M50 0 C78 0 100 16 100 36 C100 56 78 72 50 72 C44 72 38 71 33 70 L15 90 L20 64 C8 58 0 48 0 36 C0 16 22 0 50 0 Z",
    ),
    ("arrow", "M0 35 H65 V15 L100 50 L65 85 V65 H0 Z"),
    (
        "arrow-curved",
        "M10 90 C10 40 40 20 70 20 V5 L100 30 L70 55 V40 C50 40 30 55 30 90 Z",
    ),
    ("chevron", "M0 0 H60 L100 50 L60 100 H0 L40 50 Z"),
    (
        "tag",
        "M0 10 Q0 0 10 0 H70 L100 50 L70 100 H10 Q0 100 0 90 Z",
    ),
    ("arch", "M0 100 V50 A50 50 0 0 1 100 50 V100 Z"),
    (
        "shield",
        "M50 0 L100 15 V45 C100 75 78 92 50 100 C22 92 0 75 0 45 V15 Z",
    ),
    (
        "heart",
        "M50 90 C20 70 0 52 0 30 C0 13 13 0 30 0 C40 0 47 6 50 12 C53 6 60 0 70 0 C87 0 100 13 100 30 C100 52 80 70 50 90 Z",
    ),
    (
        "cloud",
        "M25 80 C11 80 0 70 0 57 C0 45 9 35 21 34 C23 20 35 10 50 10 C63 10 74 18 78 30 C91 31 100 42 100 55 C100 69 89 80 75 80 Z",
    ),
    (
        "wave",
        "M0 30 C20 10 30 10 50 30 C70 50 80 50 100 30 V100 H0 Z",
    ),
    (
        "burst",
        "M50 0 L60 22 L85 10 L78 35 L100 42 L80 56 L95 78 L70 74 L65 100 L50 80 L33 98 L30 72 L5 80 L18 57 L0 42 L22 34 L14 10 L40 22 Z",
    ),
    (
        "blob-1",
        "M50 0 C78 2 100 20 98 50 C96 80 76 100 48 98 C20 96 2 76 2 48 C2 20 22 -2 50 0 Z",
    ),
    (
        "blob-2",
        "M55 2 C82 6 98 28 96 52 C94 78 70 98 44 96 C18 94 0 72 4 46 C8 18 30 -2 55 2 Z",
    ),
    (
        "blob-3",
        "M46 4 C70 -4 96 14 98 40 C100 64 88 92 60 98 C32 104 6 86 2 60 C-2 34 22 12 46 4 Z",
    ),
    (
        "blob-4",
        "M60 0 C86 4 100 30 94 56 C88 84 62 100 36 94 C10 88 -4 62 4 38 C12 14 34 -4 60 0 Z",
    ),
    (
        "blob-5",
        "M40 2 C66 -6 94 8 98 36 C102 62 90 90 62 98 C34 106 4 90 2 62 C0 36 14 10 40 2 Z",
    ),
    (
        "blob-6",
        "M50 6 C74 0 100 18 96 46 C92 70 98 96 66 98 C36 100 4 94 2 64 C0 34 26 12 50 6 Z",
    ),
    (
        "brush-stroke",
        "M4 30 C30 18 70 20 96 26 C99 40 97 58 94 72 C66 80 32 82 6 74 C2 60 1 44 4 30 Z",
    ),
];

/// A named shape's SVG path.
pub fn path(name: &str) -> Option<&'static str> {
    SHAPES.iter().find(|s| s.0 == name).map(|s| s.1)
}

/// Every shape name, for error messages.
pub fn names() -> Vec<&'static str> {
    SHAPES.iter().map(|s| s.0).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_named_shape_parses_as_a_closed_path_in_its_box() {
        for (name, d) in super::SHAPES {
            let p = skia_safe::Path::from_svg(d).unwrap_or_else(|| panic!("{name} doesn't parse"));
            let b = p.bounds();
            assert!(
                b.width().max(b.height()) >= 90.0 && b.width().min(b.height()) >= 30.0,
                "{name}: {b:?}"
            );
            assert!(
                b.left >= -10.0 && b.right <= 110.0 && b.top >= -10.0 && b.bottom <= 110.0,
                "{name}: {b:?}"
            );
        }
        assert!(super::path("ribbon").is_some_and(|d| d.starts_with("M0 20")));
        assert!(super::path("nope").is_none());
        assert!(super::names().contains(&"blob-6"));
    }
}
