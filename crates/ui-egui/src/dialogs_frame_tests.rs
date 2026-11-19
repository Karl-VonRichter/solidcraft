use crate::test_support::run_all;
use serde_json::json;
use solidcraft_engine::Session;
use solidcraft_engine::geom::prelude::*;

use super::*;
use crate::Services;
use crate::dialogs::{Dialog, apply_commands, for_feature as edit_dialog};

fn app_with(cmds: &[(&str, Value)]) -> SolveApp {
    let mut s = Session::default();
    for (id, p) in cmds {
        s.execute(id, p).unwrap();
    }
    SolveApp::new(s, Services::default())
}

/// A 600 × 400 rectangle on XY as the frame sketch `Frame`, and, drawn after it on YZ, a ring of
/// two circles as the section sketch `Ring` (its circles' ids are not the frame's line ids).
fn frame_app() -> SolveApp {
    app_with(&[
        ("sketch.create", json!({"plane": "XY", "name": "Frame"})),
        ("sketch.rectangle.two_point", json!({"p0": [0, 0], "p1": [600, 400]})),
        ("sketch.finish", json!({})),
        ("sketch.create", json!({"plane": "YZ", "name": "Ring"})),
        ("sketch.circle.center", json!({"center": [0, 0], "radius": 20})),
        ("sketch.circle.center", json!({"center": [0, 0], "radius": 17})),
        ("sketch.finish", json!({})),
    ])
}

fn sketch_id(app: &SolveApp, name: &str) -> u64 {
    app.session.doc.find_feature(name).unwrap().id
}

fn frame_lines(app: &SolveApp) -> Vec<String> {
    app.session.doc.sketch(sketch_id(app, "Frame")).unwrap().curves.iter().map(|c| c.id.clone()).collect()
}

fn body_names(app: &SolveApp) -> Vec<String> {
    crate::open_meshing::session_world(&app.session).bodies.iter().map(|b| b.name.clone()).collect()
}

/// The dialog started with the given lines picked.
fn dialog_on(app: &mut SolveApp, lines: &[String]) -> Dialog {
    app.session.selection = lines.iter().map(|id| Sel::SketchCurve { id: id.clone(), sketch: None }).collect();
    app.start("frame.member");
    app.dialog.clone().unwrap()
}

fn member(d: &mut Dialog) -> &mut Member {
    match &mut d.kind {
        Kind::Frame(m) => m,
        k => panic!("{k:?}"),
    }
}

/// Picked lines and the default built-in profile make one member a line; Edit Feature refills
/// the dialog and runs `timeline.redefine`, and the members keep their names.
#[test]
fn members_from_picked_lines_and_a_built_in_profile() {
    let mut app = frame_app();
    let lines = frame_lines(&app);
    let mut d = dialog_on(&mut app, &lines[..2]);
    let c = apply_commands(&app, &d).unwrap();
    assert_eq!(c[0].0, "frame.member");
    let p = &c[0].1;
    assert_eq!(p["sketch"], json!(sketch_id(&app, "Frame")), "the frame sketch, not the newer section sketch: {p}");
    assert_eq!(p["lines"], json!(lines[..2]));
    assert_eq!(p["profile"], json!({"kind": "rect_tube", "width": "40 mm", "height": "40 mm", "wall": "3 mm"}));
    assert!(p.get("anchor").is_none() && p.get("angle").is_none() && p.get("mirror").is_none() && p.get("body_name").is_none(), "{p}");
    // Another built-in profile and its dimensions; an optional one left empty takes its default.
    let m = member(&mut d);
    m.kind = frame::profile_kinds().iter().position(|k| k.kind == "angle").unwrap();
    m.dims[m.kind][2] = "4 mm".into();
    let c = apply_commands(&app, &d).unwrap();
    assert_eq!(c[0].1["profile"], json!({"kind": "angle", "width": "50 mm", "height": "50 mm", "thickness": "4 mm"}));
    run_all(&mut app, c);
    assert_eq!(body_names(&app), ["Angle 50x50x4", "Angle 50x50x4 (2)"]);
    app.run("body.rename", json!({"body": "Angle 50x50x4 (2)", "name": "Rail"})).unwrap();
    let f = app.session.doc.features.last().unwrap().id;
    let mut e = edit_dialog(&app, f, None).unwrap();
    assert_eq!(e.inputs[0].items.len(), 2);
    let m = member(&mut e);
    assert_eq!((frame::profile_kinds()[m.kind].kind, m.dims[m.kind][2].as_str()), ("angle", "4 mm"));
    m.dims[m.kind][2] = "5 mm".into();
    let c = apply_commands(&app, &e).unwrap();
    assert_eq!(c[0].0, "timeline.redefine");
    run_all(&mut app, c);
    assert_eq!(body_names(&app), ["Angle 50x50x4", "Rail"], "the members keep their names through the edit");
}

