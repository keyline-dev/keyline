//! Pixel tests of v2 text: markup, highlights, decoration, vertical
//! alignment, knockout, leaders and curves.

use serde_json::json;

use super::tests::{count, pixels};

const WHITE: (u8, u8, u8) = (255, 255, 255);

fn dark(c: (u8, u8, u8)) -> bool {
    c.0 < 110 && c.1 < 110 && c.2 < 110
}

#[test]
fn markup_colors_a_word() {
    let px = pixels(
        json!([{"type": "text", "text": "ab <span color=\"#FF0000\">RED</span>", "fontSize": 30, "weight": 900}]),
    );
    assert!(
        count(&px, |(r, g, b)| r > 200 && g < 60 && b < 60) > 100,
        "the marked word is red"
    );
    assert!(count(&px, dark) > 30, "the rest stays black");
}

#[test]
fn highlights_draw_behind_the_text() {
    let px = pixels(
        json!([{"type": "text", "text": "Hi", "x": 20, "y": 30, "fontSize": 30,
        "highlight": {"color": "#FFE600", "padding": 6}}]),
    );
    assert!(
        count(&px, |(r, g, b)| r > 240 && g > 200 && b < 40) > 300,
        "yellow box"
    );
    assert!(count(&px, dark) > 30, "text on top");
}

#[test]
fn strike_draws_a_line_through() {
    let plain = pixels(json!([{"type": "text", "text": "0000", "fontSize": 40}]));
    let struck = pixels(json!([{"type": "text", "text": "<s>0000</s>", "fontSize": 40}]));
    assert!(count(&struck, dark) > count(&plain, dark) + 40);
}

#[test]
fn vertical_align_bottom_puts_text_at_the_bottom_of_its_box() {
    let px = pixels(
        json!([{"type": "text", "text": "x", "width": 100, "height": 100, "fontSize": 20, "verticalAlign": "bottom", "resize": "fixed"}]),
    );
    let top = (0..50)
        .flat_map(|y| (0..100).map(move |x| (x, y)))
        .filter(|&(x, y)| dark(px(x, y)))
        .count();
    let bottom = (50..100)
        .flat_map(|y| (0..100).map(move |x| (x, y)))
        .filter(|&(x, y)| dark(px(x, y)))
        .count();
    assert!(top == 0 && bottom > 10, "{top} {bottom}");
}

#[test]
fn knockout_text_cuts_through_its_frame() {
    let px = pixels(json!([
        {"type": "rect", "width": 100, "height": 100, "color": "#FF0000"},
        {"type": "frame", "width": 100, "height": 100, "color": "#000000", "children": [
            {"type": "text", "text": "I", "x": 30, "fontSize": 90, "weight": 900, "knockout": true}]}
    ]));
    assert!(
        count(&px, |(r, g, b)| r > 200 && g < 60 && b < 60) > 200,
        "the red below shows through the letter"
    );
    assert_eq!(px(3, 3), (0, 0, 0), "the frame stays elsewhere");
}

#[test]
fn leaders_fill_the_gap_before_a_flush_right_price() {
    let px = pixels(
        json!([{"type": "text", "text": "Tea\t$3", "width": 100, "fontSize": 14, "leader": "."}]),
    );
    let right = (80..100)
        .flat_map(|x| (0..20).map(move |y| (x, y)))
        .filter(|&(x, y)| dark(px(x, y)))
        .count();
    let middle = (45..70)
        .flat_map(|x| (0..20).map(move |y| (x, y)))
        .filter(|&(x, y)| dark(px(x, y)))
        .count();
    assert!(right > 5, "the price sits at the right edge: {right}");
    assert!(middle > 0, "dots fill the gap: {middle}");
}

#[test]
fn curved_text_bends_up_at_its_middle() {
    let px = pixels(
        json!([{"type": "text", "text": "OOOOOOO", "width": 100, "fontSize": 14, "curve": 60}]),
    );
    // The middle letters sit higher than the ends of the arc.
    let top_of = |x0: i32, x1: i32| {
        (0..100)
            .find(|&y| (x0..x1).any(|x| dark(px(x, y))))
            .unwrap_or(100)
    };
    assert!(
        top_of(40, 60) + 3 < top_of(0, 15).min(top_of(85, 100)),
        "{} {} {}",
        top_of(0, 15),
        top_of(40, 60),
        top_of(85, 100)
    );
    assert_eq!(px(50, 99), WHITE);
}
