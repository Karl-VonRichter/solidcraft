//! Frames: the profile library structural members are made of (#4003).
//!
//! A profile is a closed cross-section, an outline and its holes, built in code from a few
//! dimensions ([`Profile`]). Every built-in profile is SolidCraft's own parametric definition:
//! no vendor's or standard's size table is shipped. A dimension left out takes a default worked
//! out from the profile's size, so `{"kind": "t_slot", "width": 45}` is a complete profile.
//!
//! Coordinates are millimetres in the profile's own plane, x across and y up, with the centre
//! of the profile's bounding box at the origin. The outline runs counter-clockwise and holes
//! clockwise, as [`Region2`] expects.

use serde::{Deserialize, Serialize};
use solidcraft_geom::{Loop2, Region2, Seg2, Vec2};

use crate::{DocError, Result};

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;

/// The largest profile dimension (mm).
pub const MAX_PROFILE_SIZE: f64 = 10_000.0;
/// The smallest profile dimension (mm): walls, radii and openings thinner than this are refused.
pub const MIN_PROFILE_DIMENSION: f64 = 0.01;
/// The thinnest material a T-slot profile keeps between its cavities, its bores and its faces (mm).
pub const MIN_T_SLOT_WALL: f64 = 0.2;
/// The most slots a T-slot profile has along one side.
pub const MAX_SLOTS_PER_SIDE: usize = 64;

/// A built-in profile and its dimensions (mm). `Option` dimensions take a default worked out from
/// the profile's size ([`Profile::filled`] says which).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Profile {
    /// A solid rectangle (flat bar, square bar), its corners optionally rounded.
    Bar {
        width: f64,
        height: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        corner_radius: Option<f64>,
    },
    /// A solid circle.
    RoundBar { diameter: f64 },
    /// A rectangular or square hollow section. The outer corner radius defaults to twice the
    /// wall; the inner radius is the outer one less the wall.
    RectTube {
        width: f64,
        height: f64,
        wall: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        outer_radius: Option<f64>,
    },
    /// A round hollow section (pipe or tube).
    RoundTube { diameter: f64, wall: f64 },
    /// An L angle: one leg `width` along x, the other `height` along y, both `thickness` thick,
    /// the heel at the bottom left. The root radius defaults to the thickness, the toe radius to
    /// half of it.
    Angle {
        width: f64,
        height: f64,
        thickness: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        root_radius: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        toe_radius: Option<f64>,
    },
    /// A channel with parallel flanges, open towards +x: the web `web` thick along the left side,
    /// flanges `width` long and `flange` thick. The root radius defaults to the web thickness, the
    /// toe radius to none.
    Channel {
        height: f64,
        width: f64,
        web: f64,
        flange: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        root_radius: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        toe_radius: Option<f64>,
    },
    /// An I or H beam with parallel flanges. The root radius defaults to the web thickness.
    IBeam {
        height: f64,
        width: f64,
        web: f64,
        flange: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        root_radius: Option<f64>,
    },
    /// A generic T-slot extrusion, drawn as a solid section: each face has T-shaped slots (an
    /// opening through a lip, then a wider cavity), with a bore in the middle of each module.
    /// `height` defaults to `width`; every other dimension defaults to a fixed fraction of the
    /// smaller side `S`: opening 0.22 S, lip 0.1 S, cavity width 0.4 S, cavity depth 0.27 S,
    /// bore 0.18 S, corner radius 0.02 S, and one module (one slot per face and one bore) every
    /// `pitch` = S along a longer side. A bore of 0 leaves the core solid.
    TSlot {
        width: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        height: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        opening: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        lip: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cavity_width: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cavity_depth: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bore: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        corner_radius: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pitch: Option<f64>,
    },
}

