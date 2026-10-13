//! Sketch operations use the same selection rows, footer and scratch previews as solids.
use crate::dialogs::{Dialog, field, row_label};
use crate::selection::{AXES, CURVES, SKETCH_POINTS, SelInput};
use crate::viewport::Hit;
use egui::Ui;
use serde_json::{Value, json};
use solidcraft_engine::doc::expr::Kind;
use solidcraft_engine::geom::{Vec2, Vec3};
use solidcraft_engine::{Sel, Session};

#[derive(Clone, Debug)]
pub enum SketchModify {
    Mirror,
    Rect { count: String, spacing: String, count2: String, spacing2: String, axis: usize },
    Circular { count: String, angle: String },
    Offset { distance: String, chain: bool, flip: bool },
}

pub fn start(id: &str) -> Option<(crate::dialogs::Kind, Vec<SelInput>)> {
    let (kind, reference) = match id {
        "sketch.mirror" => (SketchModify::Mirror, Some(SelInput::new("Mirror Line", AXES, false))),
        "sketch.pattern.rectangular" => (
            SketchModify::Rect { count: "3".into(), spacing: "20 mm".into(), count2: "1".into(), spacing2: "20 mm".into(), axis: 0 },
            Some(SelInput::new("Direction", AXES, false)),
        ),
        "sketch.pattern.circular" => {
            (SketchModify::Circular { count: "6".into(), angle: "360 deg".into() }, Some(SelInput::new("Centre", SKETCH_POINTS, false)))
        }
        "sketch.offset" => (SketchModify::Offset { distance: "5 mm".into(), chain: true, flip: false }, None),
        _ => return None,
    };
    let accept = if matches!(kind, SketchModify::Offset { .. }) { CURVES } else { CURVES | SKETCH_POINTS };
    let mut inputs = vec![SelInput::new("Objects", accept, true)];
    inputs.extend(reference);
    Some((crate::dialogs::Kind::SketchModify(kind), inputs))
}

impl SketchModify {
    pub fn title(&self) -> &'static str {
        match self {
            Self::Mirror => "SKETCH MIRROR",
            Self::Rect { .. } => "SKETCH RECTANGULAR PATTERN",
            Self::Circular { .. } => "SKETCH CIRCULAR PATTERN",
            Self::Offset { .. } => "SKETCH OFFSET",
        }
    }
    pub fn primary(&mut self) -> Option<(&'static str, Kind, &mut String)> {
        match self {
            Self::Rect { spacing, .. } => Some(("Spacing", Kind::Length, spacing)),
            Self::Circular { angle, .. } => Some(("Total angle", Kind::Angle, angle)),
            Self::Offset { distance, .. } => Some(("Distance", Kind::Length, distance)),
            _ => None,
        }
    }
    pub fn rows(&mut self, ui: &mut Ui) -> bool {
        let mut enter = false;
        match self {
            Self::Mirror => {}
            Self::Rect { count, spacing, count2, spacing2, axis } => {
                row_label(ui, "Default direction");
                crate::dialogs::combo(ui, "sketch_pattern_axis", &["Sketch X", "Sketch Y"], axis);
                ui.end_row();
                enter |= crate::dialogs::rectangular_pattern_rows(ui, count, spacing, count2, spacing2);
            }
            Self::Circular { count, angle } => {
                enter |= crate::dialogs::circular_pattern_rows(ui, count, angle);
            }
            Self::Offset { distance, chain, flip } => {
                row_label(ui, "Distance");
                enter |= field(ui, distance);
                ui.end_row();
                row_label(ui, "Chain selection");
                ui.checkbox(chain, "Connected chain");
                ui.end_row();
                row_label(ui, "Side");
                ui.checkbox(flip, "Flip");
                ui.end_row();
            }
        }
        enter
    }
}

pub fn candidate(d: &Dialog, s: &Session, hit: &Hit) -> Option<Sel> {
    let inp = d.active_input()?;
    let active = s.active_sketch?;
    match hit {
        Hit::SketchCurve { sketch, id, straight, .. } if *sketch == active && (inp.accept & CURVES != 0 || (*straight && inp.accept & AXES != 0)) => {
            Some(Sel::SketchCurve { id: id.clone() })
        }
        Hit::SketchPoint { sketch, id, .. } if *sketch == active && inp.accept & SKETCH_POINTS != 0 => Some(Sel::SketchPoint { id: id.clone() }),
        _ => None,
    }
}

