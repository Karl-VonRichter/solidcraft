//! Frames and weldments (#4003): the profile library structural members are made of.
//! `frame.profile.list` lists the built-in profiles, `frame.profile.properties` works out a
//! profile's section, and `frame.profile.insert` draws one into a sketch.

use serde_json::{Value, json};
use solidcraft_doc::frame::{self, Anchor, Profile};
use solidcraft_geom::{Loop2, Seg2, Vec2};
use solidcraft_sketch::Sketch;

use super::sketch::edit;
use super::{CommandSpec, in_sketch};
use crate::params::{bad, bool_, num, vec2};
use crate::{Result, Session};

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;

/// The profile parameter, as every frame command documents it.
macro_rules! profile_doc {
    () => {
        "profile: {kind: bar|round_bar|rect_tube|round_tube|angle|channel|i_beam, …its dimensions in mm} (frame.profile.list gives each kind's dimensions and an example; dimensions left out take defaults from the profile's size)"
    };
}

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new("frame.profile.list", "Profile Library", list).icon("library")
        .noundo()
        .doc_only()
        .params("Returns kinds: [{kind, label, required, optional, example}]: the built-in profiles, each SolidCraft's own parametric definition"),
    CommandSpec::new("frame.profile.properties", "Profile Properties", properties).icon("measure").noundo().doc_only().params(concat!(
        profile_doc!(),
        "; density?: kg/m³ (adds mass_per_m, kg/m). Returns profile (every default filled in), area (mm²), centroid [x,y] (mm, from the bounding box centre), ixx, iyy, ixy (mm⁴ about the centroid), width, height"
    )),
    CommandSpec::new("frame.profile.insert", "Insert Profile", insert).icon("import").enabled(in_sketch).params(concat!(
        profile_doc!(),
        "; at?: [x,y] (sketch mm, default the origin); anchor?: center|centroid|top_left|top|top_right|left|right|bottom_left|bottom|bottom_right (the profile point placed on `at`, default center); angle?: radians, counter-clockwise; fixed?: bool (default true: the curves are locked, so solving the sketch keeps the shape); sketch?: id|name (default: the sketch being edited). Returns the section as frame.profile.properties does",
        created_doc!()
    )),
];

fn profile(cmd: &str, p: &Value) -> Result<Profile> {
    let v = p.get("profile").ok_or_else(|| bad(cmd, "`profile` is required (frame.profile.list gives the kinds)"))?;
    serde_json::from_value(v.clone()).map_err(|e| bad(cmd, format!("`profile`: {e}")))
}

fn list(_: &mut Session, _: &Value) -> Result<Value> {
    Ok(json!({"kinds": frame::profile_kinds()}))
}

/// The profile's section as the commands report it.
fn section_json(cmd: &str, profile: &Profile, density: Option<f64>) -> Result<Value> {
    let region = profile.region().map_err(|e| bad(cmd, e.to_string()))?;
    let s = frame::section(&region);
    let mut out = json!({
        "profile": profile.filled(),
        "area": s.area,
        "centroid": s.centroid,
        "ixx": s.ixx,
        "iyy": s.iyy,
        "ixy": s.ixy,
        "width": s.width,
        "height": s.height,
    });
    if let (Some(rho), Some(o)) = (density, out.as_object_mut()) {
        // mm² × 1e-6 m²/mm² × kg/m³ = kg/m.
        o.insert("mass_per_m".into(), json!(s.area * 1e-6 * rho));
    }
    Ok(out)
}

fn properties(_: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "frame.profile.properties";
    let profile = profile(cmd, p)?;
    let density = match p.get("density") {
        None | Some(Value::Null) => None,
        Some(_) => Some(
            num(p, "density")
                .filter(|d| *d > 0.0 && *d <= 1e6)
                .ok_or_else(|| bad(cmd, "`density` must be a positive number of kg/m³ (at most 1e6)"))?,
        ),
    };
    section_json(cmd, &profile, density)
}