/// One built-in profile kind, for listing.
#[derive(Clone, Debug, Serialize)]
pub struct ProfileKind {
    pub kind: &'static str,
    pub label: &'static str,
    /// The dimensions it needs.
    pub required: &'static [&'static str],
    /// The dimensions that default when left out.
    pub optional: &'static [&'static str],
    /// A complete example, as JSON.
    pub example: Profile,
}

/// Every built-in profile kind.
pub fn profile_kinds() -> Vec<ProfileKind> {
    vec![
        ProfileKind {
            kind: "bar",
            label: "Bar",
            required: &["width", "height"],
            optional: &["corner_radius"],
            example: Profile::Bar { width: 40.0, height: 10.0, corner_radius: None },
        },
        ProfileKind { kind: "round_bar", label: "Round Bar", required: &["diameter"], optional: &[], example: Profile::RoundBar { diameter: 20.0 } },
        ProfileKind {
            kind: "rect_tube",
            label: "Rectangular Tube",
            required: &["width", "height", "wall"],
            optional: &["outer_radius"],
            example: Profile::RectTube { width: 40.0, height: 40.0, wall: 3.0, outer_radius: None },
        },
        ProfileKind {
            kind: "round_tube",
            label: "Round Tube",
            required: &["diameter", "wall"],
            optional: &[],
            example: Profile::RoundTube { diameter: 42.4, wall: 3.2 },
        },
        ProfileKind {
            kind: "angle",
            label: "Angle",
            required: &["width", "height", "thickness"],
            optional: &["root_radius", "toe_radius"],
            example: Profile::Angle { width: 50.0, height: 50.0, thickness: 5.0, root_radius: None, toe_radius: None },
        },
        ProfileKind {
            kind: "channel",
            label: "Channel",
            required: &["height", "width", "web", "flange"],
            optional: &["root_radius", "toe_radius"],
            example: Profile::Channel { height: 100.0, width: 50.0, web: 6.0, flange: 8.0, root_radius: None, toe_radius: None },
        },
        ProfileKind {
            kind: "i_beam",
            label: "I-Beam",
            required: &["height", "width", "web", "flange"],
            optional: &["root_radius"],
            example: Profile::IBeam { height: 100.0, width: 50.0, web: 4.5, flange: 7.0, root_radius: None },
        },
        ProfileKind {
            kind: "t_slot",
            label: "T-Slot Extrusion",
            required: &["width"],
            optional: &["height", "opening", "lip", "cavity_width", "cavity_depth", "bore", "corner_radius", "pitch"],
            example: Profile::TSlot {
                width: 45.0,
                height: None,
                opening: None,
                lip: None,
                cavity_width: None,
                cavity_depth: None,
                bore: None,
                corner_radius: None,
                pitch: None,
            },
        },
    ]
}

/// A point of a profile's bounding box (or its centroid) that sits on a member's line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Anchor {
    #[default]
    Center,
    Centroid,
    TopLeft,
    Top,
    TopRight,
    Left,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

/// A profile's section properties (mm², mm, mm⁴). Second moments are about axes through the
/// centroid, parallel to the profile's x and y.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Section {
    pub area: f64,
    pub centroid: [f64; 2],
    /// ∫ y² dA about the centroid: bending about the x axis.
    pub ixx: f64,
    /// ∫ x² dA about the centroid: bending about the y axis.
    pub iyy: f64,
    pub ixy: f64,
    /// The bounding box's size.
    pub width: f64,
    pub height: f64,
}

fn invalid(msg: impl Into<String>) -> DocError {
    DocError::Invalid(msg.into())
}

/// `v` as a dimension: finite, at least [`MIN_PROFILE_DIMENSION`] and at most
/// [`MAX_PROFILE_SIZE`].
fn dim(name: &str, v: f64) -> Result<f64> {
    if v.is_finite() && (MIN_PROFILE_DIMENSION..=MAX_PROFILE_SIZE).contains(&v) {
        Ok(v)
    } else {
        Err(invalid(format!("profile `{name}` must be between {MIN_PROFILE_DIMENSION} and {MAX_PROFILE_SIZE} mm (got {v})")))
    }
}