/// A section picked in another sketch is the profile, the picks naming its regions; All Lines,
/// anchor, angle, mirror and name reach the command and come back when editing.
#[test]
fn a_picked_section_and_the_placement() {
    let mut app = frame_app();
    let lines = frame_lines(&app);
    let ring = sketch_id(&app, "Ring");
    let mut d = dialog_on(&mut app, &lines[..1]);
    // The ring between the circles: the region whose interior point lies between radii 17 and 20.
    let ss = crate::open_meshing::session_world(&app.session).sketch(ring).unwrap().clone();
    let index = ss.profiles.iter().position(|p| (p.region.interior_point().len() - 18.5).abs() < 1.5).unwrap();
    d.inputs[1].items = vec![Sel::Profile { sketch: ring, index }];
    let m = member(&mut d);
    m.all = true;
    m.anchor = ANCHORS.iter().position(|a| a.0 == "bottom").unwrap();
    m.angle = "15 deg".into();
    m.mirror = true;
    m.name = " Rail ".into();
    let c = apply_commands(&app, &d).unwrap();
    let p = &c[0].1;
    assert_eq!(p["sketch"], json!(sketch_id(&app, "Frame")));
    assert!(p.get("lines").is_none() && p.get("profile").is_none(), "{p}");
    assert_eq!((&p["profile_sketch"], &p["profiles"]), (&json!(ring), &json!([index])));
    assert_eq!((&p["anchor"], &p["angle"], &p["mirror"], &p["body_name"]), (&json!("bottom"), &json!("15 deg"), &json!(true), &json!("Rail")));
    run_all(&mut app, c);
    assert_eq!(body_names(&app), ["Rail", "Rail (2)", "Rail (3)", "Rail (4)"]);
    let f = app.session.doc.features.last().unwrap().id;
    let mut e = edit_dialog(&app, f, None).unwrap();
    assert_eq!(e.inputs[0].items.len(), 4, "every line of the sketch shows as picked");
    assert_eq!(e.inputs[1].items, [Sel::Profile { sketch: ring, index }]);
    let m = member(&mut e);
    assert!(m.all && m.mirror && m.name == "Rail" && m.angle == "15 deg" && ANCHORS[m.anchor].0 == "bottom", "{m:?}");
}

/// A member whose section is a whole profile sketch keeps it when edited: the dialog shows the
/// sketch rather than picking every region of it, which would fill the tube's hole.
#[test]
fn a_whole_sketch_section_stays_whole_through_an_edit() {
    let mut app = frame_app();
    app.run("frame.member", json!({"sketch": "Frame", "profile_sketch": "Ring"})).unwrap();
    let volume = |app: &mut SolveApp| app.session.execute("inspect.measure", &json!({})).unwrap()["bodies"][0]["volume_mm3"].as_f64().unwrap();
    let before = volume(&mut app);
    let f = app.session.doc.features.last().unwrap().id;
    let mut e = edit_dialog(&app, f, None).unwrap();
    assert!(e.inputs[1].items.is_empty());
    assert_eq!(member(&mut e).whole, Some(sketch_id(&app, "Ring")));
    member(&mut e).angle = "30 deg".into();
    let c = apply_commands(&app, &e).unwrap();
    assert!(c[0].1["params"].get("profiles").is_none(), "{}", c[0].1);
    run_all(&mut app, c);
    // To the accuracy SMLib measures at (1e-4 relative, the finest it takes: kernel
    // `measure::ACCURACY`); the ring's exact π (20² − 17²) 600 = 209230.07 mm³ came out
    // 209229.86 before the turn and 209230.43 after it. The filled circle would be 3.6 times it.
    let after = volume(&mut app);
    assert!((after - before).abs() < 1e-4 * before, "still the ring, not the filled circle: {after} mm³, {before} before");
    // Unticked, the section is a built-in profile again.
    let mut e = edit_dialog(&app, f, None).unwrap();
    member(&mut e).whole = None;
    assert_eq!(apply_commands(&app, &e).unwrap()[0].1["params"]["profile"]["kind"], "rect_tube");
}

