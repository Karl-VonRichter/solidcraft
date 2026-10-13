#![allow(clippy::unwrap_used)]
use super::*;
use crate::scenario::Harness;

fn step(h: &mut Harness, value: Value) {
    h.step(&value).unwrap();
}
fn fixture() -> Harness {
    let mut h = Harness::new();
    h.app.preferences.look_at_anim = false;
    for (command, params) in [
        ("sketch.create", json!({"plane":"XY"})),
        ("sketch.line", json!({"points":[[-30,10],[-15,10]]})),
        ("sketch.line", json!({"points":[[-15,10],[-15,25]]})),
        ("sketch.line", json!({"points":[[0,-10],[0,40]],"construction":true})),
    ] {
        step(&mut h, json!({"call":"engine.execute","params":{"command":command,"params":params}}));
    }
    step(&mut h, json!({"call":"ui.view","params":{"view":"top"}}));
    h
}
fn pick(h: &mut Harness, xy: [f64; 2], ctrl: bool) {
    step(h, json!({"click":{"sketch":xy},"ctrl":ctrl}));
}
fn preview(h: &mut Harness, curves: usize) {
    step(h, json!({"until":{"count":{"name":"sketch_preview_curves","value":curves}}}));
}

#[test]
fn sketch_dialog_mirror_toggles_clears_references_previews_and_cancels() {
    let mut h = fixture();
    let original = h.app.session.doc.to_json();
    let undo = h.app.session.undo.len();
    let revision = h.app.session.revision;
    step(&mut h, json!({"start":"sketch.mirror"}));
    assert!(h.app.tool.is_none());
    pick(&mut h, [-22.5, 10.0], false);
    pick(&mut h, [-15.0, 17.5], false);
    pick(&mut h, [-22.5, 10.0], true);
    assert_eq!(h.app.dialog.as_ref().unwrap().inputs[0].items.len(), 1);
    pick(&mut h, [-22.5, 10.0], true);
    step(&mut h, json!({"call":"ui.dialogInput","params":{"index":1}}));
    pick(&mut h, [0.0, 20.0], false);
    preview(&mut h, 2);
    assert_eq!(h.app.session.doc.to_json(), original);
    assert_eq!(h.app.session.undo.len(), undo);
    assert_eq!(h.app.session.revision, revision);
    step(&mut h, json!({"click":{"handle":"dialog:clear:Mirror Line"}}));
    assert!(h.app.dialog.as_ref().unwrap().inputs[1].items.is_empty());
    assert_eq!(h.app.dialog.as_ref().unwrap().inputs[0].items.len(), 2);
    pick(&mut h, [0.0, 20.0], false);
    preview(&mut h, 2);
    step(&mut h, json!({"key":"Escape"}));
    h.frames(3);
    assert!(h.app.dialog.is_none());
    assert!(!h.app.preview.active);
    assert_eq!(h.app.session.doc.to_json(), original);
    assert_eq!(h.app.session.undo.len(), undo);
}

#[test]
fn sketch_dialog_patterns_update_counts_and_place_an_unsnapped_centre() {
    let mut h = fixture();
    step(&mut h, json!({"start":"sketch.pattern.rectangular"}));
    pick(&mut h, [-22.5, 10.0], false);
    if let crate::dialogs::Kind::SketchModify(SketchModify::Rect { count, count2, .. }) = &mut h.app.dialog.as_mut().unwrap().kind {
        *count = "4".into();
        *count2 = "2".into();
    }
    h.frames(3);
    preview(&mut h, 7);
    step(&mut h, json!({"click":{"handle":"dialog:OK"}}));
    assert_eq!(h.app.session.world_state().sketch(1).unwrap().sketch.curves.len(), 10);
    step(&mut h, json!({"start":"sketch.pattern.circular"}));
    pick(&mut h, [-22.5, 10.0], false);
    step(&mut h, json!({"call":"ui.dialogInput","params":{"index":1}}));
    pick(&mut h, [-40.0, -20.0], false);
    if let crate::dialogs::Kind::SketchModify(SketchModify::Circular { count, angle }) = &mut h.app.dialog.as_mut().unwrap().kind {
        *count = "4".into();
        *angle = "180 deg".into();
    }
    h.frames(3);
    preview(&mut h, 3);
    step(&mut h, json!({"click":{"handle":"dialog:OK"}}));
    assert_eq!(h.app.session.world_state().sketch(1).unwrap().sketch.curves.len(), 13);
}

