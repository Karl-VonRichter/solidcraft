//! Profile library tests. Expected values are worked out by hand: rectangles and circles, and
//! composite rectangles by the parallel-axis theorem; each fillet adds or removes a spandrel of
//! (1 − π/4) r².
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;
use std::f64::consts::PI;

const SPANDREL: f64 = 1.0 - PI / 4.0;

fn close(got: f64, want: f64, what: &str) {
    let tol = 1e-9 * (1.0 + want.abs());
    assert!((got - want).abs() <= tol, "{what}: got {got}, want {want}");
}

fn profile(json: serde_json::Value) -> Profile {
    serde_json::from_value(json).unwrap()
}

fn sec(p: &Profile) -> Section {
    let r = p.region().unwrap();
    assert!(r.outer.is_closed(1e-9), "{} outline is closed", p.kind());
    assert!(r.outer.is_ccw(), "{} outline runs counter-clockwise", p.kind());
    for h in &r.holes {
        assert!(h.is_closed(1e-9) && !h.is_ccw(), "{} holes are closed and clockwise", p.kind());
    }
    // The exact boundary integral agrees with the region's own area.
    let s = section(&r);
    close(s.area, r.area(), "area against Region2::area");
    s
}

/// Area, centroid (in the profile's centred frame) and centroidal Ixx, Iyy of rectangles
/// given as (x0, y0, x1, y1), combined.
fn rects(parts: &[(f64, f64, f64, f64)]) -> (f64, f64, f64, f64, f64) {
    let a: f64 = parts.iter().map(|&(x0, y0, x1, y1)| (x1 - x0) * (y1 - y0)).sum();
    let cx = parts.iter().map(|&(x0, y0, x1, y1)| (x1 - x0) * (y1 - y0) * (x0 + x1) / 2.0).sum::<f64>() / a;
    let cy = parts.iter().map(|&(x0, y0, x1, y1)| (x1 - x0) * (y1 - y0) * (y0 + y1) / 2.0).sum::<f64>() / a;
    let ixx = parts
        .iter()
        .map(|&(x0, y0, x1, y1)| {
            let (b, h) = (x1 - x0, y1 - y0);
            b * h * h * h / 12.0 + b * h * ((y0 + y1) / 2.0 - cy).powi(2)
        })
        .sum();
    let iyy = parts
        .iter()
        .map(|&(x0, y0, x1, y1)| {
            let (b, h) = (x1 - x0, y1 - y0);
            h * b * b * b / 12.0 + b * h * ((x0 + x1) / 2.0 - cx).powi(2)
        })
        .sum();
    (a, cx, cy, ixx, iyy)
}

fn check_rects(s: &Section, parts: &[(f64, f64, f64, f64)], what: &str) {
    let (a, cx, cy, ixx, iyy) = rects(parts);
    close(s.area, a, &format!("{what} area"));
    close(s.centroid[0], cx, &format!("{what} centroid x"));
    close(s.centroid[1], cy, &format!("{what} centroid y"));
    close(s.ixx, ixx, &format!("{what} Ixx"));
    close(s.iyy, iyy, &format!("{what} Iyy"));
}

#[test]
fn bar_is_a_rectangle() {
    let s = sec(&profile(serde_json::json!({"kind": "bar", "width": 40, "height": 10})));
    close(s.area, 400.0, "area");
    close(s.ixx, 40.0 * 1000.0 / 12.0, "Ixx = b h³ / 12");
    close(s.iyy, 10.0 * 64_000.0 / 12.0, "Iyy = h b³ / 12");
    close(s.ixy, 0.0, "Ixy");
    close(s.width, 40.0, "width");
    close(s.height, 10.0, "height");
}

#[test]
fn rounded_bar_loses_four_spandrels() {
    let s = sec(&profile(serde_json::json!({"kind": "bar", "width": 30, "height": 20, "corner_radius": 4})));
    close(s.area, 600.0 - 4.0 * SPANDREL * 16.0, "area");
    // Fully rounded: a 20 × 20 bar with radius 10 is a circle.
    let s = sec(&profile(serde_json::json!({"kind": "bar", "width": 20, "height": 20, "corner_radius": 10})));
    close(s.area, PI * 100.0, "fully rounded area");
    close(s.ixx, PI * 10f64.powi(4) / 4.0, "fully rounded Ixx");
}