/// A member placed by a profile point (`locate`, which the dialog has no field for) keeps it
/// through an edit, so it stays attached where it was.
#[test]
fn an_edit_keeps_a_located_point() {
    let mut app = frame_app();
    let profile = json!({"kind": "angle", "width": 60, "height": 40, "thickness": 5});
    app.run("frame.member", json!({"sketch": "Frame", "profile": profile, "locate": [10, 5]})).unwrap();
    let f = app.session.doc.features.last().unwrap().id;
    let mut e = edit_dialog(&app, f, None).unwrap();
    member(&mut e).angle = "90 deg".into();
    let c = apply_commands(&app, &e).unwrap();
    assert_eq!(c[0].1["params"]["locate"], json!([10.0, 5.0]), "{}", c[0].1);
    run_all(&mut app, c);
    let FeatureKind::FrameMember { locate, .. } = &app.session.doc.feature(f).unwrap().kind else { panic!() };
    assert_eq!(*locate, Some([10.0, 5.0]));
}

/// Each group keeps its own located point through an edit, while another group is shown and
/// after the edit runs.
#[test]
fn an_edit_keeps_each_groups_located_point() {
    let mut app = frame_app();
    let lines = frame_lines(&app);
    let profile = json!({"kind": "angle", "width": 60, "height": 40, "thickness": 5});
    let groups = json!([{"lines": [&lines[0]], "locate": [10, 5]}, {"lines": [&lines[1]], "angle": "90 deg"}]);
    app.run("frame.member", json!({"sketch": "Frame", "profile": profile, "groups": groups})).unwrap();
    let f = app.session.doc.features.last().unwrap().id;
    let mut e = edit_dialog(&app, f, None).unwrap();
    if let Kind::Frame(m) = &mut e.kind {
        m.show(1, &mut e.inputs);
        m.name = "Post".into();
        m.show(0, &mut e.inputs);
        assert_eq!(m.locate, Some([10.0, 5.0]));
    }
    let c = apply_commands(&app, &e).unwrap();
    let sent = &c[0].1["params"];
    assert!(sent.get("locate").is_none(), "{sent}");
    assert_eq!(sent["groups"][0]["locate"], json!([10.0, 5.0]), "{sent}");
    assert!(sent["groups"][1].get("locate").is_none(), "{sent}");
    run_all(&mut app, c);
    let groups = frame::member_groups(&app.session.doc.feature(f).unwrap().kind);
    assert_eq!(groups.iter().map(|g| g.locate).collect::<Vec<_>>(), [Some([10.0, 5.0]), None]);
    assert_eq!(groups[1].body_name.as_deref(), Some("Post"));
}

/// Without lines there is nothing to place; lines and the section in one sketch are refused.
#[test]
fn missing_or_mixed_picks_are_refused() {
    let mut app = frame_app();
    let d = dialog_on(&mut app, &[]);
    assert!(apply_commands(&app, &d).unwrap_err().contains("lines"));
    let ring = sketch_id(&app, "Ring");
    let circles: Vec<String> = app.session.doc.sketch(ring).unwrap().curves.iter().map(|c| c.id.clone()).collect();
    let mut d = dialog_on(&mut app, &circles[..1]);
    d.inputs[1].items = vec![Sel::Profile { sketch: ring, index: 0 }];
    assert!(apply_commands(&app, &d).is_err());
}

/// Lines picked in the view keep their sketch: two 3D sketches both number their lines `w1`…, and
/// the members go on the lines that were clicked, not on the newer sketch's lines of those ids.
#[test]
fn picked_lines_stay_in_the_sketch_they_were_picked_in() {
    let mut app = app_with(&[
        ("sketch.create", json!({"plane": "XY", "name": "Uprights"})),
        ("sketch.options", json!({"sketch_3d": true})),
        ("sketch.line3d", json!({"points": [[0, 0, 0], [0, 0, 500]]})),
        ("sketch.line3d", json!({"points": [[300, 0, 0], [300, 0, 500]]})),
        ("sketch.finish", json!({})),
        ("sketch.create", json!({"plane": "XY", "name": "Rails"})),
        ("sketch.options", json!({"sketch_3d": true})),
        ("sketch.line3d", json!({"points": [[0, 0, 250], [300, 0, 250]]})),
        ("sketch.line3d", json!({"points": [[0, 0, 480], [300, 0, 480]]})),
        ("sketch.finish", json!({})),
    ]);
    let (uprights, rails) = (sketch_id(&app, "Uprights"), sketch_id(&app, "Rails"));
    let wires = |id: u64| -> Vec<String> { app.session.doc.sketch(id).unwrap().wires.iter().map(|w| w.id.clone()).collect() };
    let ids = wires(uprights);
    assert_eq!(ids, wires(rails), "the two sketches share their line ids");
    app.start("frame.member");
    let mut d = app.dialog.clone().unwrap();
    d.inputs[0].items = ids.iter().map(|id| Sel::ExternalSketchCurve { sketch: uprights, id: id.clone() }).collect();
    let c = apply_commands(&app, &d).unwrap();
    assert_eq!(c[0].1["sketch"], json!(uprights));
    run_all(&mut app, c.clone());
    // Two posts the height of the uprights, not two rails.
    let m = app.session.execute("inspect.measure", &json!({})).unwrap();
    for b in m["bodies"].as_array().unwrap() {
        let (lo, hi) = (b["bbox"]["min"][2].as_f64().unwrap(), b["bbox"]["max"][2].as_f64().unwrap());
        assert!(lo.abs() < 1e-6 && (hi - 500.0).abs() < 1e-6, "{b}");
    }
    // Lines picked in two sketches are refused.
    d.inputs[0].items.push(Sel::ExternalSketchCurve { sketch: rails, id: ids[0].clone() });
    assert!(apply_commands(&app, &d).unwrap_err().contains("one sketch"));
}

