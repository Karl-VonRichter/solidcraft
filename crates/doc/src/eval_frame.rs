//! Frame members (#4003): one body per straight line of a frame sketch, its cross-section a
//! profile laid square to the line (`frame::member_axes`) and run from one end to the other.

use solidcraft_geom::{Plane, Region2, Vec3};
use solidcraft_kernel as kernel;
use solidcraft_sketch::CurveKind;

use super::{ModelState, SolvedSketch, new_name, path_in_frame, select_profiles, unique_body_name, val};
use crate::expr::Kind;
use crate::frame::{self, MemberProfile};
use crate::lineage::Traced;
use crate::{DocError, Document, Feature, FeatureKind, ProfileSel, Result};

use crate::frame::MAX_MEMBERS;
/// Most profiles a profile sketch may have for its material to be worked out.
const MAX_PROFILE_PIECES: usize = 2000;
/// Shortest member (mm).
const MIN_LENGTH: f64 = 1e-6;

/// The member bodies of a `FrameMember` feature, in the order of its lines.
pub(super) fn bodies(
    doc: &Document,
    vals: &std::collections::BTreeMap<String, crate::expr::Value>,
    f: &Feature,
    st: &ModelState,
) -> Result<Vec<Traced>> {
    let FeatureKind::FrameMember { path_sketch, lines, profile, anchor, angle, mirror } = &f.kind else {
        return Err(DocError::Invalid("not a frame member feature".into()));
    };
    let ps = st.sketch(path_sketch.0).ok_or_else(|| DocError::Unknown(format!("sketch {path_sketch} (it must come earlier in the timeline)")))?;
    let mapped = path_in_frame(doc, ps, f.component)?;
    let segments = member_lines(mapped.as_ref(), lines)?;
    let mut regions = profile_regions(st, profile)?;
    if *mirror {
        regions = regions.iter().map(frame::mirrored).collect();
    }
    let at = frame::anchor_point(&regions, *anchor);
    let turn = match angle {
        Some(e) => val(vals, e, Kind::Angle)?,
        None => 0.0,
    };
    let mut out = Vec::with_capacity(segments.len());
    for (id, a, b) in segments {
        let (x, y) = frame::member_axes(b - a, turn).ok_or_else(|| DocError::Invalid(format!("line `{id}` has no length")))?;
        let plane = Plane::new(a - x * at.x - y * at.y, x, y).ok_or_else(|| DocError::Invalid(format!("line `{id}`: no plane square to it")))?;
        for body in kernel::extrude(&plane, &regions, 0.0, a.dist(b))? {
            out.push(crate::lineage::numbered_tool(body));
        }
    }
    Ok(out)
}

/// The name of a feature's `k`th member: its own stored name when it has one, else numbered after
/// the first (`Rect Tube 40x40x3 (2)`), each taken name moving on to the next free number, so a
/// second feature of the same profile numbers on from the first.
pub(super) fn body_name(st: &mut ModelState, f: &Feature, k: usize) -> String {
    if let Some(n) = f.body_names.get(k).filter(|n| !n.trim().is_empty() && st.body(n).is_none()) {
        return n.clone();
    }
    let Some(base) = f.body_names.first().map(|n| n.trim().to_string()).filter(|n| !n.is_empty()) else {
        return new_name(st, f, k);
    };
    let want = if k == 0 { base.clone() } else { format!("{base} ({})", k.saturating_add(1)) };
    if st.body(&want).is_none() { want } else { unique_body_name(st, &base) }
}

