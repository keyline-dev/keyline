//! The speaker card's assets: an event logo mark and a stand-in portrait
//! (original drawings for the benchmark).

use skia_safe::{Color, EncodedImageFormat, Paint, Rect, surfaces};

/// The event logo mark: three rising bars and a ring, violet and white.
pub const LOGO_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 160 48" width="160" height="48">
<rect x="0" y="28" width="12" height="20" rx="3" fill="#7C5CFF"/>
<rect x="18" y="16" width="12" height="32" rx="3" fill="#7C5CFF"/>
<rect x="36" y="4" width="12" height="44" rx="3" fill="#FFFFFF"/>
<circle cx="84" cy="24" r="18" fill="none" stroke="#FFFFFF" stroke-width="8"/>
<circle cx="84" cy="24" r="6" fill="#7C5CFF"/>
<path d="M114 40 L130 8 L146 40 Z" fill="#7C5CFF"/>
<rect x="150" y="8" width="10" height="32" rx="3" fill="#FFFFFF"/></svg>"##;

/// An 800×800 stand-in for a head-and-shoulders photo.
pub fn portrait_png() -> Vec<u8> {
    let mut surface = surfaces::raster_n32_premul((800, 800)).expect("surface");
    let c = surface.canvas();
    c.clear(Color::from_rgb(214, 196, 170));
    let mut p = Paint::default();
    p.set_anti_alias(true);
    // Window light on the wall.
    p.set_color(Color::from_rgb(236, 224, 204));
    c.draw_rect(Rect::from_xywh(520.0, 0.0, 280.0, 420.0), &p);
    // Shoulders and jacket.
    p.set_color(Color::from_rgb(38, 52, 86));
    c.draw_oval(Rect::from_xywh(110.0, 560.0, 580.0, 520.0), &p);
    // Collar.
    p.set_color(Color::from_rgb(245, 245, 245));
    c.draw_oval(Rect::from_xywh(330.0, 555.0, 140.0, 90.0), &p);
    // Neck and head.
    p.set_color(Color::from_rgb(150, 104, 78));
    c.draw_rect(Rect::from_xywh(350.0, 470.0, 100.0, 120.0), &p);
    c.draw_oval(Rect::from_xywh(275.0, 190.0, 250.0, 320.0), &p);
    // Hair.
    p.set_color(Color::from_rgb(40, 28, 24));
    c.draw_oval(Rect::from_xywh(255.0, 150.0, 290.0, 190.0), &p);
    // Eyes and smile.
    c.draw_circle((355.0, 350.0), 12.0, &p);
    c.draw_circle((445.0, 350.0), 12.0, &p);
    p.set_color(Color::from_rgb(120, 60, 50));
    c.draw_oval(Rect::from_xywh(360.0, 420.0, 80.0, 26.0), &p);
    let png = surface
        .image_snapshot()
        .encode(None, EncodedImageFormat::PNG, None)
        .expect("encode");
    png.as_bytes().to_vec()
}
