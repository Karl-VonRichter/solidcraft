//! Global preferences, new-sketch defaults and saved sketch overrides have distinct scopes.

use serde_json::json;

use crate::{SolveApp, documents, prefs, scenario::Harness};

#[test]
fn sketch_workflow_preferences_follow_new_switched_and_opened_documents() {
    let mut h = Harness::new();
    h.app.run("sketch.create", json!({})).unwrap();
    h.app.run("sketch.finish", json!({})).unwrap();
    h.app.run("sketch.auto_project", json!({"value":false})).unwrap();
    h.app.run("sketch.defaults", json!({"snap":false,"show_dimensions":false})).unwrap();
    documents::new_design(&mut h.app);
    assert!(!h.app.session.auto_project);
    assert!(!h.app.session.sketch_defaults.snap);
    documents::switch(&mut h.app, 0);
    let first = h.app.session.doc.features[0].id;
    assert!(!h.app.session.doc.sketch(first).unwrap().view.hide_dimensions);
    assert!(!h.app.session.auto_project);

    // Opening a saved design retains its units/materials/options, but gets global workflow choices.
    h.app.run("document.units", json!({"units":"in"})).unwrap();
    h.app.run("material.create", json!({"name":"Example Material","density_g_cm3":1.1})).unwrap();
    let saved = h.app.session.doc.clone();
    let path = h.dir.join("scope.solidcraft");
    std::fs::write(&path, solidcraft_engine::io::write_design(&saved)).unwrap();
    h.app.open_path_async(path.to_str().unwrap());
    h.settle();
    assert!(!h.app.session.auto_project);
    assert_eq!(h.app.session.doc.units, "in");
    assert_eq!(h.app.session.doc.user_materials, saved.user_materials);
    assert_eq!(h.app.session.doc.sketch(first).unwrap().view, saved.sketch(first).unwrap().view);
    let second = h.app.run("sketch.create", json!({})).unwrap()["sketch"].as_u64().unwrap();
    assert!(h.app.session.doc.sketch(second).unwrap().view.hide_dimensions);
    assert!(h.app.session.doc.sketch(second).unwrap().view.no_snap);
    assert!(!h.app.preferences.auto_project);
    let mut restarted = SolveApp::new(Default::default(), Default::default());
    restarted.load_prefs(&h.app.prefs());
    assert!(!restarted.session.auto_project);
    assert_eq!(restarted.session.sketch_defaults, h.app.preferences.sketch_defaults);
    assert!(!restarted.session.is_dirty());
}

#[test]
fn cancel_apply_and_restore_defaults_work_even_after_switching_documents() {
    let mut h = Harness::new();
    h.app.run("sketch.create", json!({})).unwrap();
    h.app.run("sketch.finish", json!({})).unwrap();
    documents::new_design(&mut h.app);
    h.call("ui.prefs", json!({"open":"Sketch","auto_project":false,"sketch_defaults":{"snap":false}}));
    documents::switch(&mut h.app, 0);
    assert!(!h.app.session.auto_project && !h.app.session.sketch_defaults.snap);
    h.call("ui.prefs", json!({"button":"apply"}));
    h.call("ui.prefs", json!({"button":"defaults"}));
    assert!(h.app.session.auto_project && h.app.session.sketch_defaults.snap);
    documents::switch(&mut h.app, 1);
    h.call("ui.prefs", json!({"button":"cancel"}));
    assert!(!h.app.session.auto_project && !h.app.session.sketch_defaults.snap);
    documents::switch(&mut h.app, 0);
    assert!(!h.app.session.auto_project && !h.app.session.sketch_defaults.snap);
}

#[test]
fn legacy_auto_projection_migrates_and_global_ui_choices_persist() {
    let mut app = SolveApp::new(Default::default(), Default::default());
    app.load_prefs(&json!({"auto_project":false,"preferences":{"default_units":"cm"}}).to_string());
    assert!(!app.preferences.auto_project && !app.session.auto_project);
    assert_eq!(app.session.doc.units, "cm");
    app.ui.pick_off = vec!["edges".into(), "sketch".into(), "dimensions".into(), "constraints".into()];
    app.ui.show_browser = false;
    app.ui.show_timeline = false;
    app.ui.show_origin = false;
    app.ui.show_sketches = false;
    let mut other = SolveApp::new(Default::default(), Default::default());
    other.load_prefs(&app.prefs());
    assert_eq!(other.ui.pick_off, app.ui.pick_off);
    assert!(!other.ui.show_browser && !other.ui.show_timeline && !other.ui.show_origin && !other.ui.show_sketches);
    other.load_prefs(&json!({"auto_project":false,"preferences":{"auto_project":true}}).to_string());
    assert!(other.session.auto_project, "the current preference takes precedence over the legacy key");
}

#[test]
fn new_blank_document_applies_default_units_without_changing_saved_empty_documents() {
    let mut app = SolveApp::new(Default::default(), Default::default());
    prefs::set(&mut app, &json!({"default_units":"cm"})).unwrap();
    documents::new_design(&mut app);
    assert_eq!(app.session.doc.units, "cm");
    assert!(!app.session.is_dirty());
    app.session.path = Some("saved-empty.solidcraft".into());
    prefs::set(&mut app, &json!({"default_units":"in"})).unwrap();
    assert_eq!(app.session.doc.units, "cm");
    documents::new_design(&mut app);
    assert_eq!(app.session.doc.units, "in");
    documents::switch(&mut app, 0);
    assert_eq!(app.session.doc.units, "cm");
}