/// The member lines of the frame sketch, each as (curve id, start, end) in the feature's frame:
/// the lines asked for, or every straight line that is not construction.
pub(crate) fn member_lines(ss: &SolvedSketch, ids: &[String]) -> Result<Vec<(String, Vec3, Vec3)>> {
    let sk = &ss.sketch;
    let mut out = Vec::new();
    if ids.is_empty() {
        for (ci, c) in sk.curves.iter().enumerate() {
            if !c.construction
                && !c.centerline
                && matches!(c.kind, CurveKind::Line { .. })
                && let Some((a, b)) = planar_line(ss, ci)
            {
                out.push((c.id.clone(), a, b));
            }
        }
        for w in &sk.wires {
            if let Some((a, b)) = straight(&w.pts) {
                out.push((w.id.clone(), a, b));
            }
        }
        if out.is_empty() {
            return Err(DocError::Invalid(format!("{} has no lines for members", ss.name)));
        }
    } else {
        for id in ids {
            let line = if let Some(wi) = sk.wire_index(id) {
                sk.wires.get(wi).and_then(|w| straight(&w.pts))
            } else {
                let ci = sk.curve_index(id).ok_or_else(|| DocError::Unknown(format!("line `{id}` in {}", ss.name)))?;
                planar_line(ss, ci)
            };
            let (a, b) = line.ok_or_else(|| DocError::Invalid(format!("`{id}` is not a straight line (members run along lines)")))?;
            out.push((id.clone(), a, b));
        }
    }
    if out.len() > MAX_MEMBERS {
        return Err(DocError::Invalid(format!("at most {MAX_MEMBERS} members in one feature")));
    }
    for (id, a, b) in &out {
        if !(a.dist(*b) > MIN_LENGTH) {
            return Err(DocError::Invalid(format!("line `{id}` has no length")));
        }
    }
    Ok(out)
}

/// A sketch line's ends in the sketch's frame.
fn planar_line(ss: &SolvedSketch, ci: usize) -> Option<(Vec3, Vec3)> {
    let CurveKind::Line { a, b } = ss.sketch.curves.get(ci)?.kind else { return None };
    let (pa, pb) = (ss.sketch.points.get(a)?.pos, ss.sketch.points.get(b)?.pos);
    Some((ss.plane.to_world(pa), ss.plane.to_world(pb)))
}

/// The ends of a 3D polyline that is straight (every point within 1 µm of the line between its
/// ends).
fn straight(pts: &[Vec3]) -> Option<(Vec3, Vec3)> {
    let (a, b) = (*pts.first()?, *pts.last()?);
    let d = (b - a).normalized()?;
    pts.iter()
        .all(|p| {
            let q = *p - a;
            (q - d * q.dot(d)).len() <= 1e-3
        })
        .then_some((a, b))
}

/// The regions of a member's profile, in the profile's own coordinates: one piece of material,
/// holes and all.
fn profile_regions(st: &ModelState, profile: &MemberProfile) -> Result<Vec<Region2>> {
    let regions = match profile {
        MemberProfile::Library { profile } => vec![profile.region()?],
        MemberProfile::Sketch { sketch, profiles } => {
            let ss = st.sketch(sketch.0).ok_or_else(|| DocError::Unknown(format!("sketch {sketch} (it must come earlier in the timeline)")))?;
            let picked = match profiles {
                ProfileSel::All => material(ss)?,
                sel => select_profiles(ss, sel)?,
            };
            solidcraft_sketch::merge_regions(&picked)
        }
    };
    match regions.len() {
        1 => Ok(regions),
        0 => Err(DocError::Invalid("the member profile has no area".into())),
        n => Err(DocError::Invalid(format!("the member profile is {n} separate pieces (pick one with `profiles`)"))),
    }
}

/// The material of a profile sketch as nested loops alternate: a profile inside an even number of
/// other profiles' outlines is material, inside an odd number a hole.
fn material(ss: &SolvedSketch) -> Result<Vec<Region2>> {
    let ps = &ss.profiles;
    if ps.is_empty() {
        return Err(DocError::Invalid(format!("{} has no closed profiles", ss.name)));
    }
    if ps.len() > MAX_PROFILE_PIECES {
        return Err(DocError::Invalid(format!("{} has more than {MAX_PROFILE_PIECES} profiles (pick the member's with `profiles`)", ss.name)));
    }
    Ok(ps
        .iter()
        .enumerate()
        .filter(|(i, p)| {
            let at = p.region.interior_point();
            let depth = ps.iter().enumerate().filter(|(j, q)| j != i && q.region.outer.contains(at)).count();
            depth % 2 == 0
        })
        .map(|(_, p)| p.region.clone())
        .collect())
}