/// `v` as a radius: finite, 0 (a sharp corner) up to [`MAX_PROFILE_SIZE`].
fn radius(name: &str, v: f64) -> Result<f64> {
    if v.is_finite() && (0.0..=MAX_PROFILE_SIZE).contains(&v) {
        Ok(v)
    } else {
        Err(invalid(format!("profile `{name}` must be between 0 and {MAX_PROFILE_SIZE} mm (got {v})")))
    }
}

fn less(a_name: &str, a: f64, b_name: &str, b: f64) -> Result<()> {
    if a < b { Ok(()) } else { Err(invalid(format!("profile `{a_name}` ({a}) must be less than {b_name} ({b})"))) }
}

impl Profile {
    /// The kind's id (`rect_tube`, `t_slot`…).
    pub fn kind(&self) -> &'static str {
        match self {
            Profile::Bar { .. } => "bar",
            Profile::RoundBar { .. } => "round_bar",
            Profile::RectTube { .. } => "rect_tube",
            Profile::RoundTube { .. } => "round_tube",
            Profile::Angle { .. } => "angle",
            Profile::Channel { .. } => "channel",
            Profile::IBeam { .. } => "i_beam",
            Profile::TSlot { .. } => "t_slot",
        }
    }

    /// The profile with every defaulted dimension written out.
    pub fn filled(&self) -> Profile {
        match *self {
            Profile::Bar { width, height, corner_radius } => Profile::Bar { width, height, corner_radius: Some(corner_radius.unwrap_or(0.0)) },
            Profile::RectTube { width, height, wall, outer_radius } => {
                let r = outer_radius.unwrap_or_else(|| (2.0 * wall).min(width.min(height) / 2.0));
                Profile::RectTube { width, height, wall, outer_radius: Some(r) }
            }
            Profile::Angle { width, height, thickness, root_radius, toe_radius } => Profile::Angle {
                width,
                height,
                thickness,
                root_radius: Some(root_radius.unwrap_or(thickness)),
                toe_radius: Some(toe_radius.unwrap_or(thickness / 2.0)),
            },
            Profile::Channel { height, width, web, flange, root_radius, toe_radius } => Profile::Channel {
                height,
                width,
                web,
                flange,
                root_radius: Some(root_radius.unwrap_or(web)),
                toe_radius: Some(toe_radius.unwrap_or(0.0)),
            },
            Profile::IBeam { height, width, web, flange, root_radius } => {
                Profile::IBeam { height, width, web, flange, root_radius: Some(root_radius.unwrap_or(web)) }
            }
            Profile::TSlot { width, height, opening, lip, cavity_width, cavity_depth, bore, corner_radius, pitch } => {
                let h = height.unwrap_or(width);
                let s = width.min(h);
                Profile::TSlot {
                    width,
                    height: Some(h),
                    opening: Some(opening.unwrap_or(0.22 * s)),
                    lip: Some(lip.unwrap_or(0.1 * s)),
                    cavity_width: Some(cavity_width.unwrap_or(0.4 * s)),
                    cavity_depth: Some(cavity_depth.unwrap_or(0.27 * s)),
                    bore: Some(bore.unwrap_or(0.18 * s)),
                    corner_radius: Some(corner_radius.unwrap_or(0.02 * s)),
                    pitch: Some(pitch.unwrap_or(s)),
                }
            }
            Profile::RoundBar { .. } | Profile::RoundTube { .. } => self.clone(),
        }
    }

    /// The bounding box's width and height (mm), before validation.
    pub fn size(&self) -> (f64, f64) {
        match *self {
            Profile::Bar { width, height, .. }
            | Profile::RectTube { width, height, .. }
            | Profile::Angle { width, height, .. }
            | Profile::Channel { width, height, .. }
            | Profile::IBeam { width, height, .. } => (width, height),
            Profile::RoundBar { diameter } | Profile::RoundTube { diameter, .. } => (diameter, diameter),
            Profile::TSlot { width, height, .. } => (width, height.unwrap_or(width)),
        }
    }

    /// The profile's cross-section, the bounding box centred on the origin. An error names the
    /// dimension that is out of range or does not fit.
    pub fn region(&self) -> Result<Region2> {
        let f = self.filled();
        match f {
            Profile::Bar { width, height, corner_radius } => {
                let (w, h) = (dim("width", width)?, dim("height", height)?);
                let r = radius("corner_radius", corner_radius.unwrap_or(0.0))?;
                if r > w.min(h) / 2.0 {
                    return Err(invalid(format!("profile `corner_radius` ({r}) must be at most half the smaller side ({})", w.min(h) / 2.0)));
                }
                Ok(Region2 { outer: rounded_rect(w, h, r)?, holes: Vec::new() })
            }
            Profile::RoundBar { diameter } => Ok(Region2 { outer: Loop2::circle(Vec2::ZERO, dim("diameter", diameter)? / 2.0), holes: Vec::new() }),
            Profile::RectTube { width, height, wall, outer_radius } => {
                let (w, h, t) = (dim("width", width)?, dim("height", height)?, dim("wall", wall)?);
                less("wall", 2.0 * t, "the smaller side", w.min(h))?;
                let ro = radius("outer_radius", outer_radius.unwrap_or(0.0))?;
                if ro > w.min(h) / 2.0 {
                    return Err(invalid(format!("profile `outer_radius` ({ro}) must be at most half the smaller side ({})", w.min(h) / 2.0)));
                }
                let ri = (ro - t).max(0.0);
                Ok(Region2 { outer: rounded_rect(w, h, ro)?, holes: vec![rounded_rect(w - 2.0 * t, h - 2.0 * t, ri)?.reversed()] })
            }
            Profile::RoundTube { diameter, wall } => {
                let (d, t) = (dim("diameter", diameter)?, dim("wall", wall)?);
                less("wall", 2.0 * t, "diameter", d)?;
                Ok(Region2 { outer: Loop2::circle(Vec2::ZERO, d / 2.0), holes: vec![Loop2::circle(Vec2::ZERO, d / 2.0 - t).reversed()] })
            }
            Profile::Angle { width, height, thickness, root_radius, toe_radius } => {
                let (a, b, t) = (dim("width", width)?, dim("height", height)?, dim("thickness", thickness)?);
                less("thickness", t, "the shorter leg", a.min(b))?;
                let r1 = radius("root_radius", root_radius.unwrap_or(0.0))?;
                let r2 = radius("toe_radius", toe_radius.unwrap_or(0.0))?;
                let pts = [(0.0, 0.0), (a, 0.0), (a, t), (t, t), (t, b), (0.0, b)];
                let radii = [0.0, 0.0, r2, r1, r2, 0.0];
                Ok(Region2 { outer: rounded_polygon(&shifted(&pts, a, b), &radii)?, holes: Vec::new() })
            }
            Profile::Channel { height, width, web, flange, root_radius, toe_radius } => {
                let (h, b, tw, tf) = (dim("height", height)?, dim("width", width)?, dim("web", web)?, dim("flange", flange)?);
                less("web", tw, "width", b)?;
                less("flange", 2.0 * tf, "height", h)?;
                let r1 = radius("root_radius", root_radius.unwrap_or(0.0))?;
                let r2 = radius("toe_radius", toe_radius.unwrap_or(0.0))?;
                let pts = [(0.0, 0.0), (b, 0.0), (b, tf), (tw, tf), (tw, h - tf), (b, h - tf), (b, h), (0.0, h)];
                let radii = [0.0, 0.0, r2, r1, r1, r2, 0.0, 0.0];
                Ok(Region2 { outer: rounded_polygon(&shifted(&pts, b, h), &radii)?, holes: Vec::new() })
            }
            Profile::IBeam { height, width, web, flange, root_radius } => {
                let (h, b, tw, tf) = (dim("height", height)?, dim("width", width)?, dim("web", web)?, dim("flange", flange)?);
                less("web", tw, "width", b)?;
                less("flange", 2.0 * tf, "height", h)?;
                let r = radius("root_radius", root_radius.unwrap_or(0.0))?;
                let (x, y, w, f) = (b / 2.0, h / 2.0, tw / 2.0, h / 2.0 - tf);
                let pts = [(-x, -y), (x, -y), (x, -f), (w, -f), (w, f), (x, f), (x, y), (-x, y), (-x, f), (-w, f), (-w, -f), (-x, -f)];
                let radii = [0.0, 0.0, 0.0, r, r, 0.0, 0.0, 0.0, 0.0, r, r, 0.0];
                let pts: Vec<Vec2> = pts.iter().map(|&(x, y)| Vec2::new(x, y)).collect();
                Ok(Region2 { outer: rounded_polygon(&pts, &radii)?, holes: Vec::new() })
            }
            Profile::TSlot { width, height, opening, lip, cavity_width, cavity_depth, bore, corner_radius, pitch } => {
                let t = TSlot {
                    w: dim("width", width)?,
                    h: dim("height", height.unwrap_or(width))?,
                    opening: dim("opening", opening.unwrap_or(0.0))?,
                    lip: dim("lip", lip.unwrap_or(0.0))?,
                    cavity_width: dim("cavity_width", cavity_width.unwrap_or(0.0))?,
                    cavity_depth: dim("cavity_depth", cavity_depth.unwrap_or(0.0))?,
                    bore: radius("bore", bore.unwrap_or(0.0))?,
                    corner_radius: radius("corner_radius", corner_radius.unwrap_or(0.0))?,
                    pitch: dim("pitch", pitch.unwrap_or(0.0))?,
                };
                t.region()
            }
        }
    }
}

