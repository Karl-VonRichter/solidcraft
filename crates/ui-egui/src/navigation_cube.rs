//! Original, code-drawn bevelled navigation cube. Geometry owns both paint and picking.
use super::*;

pub(super) const SIZE: f32 = 164.0;
const HALF: f64 = 40.0;
const BEVEL: f64 = 0.74;

struct Facet {
    direction: Vec3,
    points: Vec<Pos2>,
    centre: Pos2,
    label: String,
    text_axes: Option<(egui::Vec2, egui::Vec2)>,
    depth: f64,
}

fn project(cam: &Camera, v: Vec3, scale: f64) -> egui::Vec2 {
    let (r, u, _) = cam.basis();
    let v = cam.from_frame(v);
    vec2((v.dot(r) * scale) as f32, -(v.dot(u) * scale) as f32)
}

fn direction_label(d: Vec3) -> String {
    [(d.z, "BOTTOM", "TOP"), (d.y, "FRONT", "BACK"), (d.x, "LEFT", "RIGHT")]
        .into_iter()
        .filter_map(|(v, neg, pos)| {
            if v < 0.0 {
                Some(neg)
            } else if v > 0.0 {
                Some(pos)
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

fn facets(cam: &Camera, c: Pos2, half: f64) -> Vec<Facet> {
    if !cam.is_valid() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut add = |d: Vec3, vertices: Vec<Vec3>, axes: Option<(Vec3, Vec3)>| {
        let depth = cam.from_frame(d).dot(cam.back());
        if depth <= 1e-5 {
            return;
        }
        let centre = vertices.iter().copied().fold(Vec3::ZERO, |sum, p| sum + p) / vertices.len() as f64;
        let mut points: Vec<_> = vertices.into_iter().map(|v| c + project(cam, v, half)).collect();
        let at = c + project(cam, centre, half);
        points.sort_by(|a, b| (a.y - at.y).atan2(a.x - at.x).total_cmp(&(b.y - at.y).atan2(b.x - at.x)));
        out.push(Facet {
            direction: d,
            points,
            centre: at,
            label: direction_label(d),
            text_axes: axes.map(|(r, u)| (project(cam, r, 1.0), -project(cam, u, 1.0))),
            depth,
        });
    };
    // Six square faces; twelve bevel strips; eight triangular corner patches.
    for (n, r, u) in [
        (Vec3::X, Vec3::Y, Vec3::Z),
        (-Vec3::X, -Vec3::Y, Vec3::Z),
        (Vec3::Y, -Vec3::X, Vec3::Z),
        (-Vec3::Y, Vec3::X, Vec3::Z),
        (Vec3::Z, Vec3::X, Vec3::Y),
        (-Vec3::Z, Vec3::X, -Vec3::Y),
    ] {
        add(
            n,
            [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].into_iter().map(|(x, y)| n + r * (x * BEVEL) + u * (y * BEVEL)).collect(),
            Some((r, u)),
        );
    }
    for (a, b, along) in [(Vec3::X, Vec3::Y, Vec3::Z), (Vec3::X, Vec3::Z, Vec3::Y), (Vec3::Y, Vec3::Z, Vec3::X)] {
        for sa in [-1.0, 1.0] {
            for sb in [-1.0, 1.0] {
                let (a, b) = (a * sa, b * sb);
                add(
                    a + b,
                    vec![a + b * BEVEL - along * BEVEL, a * BEVEL + b - along * BEVEL, a * BEVEL + b + along * BEVEL, a + b * BEVEL + along * BEVEL],
                    None,
                );
            }
        }
    }
    for x in [-1.0, 1.0] {
        for y in [-1.0, 1.0] {
            for z in [-1.0, 1.0] {
                add(
                    Vec3::new(x, y, z),
                    vec![Vec3::new(x, y * BEVEL, z * BEVEL), Vec3::new(x * BEVEL, y, z * BEVEL), Vec3::new(x * BEVEL, y * BEVEL, z)],
                    None,
                );
            }
        }
    }
    out.sort_by(|a, b| a.depth.total_cmp(&b.depth));
    out
}

fn contains(points: &[Pos2], p: Pos2) -> bool {
    if !p.is_finite() || points.len() < 3 {
        return false;
    }
    let mut sign = 0.0_f32;
    for (a, b) in points.iter().zip(points.iter().cycle().skip(1)).take(points.len()) {
        let edge = *b - *a;
        let offset = p - *a;
        let cross = edge.x * offset.y - edge.y * offset.x;
        if cross.abs() > 1e-3 {
            if sign != 0.0 && sign.signum() != cross.signum() {
                return false;
            }
            sign = cross;
        }
    }
    sign != 0.0
}

fn hit(faces: &[Facet], p: Pos2) -> Option<usize> {
    faces.iter().enumerate().rev().find(|(_, f)| contains(&f.points, p)).map(|(i, _)| i)
}

pub(super) fn drag_origin(cam: &Camera, centre: Pos2, pointer: Pos2, scale: f32) -> bool {
    hit(&facets(cam, centre, HALF * f64::from(scale)), pointer).is_some()
}

// Private marker for label meshes with atlas-pixel UVs. The output hook replaces
// it with the font texture before any renderer sees the frame.
const LABEL_PIXELS: egui::TextureId = egui::TextureId::User(u64::from_le_bytes(*b"cubetext"));

struct LabelAtlas;

impl egui::plugin::Plugin for LabelAtlas {
    fn debug_name(&self) -> &'static str {
        "navigation cube label atlas"
    }

    fn output_hook(&mut self, ctx: &egui::Context, output: &mut egui::FullOutput) {
        let size = ctx.fonts(|f| f.font_image_size());
        let normalize = vec2(1.0 / size[0].max(1) as f32, 1.0 / size[1].max(1) as f32);
        for shape in &mut output.shapes {
            if let Shape::Mesh(mesh) = &mut shape.shape
                && mesh.texture_id == LABEL_PIXELS
            {
                let mesh = std::sync::Arc::make_mut(mesh);
                mesh.texture_id = egui::TextureId::default();
                for v in &mut mesh.vertices {
                    v.uv = (v.uv.to_vec2() * normalize).to_pos2();
                }
            }
        }
    }
}

/// Warp the font mesh into the projected face, so labels turn with the model.
fn face_text(painter: &egui::Painter, face: &Facet, ink: Color32, font_size: f32) {
    let Some((r, down)) = face.text_axes else { return };
    let opacity = ((face.depth - 0.18) / 0.17).clamp(0.0, 1.0) as f32;
    if opacity <= 0.0 {
        return;
    }
    let ink = ink.gamma_multiply(opacity);
    let galley = painter.layout_no_wrap(face.label.clone(), FontId::proportional(font_size), ink);
    let centre = galley.rect.center();
    // Keep atlas-pixel UVs until the frame is complete: another label or widget
    // can grow the atlas after this mesh is created, invalidating early UVs.
    painter.ctx().add_plugin(LabelAtlas);
    let text = Shape::Text(egui::epaint::TextShape::new(Pos2::ZERO, galley, ink));
    let atlas = painter.ctx().fonts(|f| f.font_image_size());
    let primitives =
        painter.ctx().tessellate(vec![egui::epaint::ClippedShape { clip_rect: Rect::EVERYTHING, shape: text }], painter.ctx().pixels_per_point());
    for primitive in primitives {
        if let egui::epaint::Primitive::Mesh(mut mesh) = primitive.primitive {
            mesh.texture_id = LABEL_PIXELS;
            for v in &mut mesh.vertices {
                v.uv = pos2(v.uv.x * atlas[0] as f32, v.uv.y * atlas[1] as f32);
                let local = v.pos - centre;
                v.pos = face.centre + r * local.x + down * local.y;
            }
            painter.add(Shape::mesh(mesh));
        }
    }
}

fn paint_cube(painter: &egui::Painter, faces: &[Facet], hovered: Option<usize>, t: Tokens, labels: bool, scale: f32) {
    // A shared, opaque triangle mesh has no antialiasing fringes between facets.
    // Per-polygon fringes and miter joins overlap as a bevel becomes edge-on.
    let mut fill_mesh = egui::epaint::Mesh::default();
    let mut edges: Vec<(Pos2, Pos2, bool)> = Vec::new();
    for (i, face) in faces.iter().enumerate() {
        let base = if face.text_axes.is_some() { t.cube_face } else { t.cube_bevel };
        let shade = (0.72 + 0.28 * face.depth.min(1.0)) as f32;
        let fill = if hovered == Some(i) {
            t.accent_soft.to_opaque()
        } else {
            Color32::from_rgb((f32::from(base.r()) * shade) as u8, (f32::from(base.g()) * shade) as u8, (f32::from(base.b()) * shade) as u8)
        };
        let centre = fill_mesh.vertices.len() as u32;
        fill_mesh.colored_vertex(face.centre, fill);
        for &p in &face.points {
            fill_mesh.colored_vertex(p, fill);
        }
        for (j, (&a, &b)) in face.points.iter().zip(face.points.iter().cycle().skip(1)).take(face.points.len()).enumerate() {
            fill_mesh.add_triangle(centre, centre + 1 + j as u32, centre + 1 + ((j + 1) % face.points.len()) as u32);
            let highlighted = hovered == Some(i);
            if let Some(edge) = edges
                .iter_mut()
                .find(|(p, q, _)| (p.distance_sq(a) < 1e-6 && q.distance_sq(b) < 1e-6) || (p.distance_sq(b) < 1e-6 && q.distance_sq(a) < 1e-6))
            {
                edge.2 |= highlighted;
            } else {
                edges.push((a, b, highlighted));
            }
        }
    }
    painter.add(Shape::mesh(fill_mesh));
    for (a, b, highlighted) in edges {
        painter.line_segment([a, b], Stroke::new(1.0, if highlighted { t.accent } else { t.cube_border }));
    }
    if labels {
        for face in faces {
            face_text(painter, face, t.cube_text, 14.0 * scale);
        }
    }
}

fn navigate(app: &mut SolveApp, action: Option<&str>, dir: Option<Vec3>) {
    let mut params = json!({"camera":app.cam});
    if let Some(action) = action {
        params["action"] = json!(action);
    }
    if let Some(d) = dir {
        params["direction"] = json!([d.x, d.y, d.z]);
    }
    let _ = app.run("view.navigate", params);
}

fn control(app: &mut SolveApp, ui: &mut egui::Ui, bounds: Rect, action: &str, tooltip: &str) -> bool {
    crate::scenario::publish_handle(&format!("navigation_cube:control:{action}"), bounds.center());
    let response = ui.interact(bounds, ui.id().with(("navigation_cube", action)), Sense::click()).on_hover_text(tooltip);
    let hover = response.hovered();
    if response.clicked() {
        navigate(app, Some(action), None);
    }
    hover
}

fn arrow(painter: &egui::Painter, at: Pos2, direction: egui::Vec2, hover: bool, t: Tokens) {
    let side = vec2(-direction.y, direction.x);
    painter.add(Shape::convex_polygon(
        vec![at + direction * 7.0, at - direction * 5.0 + side * 7.0, at - direction * 5.0 - side * 7.0],
        if hover { t.accent_soft } else { t.cube_face },
        Stroke::new(1.0, if hover { t.accent } else { t.cube_border }),
    ));
}

fn curved_arrow(painter: &egui::Painter, at: Pos2, clockwise: bool, hover: bool, t: Tokens, scale: f32) {
    let sign = if clockwise { 1.0 } else { -1.0 };
    let points: Vec<_> = (0..=16)
        .map(|i| {
            let angle = -2.6 + f32::from(i as u8) / 16.0 * 2.0;
            at + vec2(sign * angle.cos() * 15.0, angle.sin() * 15.0) * scale
        })
        .collect();
    painter.add(Shape::line(points.clone(), Stroke::new(5.0 * scale, if hover { t.accent } else { t.cube_border })));
    painter.add(Shape::line(points.clone(), Stroke::new(3.0 * scale, if hover { t.accent_soft } else { t.cube_face })));
    if let Some(tip) = points.last() {
        arrow(painter, *tip, vec2(sign * 0.56, 0.83) * scale, hover, t);
    }
}

pub(super) fn show(app: &mut SolveApp, ui: &mut egui::Ui, rect: Rect) {
    let outer = axis_rect(app, rect);
    if !outer.is_positive() {
        app.viewport.axis_drag = None;
        crate::scenario::publish_count("axis_view_highlights", 0.0);
        return;
    }
    crate::scenario::publish_rect("axis_view:outer", outer);
    let t = Tokens::get();
    let scale = outer.width() / SIZE;
    let c = axis_center(app, rect);
    let orbiting = axis_view_orbit(app, ui, rect);
    let faces = facets(&app.cam, c, HALF * f64::from(scale));
    let hover = ui.input(|i| i.pointer.hover_pos()).filter(|_| ui.rect_contains_pointer(rect));
    let hovered = if orbiting { None } else { hover.and_then(|p| hit(&faces, p)) };
    let painter = ui.painter_at(outer);
    paint_cube(&painter, &faces, hovered, t, true, scale);
    for face in &faces {
        crate::scenario::publish_handle(&format!("navigation_cube:{}", face.label.to_ascii_lowercase().replace(" · ", "_")), face.centre);
        // Keep principal-axis handles for existing automation, using only visible faces.
        let world = app.cam.from_frame(face.direction);
        let axis = match (world.x, world.y, world.z) {
            (1.0, 0.0, 0.0) => Some("x"),
            (-1.0, 0.0, 0.0) => Some("-x"),
            (0.0, 1.0, 0.0) => Some("y"),
            (0.0, -1.0, 0.0) => Some("-y"),
            (0.0, 0.0, 1.0) => Some("z"),
            (0.0, 0.0, -1.0) => Some("-z"),
            _ => None,
        };
        if let Some(axis) = axis {
            crate::scenario::publish_handle(&format!("axis_view:{axis}"), face.centre);
        }
    }
    crate::scenario::publish_count("axis_view_highlights", f64::from(u8::from(hovered.is_some())));
    if let Some(face) = hovered.and_then(|i| faces.get(i)) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.interact(Rect::from_center_size(hover.unwrap_or(face.centre), vec2(2.0, 2.0)), ui.id().with("navigation_cube_tip"), Sense::hover())
            .on_hover_text(format!("{} view. Drag to orbit; hold Shift to snap.", face.label));
        if ui.input(|i| i.pointer.primary_clicked()) {
            navigate(app, None, Some(face.direction));
        }
    }
    for (action, offset, direction, tooltip) in [
        ("left", vec2(-72.0, 0.0), vec2(-1.0, 0.0), "Orbit left 90°"),
        ("right", vec2(72.0, 0.0), vec2(1.0, 0.0), "Orbit right 90°"),
        ("up", vec2(0.0, -72.0), vec2(0.0, -1.0), "Orbit up 90°"),
        ("down", vec2(0.0, 72.0), vec2(0.0, 1.0), "Orbit down 90°"),
    ] {
        let at = c + offset * scale;
        let hovered = control(app, ui, Rect::from_center_size(at, vec2(20.0, 20.0) * scale), action, tooltip);
        arrow(&painter, at, direction * scale, hovered, t);
    }
    for (action, dx, clockwise, tooltip) in
        [("counterclockwise", -37.0, false, "Rotate view counterclockwise 90°"), ("clockwise", 37.0, true, "Rotate view clockwise 90°")]
    {
        let at = c + vec2(dx, -57.0) * scale;
        let hovered = control(app, ui, Rect::from_center_size(at - vec2(0.0, 8.0) * scale, vec2(36.0, 24.0) * scale), action, tooltip);
        curved_arrow(&painter, at, clockwise, hovered, t, scale);
    }
    let home = Rect::from_center_size(c + vec2(65.0, 65.0) * scale, vec2(26.0, 26.0) * scale);
    crate::scenario::publish_rect("axis_view:home", home);
    crate::scenario::publish_handle("axis_view:home", home.center());
    let home_hover = control(app, ui, home, "home", "Home: isometric view and fit model");
    paint_cube(
        &painter,
        &facets(&Camera::default(), home.center(), 9.0 * f64::from(scale)),
        if home_hover { Some(0) } else { None },
        t,
        false,
        scale,
    );
    let fit = Rect::from_center_size(c + vec2(67.0, -67.0) * scale, vec2(22.0, 22.0) * scale);
    let fit_hover = control(app, ui, fit, "fit", "Fit model in view");
    painter.circle(fit.center(), 6.0 * scale, if fit_hover { t.accent_soft } else { t.cube_face }, Stroke::new(1.0, t.cube_border));
    // Small world-axis triad below the cube; it follows the selected up-axis frame.
    let origin = c + vec2(-60.0, 52.0) * scale;
    for (d, color, label) in [(Vec3::X, t.axis_x, "X"), (Vec3::Y, t.axis_y, "Y"), (Vec3::Z, t.axis_z, "Z")] {
        let (r, u, _) = app.cam.basis();
        let tip = origin + vec2((d.dot(r) * 17.0) as f32, -(d.dot(u) * 17.0) as f32) * scale;
        painter.line_segment([origin, tip], Stroke::new(1.5, color));
        painter.text(tip, Align2::CENTER_CENTER, label, FontId::proportional(8.0), color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solvecraft_engine::render::{StandardView, UpAxis};

    #[test]
    fn projected_facets_pick_their_own_centres_in_all_views_and_up_frames() {
        for up in [UpAxis::Z, UpAxis::Y] {
            for view in [
                StandardView::Front,
                StandardView::Back,
                StandardView::Top,
                StandardView::Bottom,
                StandardView::Left,
                StandardView::Right,
                StandardView::Iso,
            ] {
                let mut cam = Camera { up, ..Default::default() };
                cam.set_view(view);
                for roll in [0.0, 0.7, std::f64::consts::FRAC_PI_2] {
                    cam.roll = roll;
                    let faces = facets(&cam, Pos2::ZERO, HALF);
                    assert!(!faces.is_empty());
                    for (i, face) in faces.iter().enumerate() {
                        assert_eq!(hit(&faces, face.centre), Some(i), "{up:?} {view:?} {roll} {}", face.label);
                    }
                    assert!(hit(&faces, pos2(80.0, 80.0)).is_none());
                }
            }
        }
    }

    #[test]
    fn face_labels_use_normalized_font_atlas_coordinates() {
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let faces = facets(&Camera::default(), pos2(100.0, 100.0), HALF);
            for face in &faces {
                face_text(ui.painter(), face, Tokens::light().cube_text, 14.0);
            }
        });
        let meshes: Vec<_> = output.shapes.iter().filter_map(|shape| if let Shape::Mesh(mesh) = &shape.shape { Some(mesh) } else { None }).collect();
        assert_eq!(meshes.len(), 3, "the isometric view must show three face labels");
        for mesh in meshes {
            assert!(!mesh.vertices.is_empty());
            assert!(mesh.vertices.iter().all(|v| (0.0..=1.0).contains(&v.uv.x) && (0.0..=1.0).contains(&v.uv.y)));
        }
    }

    #[test]
    fn label_uvs_follow_font_atlas_growth_later_in_the_same_frame() {
        let ctx = egui::Context::default();
        let mut pixel_uv = Pos2::ZERO;
        let mut glyph_vertex = 0;
        let mut early_size = [0; 2];
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let mut cam = Camera::default();
            cam.set_view(StandardView::Top);
            let faces = facets(&cam, pos2(100.0, 100.0), HALF);
            let face = faces.iter().find(|f| f.label == "TOP").unwrap();
            let galley = ui.painter().layout_no_wrap(face.label.clone(), FontId::proportional(14.0), Tokens::light().cube_text);
            let (index, vertex) = galley.rows.first().unwrap().visuals.mesh.vertices.iter().enumerate().find(|(_, v)| v.uv.y > 1.0).unwrap();
            glyph_vertex = index;
            pixel_uv = vertex.uv;
            face_text(ui.painter(), face, Tokens::light().cube_text, 14.0);
            early_size = ctx.fonts(|f| f.font_image_size());
            // Other widgets can allocate new glyphs after the cube has been drawn.
            let text: String = (32_u8..127).map(char::from).collect();
            let _ = ui.painter().layout_no_wrap(text, FontId::proportional(48.0), Color32::WHITE);
        });
        let size = ctx.fonts(|f| f.font_image_size());
        assert!(size[1] > early_size[1], "the reproduction must grow the font atlas");
        let vertex = output.shapes.iter().find_map(|s| if let Shape::Mesh(m) = &s.shape { m.vertices.get(glyph_vertex) } else { None }).unwrap();
        assert!(output.shapes.iter().all(|s| !matches!(&s.shape, Shape::Mesh(m) if m.texture_id == LABEL_PIXELS)));
        assert!((vertex.uv.x * size[0] as f32 - pixel_uv.x).abs() < 1e-3);
        assert!((vertex.uv.y * size[1] as f32 - pixel_uv.y).abs() < 1e-3);
    }

    #[test]
    fn rotating_cube_facets_stay_opaque_and_inside_the_projected_geometry() {
        let ctx = egui::Context::default();
        for up in [UpAxis::Z, UpAxis::Y] {
            for view in [StandardView::Top, StandardView::Bottom, StandardView::Front, StandardView::Back, StandardView::Left, StandardView::Right] {
                let from = Camera { up, ..Default::default() };
                let mut to = from;
                to.set_view(view);
                for step in 0..=60 {
                    let cam = Camera::interpolate(&from, &to, f64::from(step) / 60.0);
                    let faces = facets(&cam, pos2(100.0, 100.0), HALF);
                    let bounds = Rect::from_points(&faces.iter().flat_map(|f| f.points.iter().copied()).collect::<Vec<_>>());
                    let output = ctx.run_ui(egui::RawInput::default(), |ui| {
                        paint_cube(ui.painter(), &faces, Some(0), Tokens::light(), false, 1.0);
                    });
                    let fills: Vec<_> = output.shapes.iter().filter_map(|s| if let Shape::Mesh(m) = &s.shape { Some(m) } else { None }).collect();
                    assert_eq!(fills.len(), 1);
                    let mesh = fills[0];
                    assert!(mesh.is_valid());
                    assert!(mesh.vertices.iter().all(|v| v.color.a() == 255 && v.pos.is_finite() && bounds.contains(v.pos)));
                }
            }
        }
    }

    #[test]
    fn cube_faces_edges_corners_roll_and_home_drive_the_shared_command() {
        let mut h = crate::scenario::Harness::new();
        h.app.preferences.look_at_anim = false;
        let before = h.app.session.doc.to_json();
        for (name, direction) in [
            ("top", Vec3::Z),
            ("front", -Vec3::Y),
            ("right", Vec3::X),
            ("top_front", Vec3::Z - Vec3::Y),
            ("top_front_right", Vec3::Z - Vec3::Y + Vec3::X),
        ] {
            h.call("ui.view", json!({"view":"iso"}));
            let p = crate::scenario::handle_at(&format!("navigation_cube:{name}")).unwrap();
            h.call("ui.click", json!({"x":p.x,"y":p.y}));
            assert!(h.app.cam.back().dist(direction.normalized().unwrap()) < 2e-6, "{name}");
            assert!(h.app.session.selection.is_empty());
        }
        let camera = h.app.cam;
        let p = crate::scenario::handle_at("navigation_cube:control:clockwise").unwrap();
        h.call("ui.click", json!({"x":p.x,"y":p.y}));
        assert!((h.app.cam.roll - camera.roll - std::f64::consts::FRAC_PI_2).abs() < 1e-6);
        assert!(h.app.cam.back().dist(camera.back()) < 1e-6);
        let p = crate::scenario::handle_at("navigation_cube:control:home").unwrap();
        h.call("ui.click", json!({"x":p.x,"y":p.y}));
        assert!(h.app.cam.back().dist(Camera::default().back()) < 1e-6);
        assert_eq!(h.app.session.doc.to_json(), before);
        assert!(h.app.session.undo.is_empty());
    }
}