/// Editing an end cap shows a face for each saved end. When one cannot be found, the faces start
/// empty and OK names the missing end instead of dropping it unasked; what is picked again is taken,
/// even fewer ends.
#[test]
fn an_end_cap_edit_refuses_to_drop_an_end_it_cannot_find() {
    let tube = json!({"kind": "rect_tube", "width": 40, "height": 40, "wall": 3, "outer_radius": 0});
    let mut app = app_with(&[
        ("sketch.create", json!({"plane": "XY", "name": "Post path"})),
        ("sketch.options", json!({"sketch_3d": true})),
        ("sketch.line3d", json!({"points": [[0, 0, 0], [0, 0, 300]]})),
        ("sketch.finish", json!({})),
        ("frame.member", json!({"sketch": "Post path", "body_name": "Post", "profile": tube})),
    ]);
    let end = |app: &SolveApp, z: f64| {
        let st = app.session.model.state();
        let b = st.body("Post").unwrap();
        let i = (0..b.body.face_count())
            .find(|i| b.body.planar_face(*i).is_some_and(|pl| pl.normal().z.abs() > 0.999 && (pl.origin.z - z).abs() < 1e-6))
            .unwrap();
        Sel::Face { body: "Post".into(), index: i, point: solidcore_record::Vec3::new(18.5, 0.0, z) }
    };
    let (top, bottom) = (end(&app, 300.0), end(&app, 0.0));
    let faces: Vec<Value> = [&top, &bottom]
        .iter()
        .map(|s| match s {
            Sel::Face { body, index, point } => json!({"body": body, "index": index, "face": point}),
            _ => Value::Null,
        })
        .collect();
    app.session.execute("frame.end_cap", &json!({"faces": faces, "thickness": 6})).unwrap();
    let kind = app.session.doc.find_feature("EndCap1").unwrap().kind.clone();
    let (crate::dialogs::Kind::EndCap(c), inputs, extra) = crate::dialogs_end_cap::for_feature(&app, &kind).unwrap() else { panic!() };
    assert!(c.unresolved.is_empty() && inputs[0].items.len() == 2, "{c:?}");
    assert!(crate::dialogs_end_cap::commands(&app, &c, &inputs, &extra).is_ok());
    // The second end could not be found: nothing is picked, and OK says which end is missing.
    let c = crate::dialogs_end_cap::EndCap { unresolved: vec![2], ..c };
    let mut inputs = inputs;
    inputs[0].items.clear();
    let e = crate::dialogs_end_cap::commands(&app, &c, &inputs, &extra).unwrap_err();
    assert!(e.contains('2'), "{e}");
    // Picked again, the remaining end alone is a deliberate choice, as are both.
    inputs[0].items = vec![top.clone()];
    assert!(crate::dialogs_end_cap::commands(&app, &c, &inputs, &extra).is_ok());
    inputs[0].items = vec![top, bottom];
    assert!(crate::dialogs_end_cap::commands(&app, &c, &inputs, &extra).is_ok());
}