/// Points given with the heel at the origin, moved so a `w` × `h` bounding box is centred on it.
fn shifted(pts: &[(f64, f64)], w: f64, h: f64) -> Vec<Vec2> {
    pts.iter().map(|&(x, y)| Vec2::new(x - w / 2.0, y - h / 2.0)).collect()
}

fn rounded_rect(w: f64, h: f64, r: f64) -> Result<Loop2> {
    let (x, y) = (w / 2.0, h / 2.0);
    rounded_polygon(&[Vec2::new(-x, -y), Vec2::new(x, -y), Vec2::new(x, y), Vec2::new(-x, y)], &[r; 4])
}

/// The counter-clockwise polygon `pts` with each corner rounded by its radius in `radii` (0: a
/// sharp corner). Convex and concave corners alike: the arc is tangent to both edges. An error
/// when the arcs at the two ends of an edge need more than its length.
pub fn rounded_polygon(pts: &[Vec2], radii: &[f64]) -> Result<Loop2> {
    let n = pts.len();
    if n < 3 || radii.len() != n {
        return Err(invalid("a profile polygon needs 3 or more corners, each with a radius"));
    }
    // Each corner as (where its arc starts, where it ends, the arc).
    let mut corners: Vec<(Vec2, Vec2, Option<Seg2>)> = Vec::with_capacity(n);
    for i in 0..n {
        let (Some(&prev), Some(&v), Some(&next), Some(&r)) = (pts.get((i + n - 1) % n), pts.get(i), pts.get((i + 1) % n), radii.get(i)) else {
            return Err(invalid("a profile polygon corner is missing"));
        };
        let (Some(d1), Some(d2)) = ((v - prev).normalized(), (next - v).normalized()) else {
            return Err(invalid("a profile polygon has two corners in one place"));
        };
        let turn = d1.cross(d2).atan2(d1.dot(d2));
        if r <= 0.0 || turn.abs() < 1e-12 {
            corners.push((v, v, None));
            continue;
        }
        if turn.abs() > std::f64::consts::PI - 1e-9 {
            return Err(invalid("a profile polygon turns back on itself"));
        }
        let l = r * (turn.abs() / 2.0).tan();
        let (t1, t2) = (v - d1 * l, v + d2 * l);
        let side = if turn > 0.0 { d1.perp() } else { d1.perp() * -1.0 };
        let center = t1 + side * r;
        corners.push((t1, t2, Some(Seg2::Arc { center, radius: r, start: (t1 - center).angle(), sweep: turn })));
    }
    let mut segs = Vec::with_capacity(2 * n);
    for i in 0..n {
        let (Some(&(_, out, arc)), Some(&(inn, _, _)), Some(&a), Some(&b)) =
            (corners.get(i), corners.get((i + 1) % n), pts.get(i), pts.get((i + 1) % n))
        else {
            return Err(invalid("a profile polygon corner is missing"));
        };
        if let Some(arc) = arc {
            segs.push(arc);
        }
        // The edge left between the two arcs must run the edge's way (or be empty).
        let along = (inn - out).dot(b - a);
        if along < -1e-9 * (1.0 + a.dist(b)) {
            return Err(invalid(format!("profile corner radii do not fit along an edge {:.4} mm long", a.dist(b))));
        }
        if out.dist(inn) > 1e-9 {
            segs.push(Seg2::Line { a: out, b: inn });
        }
    }
    Ok(Loop2 { segs })
}

