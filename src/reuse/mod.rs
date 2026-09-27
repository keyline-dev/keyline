//! Reuse: tokens (`"$brand"` in any field), and components placed by `use`
//! layers. Tokens are substituted when a layer is written, and each layer
//! remembers which fields came from which token, so changing a token
//! updates them; components stay templates, expanded when a scene is
//! resolved for layout and rendering.

pub mod components;
pub mod tokens;