#[test]
fn sketch_dialog_offset_precise_distance_flip_and_editable_dimension() {
    let mut h = fixture();
    step(&mut h, json!({"start":"sketch.offset"}));
    pick(&mut h, [-22.5, 10.0], false);
    if let crate::dialogs::Kind::SketchModify(SketchModify::Offset { distance, flip, chain }) = &mut h.app.dialog.as_mut().unwrap().kind {
        *distance = "2 mm".into();
        *flip = true;
        *chain = false;
    }
    h.frames(3);
    preview(&mut h, 1);
    step(&mut h, json!({"click":{"handle":"dialog:OK"}}));
    let param = h.app.session.doc.params.iter().find(|p| p.name.starts_with('d')).unwrap().name.clone();
    step(&mut h, json!({"call":"engine.execute","params":{"command":"parameters.change","params":{"name":param,"expression":"3 mm"}}}));
    let state = h.app.session.world_state();
    let sk = &state.sketch(1).unwrap().sketch;
    assert_eq!(sk.offsets[0].distance, -3.0);
    assert!(sk.constraints.iter().any(|c| matches!(c.kind, solidcraft_engine::sketch::ConstraintKind::OffsetDistance { .. })));
}

#[test]
fn sketch_dialog_mirror_refuses_external_and_curved_references() {
    let mut h = fixture();
    h.app.start("sketch.mirror");
    let d = h.app.dialog.as_mut().unwrap();
    d.active = 1;
    assert!(candidate(d, &h.app.session, &Hit::SketchCurve { sketch: 2, id: "l1".into(), straight: true }).is_none());
    assert!(candidate(d, &h.app.session, &Hit::SketchCurve { sketch: 1, id: "c1".into(), straight: false }).is_none());
}

#[test]
fn sketch_dialog_box_selection_adds_replaces_and_preserves_reference() {
    let mut h = fixture();
    step(&mut h, json!({"call":"engine.execute","params":{"command":"sketch.line","params":{"points":[[-50,-20],[-40,-20]]}}}));
    step(&mut h, json!({"start":"sketch.mirror"}));
    step(&mut h, json!({"call":"ui.dialogInput","params":{"index":1}}));
    pick(&mut h, [0.0, 20.0], false);
    step(&mut h, json!({"call":"ui.dialogInput","params":{"index":0}}));
    step(&mut h, json!({"drag":[{"sketch":[-32,8]},{"sketch":[-13,27]}]}));
    let sources = |h: &Harness| {
        h.app.dialog.as_ref().unwrap().inputs[0]
            .items
            .iter()
            .filter_map(|s| if let Sel::SketchCurve { id } = s { Some(id.clone()) } else { None })
            .collect::<Vec<_>>()
    };
    assert_eq!(sources(&h), vec!["l1", "l2"]);
    step(&mut h, json!({"drag":[{"sketch":[-52,-22]},{"sketch":[-38,-18]}],"shift":true}));
    assert_eq!(sources(&h), vec!["l1", "l2", "l4"]);
    step(&mut h, json!({"drag":[{"sketch":[-32,8]},{"sketch":[-13,12]}]}));
    assert_eq!(sources(&h), vec!["l1"]);
    assert_eq!(h.app.dialog.as_ref().unwrap().inputs[1].items, vec![Sel::SketchCurve { id: "l3".into() }]);
    step(&mut h, json!({"key":"Escape"}));
    step(&mut h, json!({"start":"sketch.offset"}));
    step(&mut h, json!({"drag":[{"sketch":[-32,8]},{"sketch":[-13,27]}]}));
    assert_eq!(h.app.dialog.as_ref().unwrap().inputs[0].items.len(), 2, "Offset accepts curves, not their endpoints or dimensions");
}