/// A T-slot profile's dimensions, defaults filled and each checked on its own.
struct TSlot {
    w: f64,
    h: f64,
    opening: f64,
    lip: f64,
    cavity_width: f64,
    cavity_depth: f64,
    bore: f64,
    corner_radius: f64,
    pitch: f64,
}

impl TSlot {
    /// Slots (and modules) along a side `len` long.
    fn count(&self, len: f64) -> Result<usize> {
        let n = (len / self.pitch).round().max(1.0);
        if n > MAX_SLOTS_PER_SIDE as f64 {
            return Err(invalid(format!("a T-slot profile has at most {MAX_SLOTS_PER_SIDE} slots along a side (make `pitch` larger)")));
        }
        Ok(n as usize)
    }

    fn region(&self) -> Result<Region2> {
        let (w, h) = (self.w, self.h);
        let (nx, ny) = (self.count(w)?, self.count(h)?);
        let (mx, my) = (w / nx as f64, h / ny as f64);
        let (o, lip, u, d, rb) = (self.opening, self.lip, self.cavity_width, self.cavity_depth, self.bore / 2.0);
        less("opening", o, "cavity_width", u)?;
        less("lip", lip, "cavity_depth", d)?;
        // Neighbouring cavities on one side.
        let wall = MIN_T_SLOT_WALL;
        if mx.min(my) - u < wall {
            return Err(invalid(format!("profile `cavity_width` ({u}) leaves less than {wall} mm between neighbouring slots")));
        }
        // The cavities of two faces meeting at a corner: the first cavity along each face starts
        // half a module less half a cavity from the corner, and they meet unless one of them
        // starts beyond the other's depth.
        let (ax, ay) = (mx / 2.0 - u / 2.0, my / 2.0 - u / 2.0);
        if ax < d + wall && ay < d + wall {
            return Err(invalid(format!("profile `cavity_depth` ({d}) and `cavity_width` ({u}) make the slots of two faces meet at a corner")));
        }
        if mx.min(my) / 2.0 - rb < d + wall {
            return Err(invalid(format!(
                "profile `bore` ({}) and `cavity_depth` ({d}) leave less than {wall} mm between the bore and the slots",
                2.0 * rb
            )));
        }
        let rc = self.corner_radius;
        if rc > mx.min(my) / 2.0 - o / 2.0 {
            return Err(invalid(format!("profile `corner_radius` ({rc}) reaches into the slot openings")));
        }
        // Around the outline counter-clockwise from the bottom-left corner: each side's corner,
        // then its slots, each an opening through the lip and a wider cavity.
        let (x, y) = (w / 2.0, h / 2.0);
        let sides = [
            (Vec2::new(-x, -y), Vec2::new(1.0, 0.0), w, nx),
            (Vec2::new(x, -y), Vec2::new(0.0, 1.0), h, ny),
            (Vec2::new(x, y), Vec2::new(-1.0, 0.0), w, nx),
            (Vec2::new(-x, y), Vec2::new(0.0, -1.0), h, ny),
        ];
        let mut pts = Vec::new();
        let mut radii = Vec::new();
        for (corner, e, len, n) in sides {
            pts.push(corner);
            radii.push(rc);
            let inward = e.perp();
            let m = len / n as f64;
            for i in 0..n {
                let s = m * (i as f64 + 0.5);
                for (along, depth) in [(-o, 0.0), (-o, lip), (-u, lip), (-u, d), (u, d), (u, lip), (o, lip), (o, 0.0)] {
                    pts.push(corner + e * (s + along / 2.0) + inward * depth);
                    radii.push(0.0);
                }
            }
        }
        let outer = rounded_polygon(&pts, &radii)?;
        let mut holes = Vec::new();
        if rb > 0.0 {
            for i in 0..nx {
                for j in 0..ny {
                    let c = Vec2::new(-x + mx * (i as f64 + 0.5), -y + my * (j as f64 + 0.5));
                    holes.push(Loop2::circle(c, rb).reversed());
                }
            }
        }
        Ok(Region2 { outer, holes })
    }
}