#[test]
fn switching_groups_keeps_picks_and_placement_through_an_edit() {
    let mut app = frame_app();
    let lines = frame_lines(&app);
    let mut d = dialog_on(&mut app, &lines[..1]);
    let m = member(&mut d);
    m.all = true;
    m.name = "Rail".into();
    m.angle = "15 deg".into();
    if let Kind::Frame(m) = &mut d.kind {
        m.add_group(&mut d.inputs);
        assert!(!m.all);
        assert!(d.inputs[0].items.is_empty());
        m.name = "Post".into();
        m.angle = "90 deg".into();
        m.mirror = true;
        d.inputs[0].items = vec![Sel::ExternalSketchCurve { sketch: sketch_id(&app, "Frame"), id: lines[1].clone() }];
        m.show(0, &mut d.inputs);
        assert_eq!((m.name.as_str(), m.angle.as_str(), m.mirror), ("Rail", "15 deg", false));
        assert_eq!(d.inputs[0].items.len(), 1);
        m.show(1, &mut d.inputs);
        assert_eq!((m.name.as_str(), m.angle.as_str(), m.mirror), ("Post", "90 deg", true));
    }
    let c = apply_commands(&app, &d).unwrap();
    let original = c[0].1.clone();
    assert_eq!(original["groups"][0]["lines"], json!([lines[0]]));
    assert_eq!(original["groups"][1]["lines"], json!([lines[1]]));
    run_all(&mut app, c);
    assert_eq!(body_names(&app), ["Rail", "Post"]);
    let feature = app.session.doc.features.last().unwrap().id;
    let e = edit_dialog(&app, feature, None).unwrap();
    let c = apply_commands(&app, &e).unwrap();
    assert_eq!(c[0].1["params"]["groups"], original["groups"]);
    run_all(&mut app, c);
    assert_eq!(body_names(&app), ["Rail", "Post"]);
}

#[test]
fn group_buttons_stay_visible_and_new_group_is_clickable_at_default_dialog_width() {
    use crate::scenario::Harness;
    use solidcraft_ui_registry::{handle_at, rect_of};

    for (width, height) in [(1280, 720), (800, 600)] {
        for count in [1, 3] {
            let mut h = Harness::new();
            h.app = frame_app();
            h.call("ui.resize", json!({"width": width, "height": height}));
            let lines = frame_lines(&h.app);
            let mut d = dialog_on(&mut h.app, &lines[..1]);
            if let Kind::Frame(m) = &mut d.kind {
                for _ in 1..count {
                    m.add_group(&mut d.inputs);
                }
                m.show(0, &mut d.inputs);
            }
            h.app.dialog = Some(d);
            h.frames(3);

            let visible = rect_of("dialog:card").unwrap().intersect(h.ctx.content_rect());
            let row = rect_of("dialog:row:Groups").unwrap();
            if count == 1 {
                assert!(rect_of("dialog:row:All Lines").unwrap().bottom() <= row.top());
            }
            for label in (1..=count).map(|k| format!("Group {k}")).chain(["New Group".into()]) {
                let key = format!("dialog:button:Groups:{label}");
                let rect = rect_of(&key).unwrap();
                assert!(
                    visible.contains_rect(rect) && row.contains_rect(rect),
                    "{width}x{height}, {count} groups: {key} {rect:?}, row {row:?}, visible {visible:?}"
                );
                assert!(visible.contains(handle_at(&key).unwrap()));
            }
            for k in 1..=count {
                crate::test_support::click(&mut h, &format!("dialog:button:Groups:Group {k}"));
                assert_eq!(member(h.app.dialog.as_mut().unwrap()).current, k - 1);
            }
            crate::test_support::click(&mut h, "dialog:button:Groups:New Group");
            let d = h.app.dialog.as_mut().unwrap();
            assert_eq!(member(d).groups.len(), count + 1, "{width}x{height}, {count} groups: New Group must respond to a click");
            assert_eq!(member(d).current, count);
            assert!(d.inputs[0].items.is_empty());
        }
    }
}

#[test]
fn removing_the_shown_group_keeps_its_neighbours_values() {
    let mut app = frame_app();
    let lines = frame_lines(&app);
    let mut d = dialog_on(&mut app, &lines[..1]);
    member(&mut d).name = "Removed".into();
    if let Kind::Frame(m) = &mut d.kind {
        m.add_group(&mut d.inputs);
        m.name = "Kept".into();
        m.angle = "45 deg".into();
        d.inputs[0].items = vec![Sel::ExternalSketchCurve { sketch: sketch_id(&app, "Frame"), id: lines[1].clone() }];
        m.show(0, &mut d.inputs);
        m.remove_group(&mut d.inputs);
        assert_eq!(m.groups.len(), 1);
        assert_eq!((m.name.as_str(), m.angle.as_str()), ("Kept", "45 deg"));
        m.remove_group(&mut d.inputs);
        assert_eq!(m.groups.len(), 1, "the last group cannot be removed");
    }
    let c = apply_commands(&app, &d).unwrap();
    assert!(c[0].1.get("groups").is_none());
    assert_eq!(c[0].1["lines"], json!([lines[1]]));
    assert_eq!(c[0].1["body_name"], "Kept");
    run_all(&mut app, c);
    assert_eq!(body_names(&app), ["Kept"]);
}