/// A centre may be placed anywhere on the active sketch, without adding a document point.
pub fn pick_position(d: &mut Dialog, s: &Session, point: Vec3) -> bool {
    if !matches!(d.kind, crate::dialogs::Kind::SketchModify(SketchModify::Circular { .. })) || d.active != 1 {
        return false;
    }
    d.pick(s, Sel::Vertex { body: String::new(), point });
    true
}

pub fn commands(s: &Session, d: &Dialog, kind: &SketchModify) -> Result<Vec<(String, Value)>, String> {
    let state = s.world_state();
    let sketch = s.active_sketch.and_then(|id| state.sketch(id)).ok_or("Edit a sketch first")?;
    let ids = d
        .inputs
        .first()
        .map(|i| {
            i.items
                .iter()
                .filter_map(|x| match x {
                    Sel::SketchCurve { id } | Sel::SketchPoint { id } => Some(id.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if ids.is_empty() {
        return Err("Select the objects first".into());
    }
    let reference = d.inputs.get(1).and_then(|i| i.items.first());
    let line = || -> Result<Vec2, String> {
        let Some(Sel::SketchCurve { id }) = reference else { return Err("Select a sketch line".into()) };
        let index = sketch.sketch.curve_index(id).ok_or("The selected line no longer exists")?;
        let Some(solidcraft_engine::sketch::Shape::Line { a, b }) = sketch.sketch.shape(index) else {
            return Err("Select a straight sketch line".into());
        };
        (b - a).normalized().ok_or_else(|| "The selected line has no length".into())
    };
    let values = s.doc.param_values().0;
    let defaults = solidcraft_engine::doc::expr::Defaults::for_unit(&s.doc.units, 1.0);
    let eval = |value: &str, kind| {
        solidcraft_engine::doc::expr::eval_with(value, &|name| {
            values.get(name).copied().ok_or_else(|| solidcraft_engine::doc::DocError::Unknown(format!("parameter `{name}`")))
        })
        .and_then(|value| value.to_kind_in(kind, defaults))
        .map_err(|e| e.to_string())
    };
    let count = |value: &str, minimum| -> Result<usize, String> {
        let n = eval(value, Kind::Unitless)?;
        if !n.is_finite() || n.fract() != 0.0 || n < minimum as f64 || n > 500.0 {
            return Err(format!("Quantity must be an integer from {minimum} to 500"));
        }
        Ok(n as usize)
    };
    let (command, params) = match kind {
        SketchModify::Mirror => {
            line()?;
            let Some(Sel::SketchCurve { id }) = reference else { return Err("Select the mirror line".into()) };
            ("sketch.mirror", json!({"entities":ids,"line":id}))
        }
        SketchModify::Rect { count: n, spacing, count2, spacing2, axis } => {
            let dir = if reference.is_some() {
                line()?
            } else if *axis == 1 {
                Vec2::Y
            } else {
                Vec2::X
            };
            (
                "sketch.pattern.rectangular",
                json!({"entities":ids,"dir":[dir.x,dir.y],"count":count(n, 1)?,"spacing":eval(spacing, Kind::Length)?,"count2":count(count2, 1)?,"spacing2":eval(spacing2, Kind::Length)?}),
            )
        }
        SketchModify::Circular { count: n, angle } => {
            let centre = match reference {
                Some(Sel::SketchPoint { id }) => json!(id),
                Some(Sel::Vertex { point, body }) if body.is_empty() => {
                    let p = sketch.plane.to_local(*point);
                    json!([p.x, p.y])
                }
                _ => return Err("Select a point or click in the sketch to place the centre".into()),
            };
            ("sketch.pattern.circular", json!({"entities":ids,"center":centre,"count":count(n, 2)?,"angle":eval(angle, Kind::Angle)?.to_degrees()}))
        }
        SketchModify::Offset { distance, chain, flip } => {
            let value = eval(distance, Kind::Length)?;
            if value <= 1e-9 {
                return Err("Enter a positive offset distance; use Flip to change the side".into());
            }
            ("sketch.offset", json!({"curves":ids,"distance":distance,"chain":chain,"flip":flip}))
        }
    };
    Ok(vec![(command.into(), params)])
}

#[cfg(test)]
#[path = "dialogs_sketch_tests.rs"]
mod tests;