#[test]
fn round_bar_and_tube() {
    let s = sec(&Profile::RoundBar { diameter: 20.0 });
    close(s.area, PI * 100.0, "bar area");
    close(s.ixx, PI * 20f64.powi(4) / 64.0, "bar Ixx = π d⁴ / 64");
    close(s.iyy, s.ixx, "bar Iyy");
    let s = sec(&Profile::RoundTube { diameter: 42.4, wall: 3.2 });
    let d = 42.4 - 6.4;
    close(s.area, PI / 4.0 * (42.4f64.powi(2) - d * d), "tube area");
    close(s.ixx, PI / 64.0 * (42.4f64.powi(4) - d.powi(4)), "tube Ixx = π (D⁴ − d⁴) / 64");
}

#[test]
fn rect_tube_sharp_and_rounded() {
    let s = sec(&profile(serde_json::json!({"kind": "rect_tube", "width": 60, "height": 40, "wall": 3, "outer_radius": 0})));
    close(s.area, 60.0 * 40.0 - 54.0 * 34.0, "sharp area");
    close(s.ixx, (60.0 * 40f64.powi(3) - 54.0 * 34f64.powi(3)) / 12.0, "sharp Ixx");
    close(s.iyy, (40.0 * 60f64.powi(3) - 34.0 * 54f64.powi(3)) / 12.0, "sharp Iyy");
    // Defaults: outer radius 2 t = 6, inner radius 3.
    let p = profile(serde_json::json!({"kind": "rect_tube", "width": 40, "height": 40, "wall": 3}));
    assert_eq!(p.filled(), Profile::RectTube { width: 40.0, height: 40.0, wall: 3.0, outer_radius: Some(6.0) });
    let s = sec(&p);
    let outer = 1600.0 - 4.0 * SPANDREL * 36.0;
    let inner = 34.0 * 34.0 - 4.0 * SPANDREL * 9.0;
    close(s.area, outer - inner, "rounded area");
    close(s.centroid[0], 0.0, "centroid x");
    close(s.ixy, 0.0, "Ixy");
}

#[test]
fn angle_sharp_matches_two_rectangles() {
    let (a, b, t) = (60.0, 40.0, 5.0);
    let s = sec(&profile(serde_json::json!({"kind": "angle", "width": a, "height": b, "thickness": t, "root_radius": 0, "toe_radius": 0})));
    let (x0, y0) = (-a / 2.0, -b / 2.0);
    check_rects(&s, &[(x0, y0, x0 + a, y0 + t), (x0, y0 + t, x0 + t, y0 + b)], "angle");
    assert!(s.ixy.abs() > 1.0, "an unequal angle has a product of inertia");
}

#[test]
fn angle_root_adds_and_toes_remove_spandrels() {
    let (a, b, t) = (50.0, 50.0, 5.0);
    let p = profile(serde_json::json!({"kind": "angle", "width": a, "height": b, "thickness": t}));
    let s = sec(&p);
    // Defaults: root radius t, toe radius t / 2.
    close(s.area, t * (a + b - t) + SPANDREL * t * t - 2.0 * SPANDREL * (t / 2.0).powi(2), "area");
    // An equal angle is symmetric about its diagonal.
    close(s.centroid[0], s.centroid[1], "centroid on the diagonal");
    close(s.ixx, s.iyy, "Ixx = Iyy");
}

#[test]
fn channel_and_i_beam_sharp_match_rectangles() {
    let (h, b, tw, tf) = (100.0, 50.0, 6.0, 8.0);
    let s = sec(&profile(serde_json::json!({"kind": "channel", "height": h, "width": b, "web": tw, "flange": tf, "root_radius": 0})));
    let (x0, y0) = (-b / 2.0, -h / 2.0);
    let parts = [(x0, y0, x0 + b, y0 + tf), (x0, y0 + tf, x0 + tw, y0 + h - tf), (x0, y0 + h - tf, x0 + b, y0 + h)];
    check_rects(&s, &parts, "channel");
    close(s.ixy, 0.0, "channel Ixy");

    let (h, b, tw, tf) = (100.0, 50.0, 4.5, 7.0);
    let s = sec(&profile(serde_json::json!({"kind": "i_beam", "height": h, "width": b, "web": tw, "flange": tf, "root_radius": 0})));
    close(s.area, 2.0 * b * tf + (h - 2.0 * tf) * tw, "I area");
    close(s.ixx, (b * h.powi(3) - (b - tw) * (h - 2.0 * tf).powi(3)) / 12.0, "I Ixx");
    close(s.iyy, (2.0 * tf * b.powi(3) + (h - 2.0 * tf) * tw.powi(3)) / 12.0, "I Iyy");
}

