//! The moments a moving scene's preview shows.

use super::shots::{length, timeline};
use crate::scene::Scene;

/// Up to `n` moments worth looking at in a moving scene, sorted, for its
/// preview: each shot's middle, then the loop's seam (its last frame) when
/// it loops or something leaves, then the middle of each entrance and each
/// keyframe track (where it's biggest when it grows: a `pop`'s overshoot, a
/// pulse), earliest first, then even steps. None closer than a
/// quarter second to another. Empty for a still scene.
pub fn moments(scene: &Scene, n: usize) -> Vec<f32> {
    fn motions(layers: &[crate::scene::Layer], offset: f32, out: &mut Vec<f32>, leaves: &mut bool) {
        for l in layers {
            // An entrance at its middle, or where it's biggest when it
            // overshoots (a `pop`), where a frame would cut it.
            if let Some(m) = &l.time.enter {
                let a = m.at.unwrap_or(0.0);
                let at = (0..=16)
                    .map(|i| a + m.duration * i as f32 / 16.0)
                    .map(|t| (t, m.enter(t).scale))
                    .filter(|(_, s)| *s > 1.001)
                    .max_by(|x, y| x.1.total_cmp(&y.1))
                    .map_or(a + m.duration / 2.0, |(t, _)| t);
                out.push(offset + at);
            }
            *leaves |= l.time.out.is_some();
            for t in l
                .time
                .animate
                .iter()
                .flat_map(crate::scene::OneOrMany::as_slice)
            {
                // A scale track where it's biggest; others at their middle.
                let seed = super::track::seed(&l.id);
                let at = (0..=16)
                    .map(|i| t.at + t.duration * i as f32 / 16.0)
                    .filter_map(|at| {
                        match t.value("scale", super::track::Val::Num(1.0), at, seed) {
                            Some(super::track::Val::Num(s)) if s > 1.001 => Some((at, s)),
                            _ => None,
                        }
                    })
                    .max_by(|x, y| x.1.total_cmp(&y.1))
                    .map_or(t.at + t.duration / 2.0, |(at, _)| at);
                out.push(offset + at);
            }
            if let Some(children) = l.kind.children() {
                motions(children, offset, out, leaves);
            }
        }
    }
    let Some(len) = length(scene).filter(|l| *l > 0.0) else {
        return Vec::new();
    };
    let shots = timeline(scene);
    let mut wanted: Vec<f32> = shots
        .iter()
        .map(|(_, start, dur, _)| start + dur / 2.0)
        .collect();
    // Each shot's own clock starts with the shot.
    let start_of = |i: usize| shots.iter().find(|s| s.0 == i).map_or(0.0, |s| s.1);
    let (mut moving, mut leaves) = (Vec::new(), false);
    for (i, l) in scene.layers.iter().enumerate() {
        motions(
            std::slice::from_ref(l),
            start_of(i),
            &mut moving,
            &mut leaves,
        );
    }
    if scene.looping || leaves {
        wanted.push(len - 1.0 / scene.fps.max(1.0));
    }
    moving.sort_by(f32::total_cmp);
    wanted.extend(moving);
    let steps = n.max(1);
    wanted.extend((0..steps).map(|k| len * (k as f32 + 0.5) / steps as f32));
    let mut out: Vec<f32> = Vec::new();
    for t in wanted {
        if out.len() == n {
            break;
        }
        if (0.0..len).contains(&t) && out.iter().all(|o| (o - t).abs() >= 0.25) {
            out.push(t);
        }
    }
    out.sort_by(f32::total_cmp);
    out
}

#[cfg(test)]
mod tests {
    use super::moments;
    use serde_json::json;

    fn scene(v: &serde_json::Value) -> crate::scene::Scene {
        let mut base = json!({"width": 100, "height": 100, "sizes": [{"id": "a", "width": 100, "height": 100}]});
        base.as_object_mut()
            .unwrap()
            .extend(v.as_object().unwrap().clone());
        serde_json::from_value(base).unwrap()
    }

    #[test]
    fn every_shot_is_sampled_even_a_short_one() {
        let shot = |id: &str, d: f32| json!({"id": id, "type": "frame", "width": "fill", "height": "fill", "shot": {"duration": d}});
        let s = scene(&json!({"layers": [shot("s1", 2.0), shot("s2", 1.5), shot("s3", 4.5)]}));
        let m = moments(&s, 6);
        assert!(
            m.iter().any(|t| (2.0..3.5).contains(t)),
            "the 1.5 s shot: {m:?}"
        );
        assert_eq!(m.len(), 6, "{m:?}");
        assert!(m.windows(2).all(|w| w[1] - w[0] >= 0.25), "{m:?}");
    }

    #[test]
    fn a_loop_shows_its_seam_and_its_tracks_midpoints() {
        let s = scene(&json!({"duration": 5, "loop": true, "layers": [
            {"id": "ring", "type": "ellipse", "width": 50, "height": 50, "animate": {"draw": [0, 1], "delay": 1.4, "duration": 1.6}}]}));
        let m = moments(&s, 6);
        assert!(
            m.iter().any(|t| (t - (5.0 - 1.0 / 30.0)).abs() < 0.01),
            "the seam: {m:?}"
        );
        assert!(
            m.iter().any(|t| (t - 2.2).abs() < 0.01),
            "the ring mid-draw: {m:?}"
        );
        assert!(
            moments(&scene(&json!({"layers": []})), 6).is_empty(),
            "a still"
        );
    }

    #[test]
    fn a_pop_is_sampled_where_it_overshoots() {
        let s: crate::scene::Scene =
            serde_json::from_value(json!({"width": 100, "height": 100, "duration": 6,
            "sizes": [{"id": "a", "width": 100, "height": 100}],
            "layers": [{"id": "ticket", "type": "rect", "width": 50, "height": 50,
                "enter": {"effect": "pop", "delay": 2.1, "duration": 0.6}}]}))
            .unwrap();
        let m = moments(&s, 6);
        let pop = s.layers[0].time.enter.clone().unwrap();
        let biggest = m
            .iter()
            .map(|t| pop.enter(*t).scale)
            .fold(0.0_f32, f32::max);
        assert!(biggest > 1.01, "a moment shows the overshoot: {m:?}");
    }
}