fn insert(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "frame.profile.insert";
    let profile = profile(cmd, p)?;
    let at = match p.get("at") {
        None | Some(Value::Null) => Vec2::ZERO,
        Some(v) => vec2(v).ok_or_else(|| bad(cmd, "`at` must be [x, y] in sketch millimetres"))?,
    };
    let anchor: Anchor = match p.get("anchor") {
        None | Some(Value::Null) => Anchor::default(),
        Some(v) => serde_json::from_value(v.clone())
            .map_err(|_| bad(cmd, "`anchor` must be center, centroid, top_left, top, top_right, left, right, bottom_left, bottom or bottom_right"))?,
    };
    let angle = match p.get("angle") {
        None | Some(Value::Null) => 0.0,
        Some(_) => num(p, "angle").filter(|a| a.abs() <= 1e6).ok_or_else(|| bad(cmd, "`angle` must be a number of radians"))?,
    };
    let fixed = match p.get("fixed") {
        None | Some(Value::Null) => true,
        Some(_) => bool_(p, "fixed").ok_or_else(|| bad(cmd, "`fixed` must be true or false"))?,
    };
    let region = profile.region().map_err(|e| bad(cmd, e.to_string()))?;
    let s0 = frame::section(&region);
    let (w, h) = (s0.width, s0.height);
    let placed = frame::placed(&region, anchor.point(w, h, Vec2::new(s0.centroid[0], s0.centroid[1])), at, angle);
    let section = section_json(cmd, &profile, None)?;
    let (created, info) = edit(s, p, cmd, false, |sk, _| {
        let before = sk.clone();
        for l in std::iter::once(&placed.outer).chain(&placed.holes) {
            add_loop(sk, l, fixed, cmd)?;
        }
        Ok(super::sketch_created(Some(&before), sk))
    })?;
    Ok(json!({"created": created, "section": section, "sketch": info}))
}

/// Draw the closed loop `l` (lines and arcs) into the sketch, each segment sharing its end
/// points with its neighbours; a loop of two half arcs of one circle becomes a circle.
fn add_loop(sk: &mut Sketch, l: &Loop2, fixed: bool, cmd: &str) -> Result<()> {
    let mut curves = Vec::new();
    if let [Seg2::Arc { center: c0, radius: r0, sweep: s0, .. }, Seg2::Arc { center: c1, radius: r1, sweep: s1, .. }] = l.segs.as_slice()
        && c0.dist(*c1) <= 1e-9
        && (r0 - r1).abs() <= 1e-9
        && ((s0 + s1).abs() - std::f64::consts::TAU).abs() <= 1e-9
    {
        curves.push(sk.add_circle(*c0, *r0, None, None)?);
    } else {
        let pts = l.segs.iter().map(|g| sk.add_point(g.start(), None)).collect::<std::result::Result<Vec<_>, _>>()?;
        let n = pts.len();
        for (i, g) in l.segs.iter().enumerate() {
            let (Some(&a), Some(&b)) = (pts.get(i), pts.get((i + 1) % n)) else {
                return Err(bad(cmd, "a profile segment is missing its end"));
            };
            let c = match *g {
                Seg2::Line { .. } => sk.add_line_pts(a, b, None)?,
                // The sketch's arcs run counter-clockwise: a clockwise one is drawn from its end.
                Seg2::Arc { center, sweep, .. } if sweep >= 0.0 => sk.add_arc(center, g.start(), g.end(), [None, Some(a), Some(b)], None)?,
                Seg2::Arc { center, .. } => sk.add_arc(center, g.end(), g.start(), [None, Some(b), Some(a)], None)?,
                _ => return Err(bad(cmd, "not supported yet: profiles with free-form edges")),
            };
            curves.push(c);
        }
    }
    if fixed {
        for c in curves {
            if let Some(cu) = sk.curves.get_mut(c) {
                cu.fixed = true;
            }
        }
    }
    Ok(())
}