#[test]
fn i_beam_root_radii_add_four_spandrels() {
    let (h, b, tw, tf, r) = (100.0, 50.0, 4.5, 7.0, 7.0);
    let s = sec(&profile(serde_json::json!({"kind": "i_beam", "height": h, "width": b, "web": tw, "flange": tf, "root_radius": r})));
    close(s.area, 2.0 * b * tf + (h - 2.0 * tf) * tw + 4.0 * SPANDREL * r * r, "area");
    close(s.centroid[0], 0.0, "centroid x");
    close(s.centroid[1], 0.0, "centroid y");
}

/// A T-slot profile's area: the box less its rounded corners, its slots and its bores.
fn t_slot_area(w: f64, h: f64, slots: f64, bores: f64) -> f64 {
    let s = w.min(h);
    let (o, lip, u, d, rb, rc) = (0.22 * s, 0.1 * s, 0.4 * s, 0.27 * s, 0.09 * s, 0.02 * s);
    w * h - 4.0 * SPANDREL * rc * rc - slots * (o * lip + u * (d - lip)) - bores * PI * rb * rb
}

#[test]
fn t_slot_defaults_from_its_size() {
    let p = profile(serde_json::json!({"kind": "t_slot", "width": 45}));
    let s = sec(&p);
    close(s.area, t_slot_area(45.0, 45.0, 4.0, 1.0), "45 × 45 area");
    close(s.width, 45.0, "width");
    close(s.height, 45.0, "height");
    close(s.centroid[0], 0.0, "centroid x");
    close(s.centroid[1], 0.0, "centroid y");
    close(s.ixx, s.iyy, "a square profile bends alike both ways");
    // 45 × 60: one slot on each face (60 / 45 rounds to one module), one bore.
    let s = sec(&profile(serde_json::json!({"kind": "t_slot", "width": 45, "height": 60})));
    close(s.area, t_slot_area(45.0, 60.0, 4.0, 1.0), "45 × 60 area");
    // 45 × 90: two modules, so two slots on each long face and two bores.
    let s = sec(&profile(serde_json::json!({"kind": "t_slot", "width": 45, "height": 90})));
    close(s.area, t_slot_area(45.0, 90.0, 6.0, 2.0), "45 × 90 area");
    close(s.centroid[1], 0.0, "45 × 90 centroid y");
    // A solid core.
    let s = sec(&profile(serde_json::json!({"kind": "t_slot", "width": 20, "bore": 0})));
    close(s.area, t_slot_area(20.0, 20.0, 4.0, 0.0), "solid core area");
}

#[test]
fn every_kind_example_builds() {
    for k in profile_kinds() {
        assert_eq!(k.example.kind(), k.kind);
        let s = sec(&k.example);
        assert!(s.area > 0.0 && s.ixx > 0.0 && s.iyy > 0.0, "{}: {s:?}", k.kind);
        let (w, h) = k.example.size();
        close(s.width, w, &format!("{} width", k.kind));
        close(s.height, h, &format!("{} height", k.kind));
    }
}