/// Gauss–Legendre nodes and weights on [0, 1] (5 points).
const GL5: [(f64, f64); 5] = [
    (0.046_910_077_030_668, 0.118_463_442_528_094_5),
    (0.230_765_344_947_158_5, 0.239_314_335_249_683_2),
    (0.5, 0.284_444_444_444_444_4),
    (0.769_234_655_052_841_5, 0.239_314_335_249_683_2),
    (0.953_089_922_969_332, 0.118_463_442_528_094_5),
];

/// The boundary integrals of one loop, as it runs (counter-clockwise positive): area, ∫x dA,
/// ∫y dA, ∫x² dA, ∫y² dA, ∫xy dA, by Green's theorem.
fn loop_moments(l: &Loop2) -> [f64; 6] {
    let mut m = [0.0; 6];
    for seg in &l.segs {
        let pieces = match seg {
            Seg2::Line { .. } => 1,
            _ => 16,
        };
        for k in 0..pieces {
            let h = 1.0 / pieces as f64;
            for (x, wt) in GL5 {
                let t = (k as f64 + x) * h;
                let (p, dp) = (seg.point_at(t), seg.derivative(t));
                let w = wt * h;
                let terms = [
                    p.x * dp.y,
                    0.5 * p.x * p.x * dp.y,
                    -0.5 * p.y * p.y * dp.x,
                    p.x * p.x * p.x * dp.y / 3.0,
                    -p.y * p.y * p.y * dp.x / 3.0,
                    0.5 * p.x * p.x * p.y * dp.y,
                ];
                for (acc, v) in m.iter_mut().zip(terms) {
                    *acc += w * v;
                }
            }
        }
    }
    m
}

