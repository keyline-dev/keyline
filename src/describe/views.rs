//! What the checks look at: the scene once per shot, without the shots a
//! size never plays, and with knockout frames hidden for what shows
//! through them.

use crate::layout::Placed;
use crate::scene::{Kind, Scene};

/// The scene as checked: whole, or once per shot with only that shot
/// shown, each with a moment it's fully on screen (its transition in done)
/// and whether it's the first view. Shots in `unplayed` (hidden at this
/// size by `media`) aren't checked.
pub(super) fn views(scene: &Scene, unplayed: &[usize]) -> Vec<(Scene, f32, bool)> {
    let shots = crate::anim::shots::timeline(scene);
    if shots.is_empty() {
        return vec![(scene.clone(), 0.0, true)];
    }
    let mut out: Vec<(Scene, f32, bool)> = shots
        .iter()
        .enumerate()
        .filter(|(_, (i, ..))| !unplayed.contains(i))
        .map(|(n, &(i, start, _, into))| {
            let mut view = scene.clone();
            for &(j, ..) in &shots {
                view.layers[j].hidden = j != i;
            }
            let settled = if n == 0 {
                0.0
            } else {
                start + into.map_or(0.0, crate::anim::shots::Transition::overlap)
            };
            (view, settled, false)
        })
        .collect();
    // The layers around the shots are listed with the first view checked.
    if let Some(first) = out.first_mut() {
        first.2 = true;
    }
    out
}

/// The top-level shots `media` hides at `size` (an A4 page that shows a
/// video's first shot only): `scene` is before `for_size`, which forgets
/// where `hidden` came from.
pub(super) fn unplayed(scene: &Scene, size: &crate::scene::Size) -> Vec<usize> {
    let mut keys = crate::scene::aspect_classes(size);
    keys.push(&size.id);
    scene
        .layers
        .iter()
        .enumerate()
        .filter(|(_, l)| l.time.shot.is_some())
        .filter(|(_, l)| {
            keys.iter()
                .rev()
                .find_map(|k| l.at.get(*k)?.get("hidden")?.as_bool())
                .unwrap_or(false)
        })
        .map(|(i, _)| i)
        .collect()
}

/// Ids of the frames whose knockout text cuts through them.
pub(super) fn knockout_frames(placed: &[Placed]) -> Vec<String> {
    let mut out = Vec::new();
    for p in placed {
        if p.children
            .iter()
            .any(|c| matches!(&c.layer.kind, Kind::Text { more, .. } if more.knockout))
        {
            out.push(p.layer.id.clone());
        }
        out.extend(knockout_frames(&p.children));
    }
    out
}

/// Hides the layers with these ids, wherever they are.
pub(super) fn hide(layers: &mut [crate::scene::Layer], ids: &[String]) {
    for l in layers {
        if ids.contains(&l.id) {
            l.hidden = true;
        }
        if let Some(children) = l.kind.children_mut() {
            hide(children, ids);
        }
    }
}
