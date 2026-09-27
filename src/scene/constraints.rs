//! Layer constraints: how a layer follows its parent when the parent resizes.

use serde::{Deserialize, Serialize};

/// A layer's constraints on each axis.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Constraints {
    /// Horizontal constraint.
    #[serde(default)]
    pub h: HConstraint,
    /// Vertical constraint.
    #[serde(default)]
    pub v: VConstraint,
}

impl Constraints {
    pub(super) fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Horizontal constraint, as in design tools.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HConstraint {
    /// Keep the distance to the parent's left edge.
    #[default]
    Left,
    /// Keep the distance to the parent's right edge.
    Right,
    /// Keep the offset from the parent's center.
    Center,
    /// Keep both margins; the width changes.
    Stretch,
    /// Position and width scale with the parent.
    Scale,
}

/// Vertical constraint, as in design tools.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VConstraint {
    /// Keep the distance to the parent's top edge.
    #[default]
    Top,
    /// Keep the distance to the parent's bottom edge.
    Bottom,
    /// Keep the offset from the parent's center.
    Center,
    /// Keep both margins; the height changes.
    Stretch,
    /// Position and height scale with the parent.
    Scale,
}

/// A constraint on one axis, with `left`/`top` as `Start`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pin {
    /// Pinned to the start edge (left or top).
    Start,
    /// Pinned to the end edge (right or bottom).
    End,
    /// Pinned to the center.
    Center,
    /// Pinned to both edges.
    Stretch,
    /// Proportional to the parent.
    Scale,
}

impl From<HConstraint> for Pin {
    fn from(c: HConstraint) -> Self {
        match c {
            HConstraint::Left => Pin::Start,
            HConstraint::Right => Pin::End,
            HConstraint::Center => Pin::Center,
            HConstraint::Stretch => Pin::Stretch,
            HConstraint::Scale => Pin::Scale,
        }
    }
}

impl From<VConstraint> for Pin {
    fn from(c: VConstraint) -> Self {
        match c {
            VConstraint::Top => Pin::Start,
            VConstraint::Bottom => Pin::End,
            VConstraint::Center => Pin::Center,
            VConstraint::Stretch => Pin::Stretch,
            VConstraint::Scale => Pin::Scale,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::scene::{Constraints, Pin};
    use serde_json::json;

    #[test]
    fn constraint_names_parse() {
        let c: Constraints = serde_json::from_value(json!({"h": "right", "v": "bottom"})).unwrap();
        assert_eq!(Pin::from(c.h), Pin::End);
        assert_eq!(Pin::from(c.v), Pin::End);
        assert!(serde_json::from_value::<Constraints>(json!({"h": "top"})).is_err());
    }
}