#[test]
fn dimensions_that_do_not_fit_are_refused() {
    let bad = [
        serde_json::json!({"kind": "bar", "width": f64::MAX, "height": 1}),
        serde_json::json!({"kind": "bar", "width": -1, "height": 1}),
        serde_json::json!({"kind": "bar", "width": 0, "height": 1}),
        serde_json::json!({"kind": "bar", "width": 10, "height": 10, "corner_radius": 6}),
        serde_json::json!({"kind": "round_bar", "diameter": 1e300}),
        serde_json::json!({"kind": "rect_tube", "width": 10, "height": 10, "wall": 5}),
        serde_json::json!({"kind": "rect_tube", "width": 10, "height": 10, "wall": 1, "outer_radius": -1}),
        serde_json::json!({"kind": "round_tube", "diameter": 10, "wall": 5}),
        serde_json::json!({"kind": "angle", "width": 10, "height": 10, "thickness": 10}),
        serde_json::json!({"kind": "angle", "width": 10, "height": 10, "thickness": 2, "root_radius": 8}),
        serde_json::json!({"kind": "angle", "width": 10, "height": 10, "thickness": 2, "toe_radius": 3}),
        serde_json::json!({"kind": "channel", "height": 10, "width": 10, "web": 2, "flange": 5}),
        serde_json::json!({"kind": "channel", "height": 10, "width": 10, "web": 10, "flange": 2}),
        serde_json::json!({"kind": "i_beam", "height": 100, "width": 50, "web": 5, "flange": 7, "root_radius": 30}),
        serde_json::json!({"kind": "t_slot", "width": 45, "opening": 20}),
        serde_json::json!({"kind": "t_slot", "width": 45, "lip": 13}),
        serde_json::json!({"kind": "t_slot", "width": 45, "cavity_width": 30}),
        serde_json::json!({"kind": "t_slot", "width": 45, "cavity_depth": 20}),
        serde_json::json!({"kind": "t_slot", "width": 45, "bore": 30}),
        serde_json::json!({"kind": "t_slot", "width": 45, "corner_radius": 18}),
        serde_json::json!({"kind": "t_slot", "width": 10000, "height": 10000, "pitch": 1, "opening": 0.2, "lip": 0.05, "cavity_width": 0.3, "cavity_depth": 0.1, "bore": 0}),
    ];
    for j in bad {
        let p: Profile = serde_json::from_value(j.clone()).unwrap();
        assert!(p.region().is_err(), "{j} is refused");
    }
    for j in [
        serde_json::json!({"kind": "bar", "width": 1}),
        serde_json::json!({"kind": "bar", "width": 1, "height": 1, "colour": 2}),
        serde_json::json!({"kind": "hexagon"}),
    ] {
        assert!(serde_json::from_value::<Profile>(j.clone()).is_err(), "{j} is not a profile");
    }
}

#[test]
fn non_finite_dimensions_are_refused() {
    for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(Profile::RoundBar { diameter: v }.region().is_err());
        assert!(Profile::RectTube { width: 40.0, height: 40.0, wall: 3.0, outer_radius: Some(v) }.region().is_err());
        let t = Profile::TSlot {
            width: 45.0,
            height: None,
            opening: None,
            lip: None,
            cavity_width: None,
            cavity_depth: Some(v),
            bore: None,
            corner_radius: None,
            pitch: None,
        };
        assert!(t.region().is_err());
    }
}

#[test]
fn placing_moves_the_anchor_onto_the_point() {
    let p = profile(serde_json::json!({"kind": "angle", "width": 60, "height": 40, "thickness": 5}));
    let r = p.region().unwrap();
    let s0 = section(&r);
    let (w, h) = p.size();
    let anchor = Anchor::BottomLeft.point(w, h, Vec2::new(s0.centroid[0], s0.centroid[1]));
    let at = Vec2::new(100.0, 50.0);
    let moved = placed(&r, anchor, at, PI / 2.0);
    let s1 = section(&moved);
    close(s1.area, s0.area, "area is kept");
    // Turned a quarter: the heel stays on `at`, the 60 mm leg now runs up and the 40 mm one left.
    close(s1.width, 40.0, "turned width");
    close(s1.height, 60.0, "turned height");
    let c = at + Vec2::new(-(s0.centroid[1] + 20.0), s0.centroid[0] + 30.0);
    close(s1.centroid[0], c.x, "centroid x");
    close(s1.centroid[1], c.y, "centroid y");
    close(s1.ixx, s0.iyy, "Ixx after a quarter turn");
    let centre = Anchor::Centroid.point(w, h, Vec2::new(s0.centroid[0], s0.centroid[1]));
    let s2 = section(&placed(&r, centre, Vec2::ZERO, 0.0));
    close(s2.centroid[0], 0.0, "centroid anchor x");
    close(s2.centroid[1], 0.0, "centroid anchor y");
}
