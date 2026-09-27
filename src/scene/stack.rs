//! Stack (auto layout) settings for frames.

use serde::{Deserialize, Serialize};

use super::defaults::{is_default, is_zero};

/// A frame that places its children one after another, like CSS flexbox or
/// a design tool's Auto Layout. Children's `x`, `y` and `constraints` are
/// ignored. Without a `width` or `height`, the frame hugs its children.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Stack {
    /// Main axis.
    pub dir: Dir,
    /// Space between children, px (default 0).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub gap: f32,
    /// Space inside the frame's edges, px (default 0).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub padding: f32,
    /// Where children sit across the main axis (default start).
    #[serde(default, skip_serializing_if = "is_default")]
    pub align: StackAlign,
    /// How children share leftover space along the main axis (default start).
    #[serde(default, skip_serializing_if = "is_default")]
    pub justify: Justify,
}

/// A stack's main axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Dir {
    /// Left to right.
    Row,
    /// Top to bottom.
    Column,
}

/// Cross-axis placement of a stack's children.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StackAlign {
    /// Top of a row, left of a column.
    #[default]
    Start,
    /// Centered.
    Center,
    /// Bottom of a row, right of a column.
    End,
}

/// Main-axis distribution of a stack's children.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Justify {
    /// Packed at the start.
    #[default]
    Start,
    /// Packed in the middle.
    Center,
    /// Packed at the end.
    End,
    /// First and last at the edges, the rest spread evenly between.
    Between,
    /// Equal space around every child, edges included.
    Evenly,
}
