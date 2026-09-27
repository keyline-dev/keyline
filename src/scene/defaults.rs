//! Serde default values and `skip_serializing_if` predicates for the scene
//! model, so every default is omitted from the stored JSON.

use super::Color;

pub(super) fn white() -> Color {
    Color(0xFFFF_FFFF)
}
pub(super) fn black() -> Color {
    Color(0xFF00_0000)
}
pub(super) fn is_black(c: &Color) -> bool {
    *c == black()
}
pub(super) fn one() -> f32 {
    1.0
}
pub(super) fn is_one(v: &f32) -> bool {
    *v == 1.0
}
pub(super) fn is_zero(v: &f32) -> bool {
    *v == 0.0
}
pub(super) fn sixteen() -> f32 {
    16.0
}
pub(super) fn is_sixteen(v: &f32) -> bool {
    *v == 16.0
}
pub(super) fn w400() -> u16 {
    400
}
pub(super) fn is_w400(v: &u16) -> bool {
    *v == 400
}
pub(super) fn inter() -> String {
    "Inter".into()
}
pub(super) fn is_inter(v: &String) -> bool {
    v == "Inter"
}
pub(super) fn left_mid() -> [f32; 2] {
    [0.0, 0.5]
}
pub(super) fn right_mid() -> [f32; 2] {
    [1.0, 0.5]
}
pub(super) fn center() -> [f32; 2] {
    [0.5, 0.5]
}
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's skip_serializing_if passes a reference"
)]
pub(super) fn is_center(v: &[f32; 2]) -> bool {
    *v == center()
}
pub(super) fn half() -> f32 {
    0.5
}
pub(super) fn is_half(v: &f32) -> bool {
    *v == 0.5
}
pub(super) fn yes() -> bool {
    true
}
pub(super) fn is_true(v: &bool) -> bool {
    *v
}
pub(super) fn is_false(v: &bool) -> bool {
    !*v
}
pub(super) fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}
