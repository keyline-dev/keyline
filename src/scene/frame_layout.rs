//! A frame's layout, written on the frame as in CSS: `flexDirection` makes
//! it a stack, a grid template makes it a grid, and `gap`, `padding` and
//! the alignment fields belong to whichever it is.

use serde::{Deserialize, Serialize};

use super::{Dirs, Gap, Grid, Justify, Padding, Stack, StackAlign, Tracks};

/// How a frame places its children: a stack, a grid, or neither (each
/// child by its own position).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FrameLayout {
    /// A row or column (CSS flexbox).
    pub stack: Option<Stack>,
    /// Rows and columns (CSS grid).
    pub grid: Option<Grid>,
    /// Stack fields (`alignItems`, `gap`, …) written without a direction:
    /// kept, since a `style` may bring the `flexDirection`; a frame that
    /// still has them once styles apply is refused.
    pub loose: Option<serde_json::Map<String, serde_json::Value>>,
}

/// Why a frame's [`FrameLayout::loose`] fields are refused.
pub const LOOSE: &str = "padding, gap, justifyContent, alignItems and flexWrap need flexDirection (a row or column) or gridTemplateColumns (a grid)";

/// CSS `flex-wrap`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum FlexWrap {
    Nowrap,
    Wrap,
}

/// The fields as written.
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Flat {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    flex_direction: Option<Dirs>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    justify_content: Option<Justify>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    align_items: Option<StackAlign>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    flex_wrap: Option<FlexWrap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    gap: Option<Gap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    padding: Option<Padding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    grid_template_columns: Option<Tracks>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    grid_template_rows: Option<Tracks>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    grid_template_areas: Option<Vec<String>>,
}

impl<'de> Deserialize<'de> for FrameLayout {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let f = Flat::deserialize(d)?;
        let grid = f.grid_template_columns.is_some()
            || f.grid_template_rows.is_some()
            || f.grid_template_areas.is_some();
        let flex_only =
            f.justify_content.is_some() || f.align_items.is_some() || f.flex_wrap.is_some();
        match (f.flex_direction.clone(), grid) {
            (Some(_), true) => Err(D::Error::custom(
                "a frame is a stack (flexDirection) or a grid (gridTemplateColumns), not both",
            )),
            (Some(dir), false) => Ok(FrameLayout {
                stack: Some(Stack {
                    dir,
                    gap: f.gap.unwrap_or_default(),
                    padding: f.padding.unwrap_or_default(),
                    align: f.align_items.unwrap_or_default(),
                    justify: f.justify_content.unwrap_or_default(),
                    wrap: f.flex_wrap == Some(FlexWrap::Wrap),
                }),
                grid: None,
                loose: None,
            }),
            (None, true) if flex_only => Err(D::Error::custom(
                "justifyContent, alignItems and flexWrap are for stacks; a grid's children take gridArea, gridRow and gridColumn",
            )),
            (None, true) => Ok(FrameLayout {
                stack: None,
                grid: Some(Grid {
                    columns: f.grid_template_columns,
                    rows: f.grid_template_rows,
                    gap: f.gap.unwrap_or_default(),
                    padding: f.padding.unwrap_or_default(),
                    areas: f.grid_template_areas.unwrap_or_default(),
                }),
                loose: None,
            }),
            (None, false) if flex_only || f.gap.is_some() || f.padding.is_some() => {
                match serde_json::to_value(&f).map_err(D::Error::custom)? {
                    serde_json::Value::Object(o) => Ok(FrameLayout {
                        loose: Some(o),
                        ..FrameLayout::default()
                    }),
                    _ => Err(D::Error::custom(LOOSE)),
                }
            }
            (None, false) => Ok(FrameLayout::default()),
        }
    }
}

/// `v` when `on`: a field written only when it isn't its default.
fn set<T>(on: bool, v: T) -> Option<T> {
    on.then_some(v)
}

impl Serialize for FrameLayout {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let f = match (&self.stack, &self.grid) {
            (Some(st), _) => Flat {
                flex_direction: Some(st.dir.clone()),
                justify_content: set(st.justify != Justify::default(), st.justify),
                align_items: set(st.align != StackAlign::default(), st.align),
                flex_wrap: set(st.wrap, FlexWrap::Wrap),
                gap: set(!st.gap.is_zero(), st.gap),
                padding: set(!st.padding.is_zero(), st.padding),
                ..Flat::default()
            },
            (None, Some(g)) => Flat {
                grid_template_columns: g.columns.clone(),
                grid_template_rows: g.rows.clone(),
                grid_template_areas: set(!g.areas.is_empty(), g.areas.clone()),
                gap: set(!g.gap.is_zero(), g.gap),
                padding: set(!g.padding.is_zero(), g.padding),
                ..Flat::default()
            },
            (None, None) => match &self.loose {
                Some(o) => return o.serialize(s),
                None => Flat::default(),
            },
        };
        f.serialize(s)
    }
}

#[cfg(test)]
mod tests {
    use super::FrameLayout;
    use serde_json::json;

    #[test]
    fn flex_and_grid_fields_on_the_frame_make_a_stack_or_a_grid() {
        let l: FrameLayout = serde_json::from_value(json!({"flexDirection": "row", "gap": 8,
            "justifyContent": "space-between", "flexWrap": "wrap"}))
        .unwrap();
        let s = l.stack.clone().unwrap();
        assert!(s.wrap && l.grid.is_none());
        assert_eq!(
            serde_json::to_value(&l).unwrap(),
            json!({"flexDirection": "row", "justifyContent": "space-between", "flexWrap": "wrap", "gap": 8.0})
        );
        let g: FrameLayout =
            serde_json::from_value(json!({"gridTemplateColumns": "1fr 2fr", "gap": 4})).unwrap();
        assert!(g.grid.is_some() && g.stack.is_none());
        let free: FrameLayout = serde_json::from_value(json!({})).unwrap();
        assert_eq!(free, FrameLayout::default());
        // Stack fields without a direction wait for a style's (validation
        // refuses them if none comes), and are written back as they were.
        let loose: FrameLayout =
            serde_json::from_value(json!({"padding": 24, "alignItems": "center"})).unwrap();
        assert!(loose.stack.is_none() && loose.loose.is_some());
        assert_eq!(
            serde_json::to_value(&loose).unwrap(),
            json!({"padding": 24.0, "alignItems": "center"})
        );
    }
}