/// Points of `l` that include its extremes along x and y: every segment's ends, and where an arc
/// passes a quarter turn. Free-form segments add the polyline points within 1 µm.
fn extreme_points(l: &Loop2) -> Vec<Vec2> {
    let mut out = Vec::new();
    for seg in &l.segs {
        match *seg {
            Seg2::Line { a, b } => out.extend([a, b]),
            Seg2::Arc { center, radius, start, sweep } => {
                out.extend([seg.start(), seg.end()]);
                let (lo, hi) = if sweep >= 0.0 { (start, start + sweep) } else { (start + sweep, start) };
                let quarter = std::f64::consts::FRAC_PI_2;
                let mut k = (lo / quarter).ceil();
                // An arc turns at most once around: five quarter turns at most.
                for _ in 0..5 {
                    let t = k * quarter;
                    if t > hi {
                        break;
                    }
                    out.push(center + Vec2::from_angle(t) * radius);
                    k += 1.0;
                }
            }
            _ => out.extend(seg.polyline(1e-3)),
        }
    }
    out
}

/// The section properties of `region` (outer loop and holes, whichever way each runs).
pub fn section(region: &Region2) -> Section {
    let mut m = loop_moments(&region.outer.ccw());
    for hole in &region.holes {
        for (acc, v) in m.iter_mut().zip(loop_moments(&hole.ccw())) {
            *acc -= v;
        }
    }
    let [a, sx, sy, jyy, jxx, jxy] = m;
    let (cx, cy) = if a.abs() > 1e-300 { (sx / a, sy / a) } else { (0.0, 0.0) };
    let (mut lo, mut hi) = (Vec2::new(f64::INFINITY, f64::INFINITY), Vec2::new(f64::NEG_INFINITY, f64::NEG_INFINITY));
    for p in extreme_points(&region.outer) {
        lo = Vec2::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Vec2::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    let (width, height) = if lo.x <= hi.x { (hi.x - lo.x, hi.y - lo.y) } else { (0.0, 0.0) };
    Section { area: a, centroid: [cx, cy], ixx: jxx - a * cy * cy, iyy: jyy - a * cx * cx, ixy: jxy - a * cx * cy, width, height }
}

impl Anchor {
    /// Where the anchor is on a profile whose bounding box is `w` × `h`, centred on the origin,
    /// with its centroid at `centroid`.
    pub fn point(self, w: f64, h: f64, centroid: Vec2) -> Vec2 {
        let (x, y) = (w / 2.0, h / 2.0);
        match self {
            Anchor::Center => Vec2::ZERO,
            Anchor::Centroid => centroid,
            Anchor::TopLeft => Vec2::new(-x, y),
            Anchor::Top => Vec2::new(0.0, y),
            Anchor::TopRight => Vec2::new(x, y),
            Anchor::Left => Vec2::new(-x, 0.0),
            Anchor::Right => Vec2::new(x, 0.0),
            Anchor::BottomLeft => Vec2::new(-x, -y),
            Anchor::Bottom => Vec2::new(0.0, -y),
            Anchor::BottomRight => Vec2::new(x, -y),
        }
    }
}

/// `region` turned by `angle` (radians, counter-clockwise) about its `anchor` point and moved so
/// that point lands on `at`.
pub fn placed(region: &Region2, anchor: Vec2, at: Vec2, angle: f64) -> Region2 {
    let (c, s) = (angle.cos(), angle.sin());
    let map = |p: Vec2| {
        let q = p - anchor;
        at + Vec2::new(c * q.x - s * q.y, s * q.x + c * q.y)
    };
    let seg = |g: &Seg2| match *g {
        Seg2::Line { a, b } => Seg2::Line { a: map(a), b: map(b) },
        Seg2::Arc { center, radius, start, sweep } => Seg2::Arc { center: map(center), radius, start: start + angle, sweep },
        Seg2::Cubic { p0, p1, p2, p3 } => Seg2::Cubic { p0: map(p0), p1: map(p1), p2: map(p2), p3: map(p3) },
        Seg2::Conic { a, apex, b, w } => Seg2::Conic { a: map(a), apex: map(apex), b: map(b), w },
        Seg2::Bezier { n, p } => Seg2::Bezier { n, p: p.map(map) },
    };
    let lp = |l: &Loop2| Loop2 { segs: l.segs.iter().map(seg).collect() };
    Region2 { outer: lp(&region.outer), holes: region.holes.iter().map(lp).collect() }
}
