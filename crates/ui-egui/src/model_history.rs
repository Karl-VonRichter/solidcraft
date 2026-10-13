//! Feature history in the model browser. The engine remains responsible for edits,
//! dependency checks, suppression and rollback; these rows share the timeline's commands.

use egui::{Sense, Stroke, vec2};
use serde_json::json;
use solidcraft_engine::{Sel, doc::FeatureKind};

use crate::{SolveApp, browser::Row, theme::Tokens};

#[derive(Clone, Copy)]
enum HistoryDrag {
    Feature(u64),
    Marker,
}

fn drop_at(app: &mut SolveApp, response: &egui::Response, slot: usize) {
    if response.dnd_hover_payload::<HistoryDrag>().is_some() {
        response
            .ctx
            .layer_painter(response.layer_id)
            .line_segment([response.rect.left_top(), response.rect.right_top()], Stroke::new(2.0, Tokens::get().accent));
    }
    if let Some(drag) = response.dnd_release_payload::<HistoryDrag>() {
        match *drag {
            HistoryDrag::Marker => {
                let params = if slot >= app.session.doc.features.len() { json!({}) } else { json!({"position":slot}) };
                let _ = app.run("timeline.roll_to", params);
            }
            HistoryDrag::Feature(id) => {
                if let Some(from) = app.session.doc.feature_index(id) {
                    let to = if slot > from { slot.saturating_sub(1) } else { slot };
                    if to != from {
                        let _ = app.run("timeline.reorder", json!({"feature":id,"position":to}));
                    }
                }
            }
        }
    }
}

fn marker(app: &mut SolveApp, ui: &mut egui::Ui, depth: usize, slot: usize) {
    let row = Row { depth, icon: "finish", label: "End of Part", ..Default::default() };
    let result = crate::browser::draw_row(ui, ui.id().with("history_marker"), &row);
    if let Some(response) = result.resp {
        crate::scenario::publish_handle("history:marker", response.rect.center());
        if response.drag_started() {
            response.dnd_set_drag_payload(HistoryDrag::Marker);
        }
        drop_at(app, &response, slot);
        response.context_menu(|ui| {
            if ui.button("Roll to End").clicked() {
                let _ = app.run("timeline.roll_to", json!({}));
                ui.close();
            }
        });
        response.on_hover_text("Drag above a feature to roll back; drag below it to restore. New features are inserted here.");
    }
}

pub(crate) fn rows(app: &mut SolveApp, ui: &mut egui::Ui, component: u64, depth: usize) {
    // Copy the feature metadata before executing any commands from a row.
    let features: Vec<_> = app
        .session
        .doc
        .features
        .iter()
        .enumerate()
        .filter(|(_, f)| f.component == component && !matches!(f.kind, FeatureKind::Sketch { .. } | FeatureKind::ConstructionPlane { .. }))
        .map(|(i, f)| (i, f.clone()))
        .collect();
    let end = app.session.doc.features.len();
    let position = app.session.doc.marker.unwrap_or(end);
    let show_marker = component == app.session.active_component;
    let mut marker_shown = false;
    let mut edit = None;
    for (index, feature) in features {
        if show_marker && !marker_shown && index >= position {
            marker(app, ui, depth, index);
            marker_shown = true;
        }
        let id = feature.id;
        let selected = app.session.selection.iter().any(|s| matches!(s, Sel::Feature { id: picked } if *picked == id));
        let outcome = app.session.model.result(id);
        let error = outcome.and_then(|r| r.error.clone());
        let warning = outcome.and_then(|r| r.warning.clone());
        let badges: &[&str] = if error.is_some() || warning.is_some() { &["warning"] } else { &[] };
        let row = Row {
            depth,
            icon: crate::timeline::icon_of(&feature.kind),
            label: &feature.name,
            selected,
            color: if error.is_some() {
                Some(Tokens::get().error)
            } else if warning.is_some() {
                Some(Tokens::get().warning)
            } else {
                None
            },
            dim: feature.suppressed || index >= position,
            badges,
            ..Default::default()
        };
        let result = crate::browser::draw_row(ui, ui.id().with(("history_feature", id)), &row);
        if result.hovered {
            app.viewport.hover_feature = Some(id);
        }
        if result.clicked {
            let takes_features = app.dialog.as_ref().and_then(|d| d.active_input()).is_some_and(|i| i.accept & crate::selection::FEATURES != 0);
            if takes_features {
                if let Some(mut dialog) = app.dialog.take() {
                    dialog.pick(&app.session, Sel::Feature { id });
                    app.dialog = Some(dialog);
                }
            } else {
                let _ = app.run("select.set", json!({"items":[{"type":"feature","id":id}]}));
            }
        }
        if result.double {
            edit = Some(id);
        }
        if let Some(response) = result.resp {
            crate::scenario::publish_handle(&format!("history:{id}"), response.rect.center());
            crate::scenario::publish_handle(&format!("history:{}", feature.name), response.rect.center());
            // Existing automation can select features even with the horizontal strip hidden.
            if !app.ui.show_timeline {
                crate::scenario::publish_handle(&format!("timeline:{}", feature.name), response.rect.center());
            }
            if response.drag_started() {
                response.dnd_set_drag_payload(HistoryDrag::Feature(id));
            }
            let slot = if ui.input(|i| i.pointer.latest_pos()).is_some_and(|p| p.y > response.rect.center().y) { index + 1 } else { index };
            drop_at(app, &response, slot);
            response.context_menu(|ui| {
                if ui.button("Edit Feature").clicked() {
                    edit = Some(id);
                    ui.close();
                }
                let sketch = match &feature.kind {
                    FeatureKind::Extrude { sketch, .. } | FeatureKind::Revolve { sketch, .. } | FeatureKind::Sweep { sketch, .. } => Some(*sketch),
                    _ => None,
                };
                if let Some(sketch) = sketch
                    && ui.button("Edit Sketch").clicked()
                {
                    app.edit_sketch(sketch);
                    ui.close();
                }
                if ui.button("Rename").clicked() {
                    app.dialog = Some(crate::dialogs::Dialog::rename(id, &feature.name));
                    ui.close();
                }
                if ui.button(if feature.suppressed { "Unsuppress" } else { "Suppress" }).clicked() {
                    let _ = app.run("timeline.suppress", json!({"feature":id}));
                    ui.close();
                }
                if ui.button("Move End of Part Here").clicked() {
                    let _ = app.run("timeline.roll_to", json!({"feature":id}));
                    ui.close();
                }
                if ui.button("Delete").clicked() {
                    crate::timeline::delete_feature(app, id);
                    ui.close();
                }
            });
            let mut tip = format!("{}\nDouble-click to edit; drag to reorder; right-click for more", feature.name);
            if let Some(error) = error {
                tip.push_str(&format!("\nError: {error}"));
            }
            if let Some(warning) = warning {
                tip.push_str(&format!("\nWarning: {warning}"));
            }
            response.on_hover_text(tip);
        }
    }
    if show_marker && !marker_shown {
        marker(app, ui, depth, end);
    }
    // A drop below the last row restores all features or moves a feature to the end.
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 10.0), Sense::hover());
    let response = ui.interact(rect, ui.id().with("history_end"), Sense::hover());
    drop_at(app, &response, end);
    if let Some(id) = edit {
        app.edit_feature(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::Harness;

    #[test]
    fn browser_feature_edits_restore_later_history_after_cancel() {
        let mut h = Harness::new();
        h.app.run("sketch.create", json!({"plane":"XY"})).unwrap();
        h.app.run("sketch.rectangle.two_point", json!({"p0":[0,0],"p1":[20,10]})).unwrap();
        h.app.run("sketch.finish", json!({})).unwrap();
        h.app.run("solid.extrude", json!({"distance":8})).unwrap();
        let id = h.app.session.doc.features.last().unwrap().id;
        h.frames(4);
        assert!(!h.app.ui.show_timeline);
        h.step(&json!({"click":{"handle":format!("history:{id}")}})).unwrap();
        assert!(h.app.session.selection.iter().any(|s| matches!(s, Sel::Feature { id: picked } if *picked == id)));
        h.step(&json!({"click":{"handle":format!("history:{id}")},"double":true})).unwrap();
        assert!(h.app.dialog.is_some());
        assert_eq!(h.app.session.doc.marker, Some(1));
        h.step(&json!({"key":"Escape"})).unwrap();
        assert!(h.app.dialog.is_none());
        assert_eq!(h.app.session.doc.marker, None);
        assert_eq!(h.app.session.world_state().bodies.len(), 1);
    }

    #[test]
    fn browser_end_of_part_rolls_back_and_restores_using_drag() {
        let mut h = Harness::new();
        h.app.run("solid.box", json!({"length":10,"width":10,"height":10})).unwrap();
        let id = h.app.session.doc.features.first().unwrap().id;
        h.frames(4);
        let row = crate::scenario::handle_at(&format!("history:{id}")).unwrap();
        h.step(&json!({"drag":[{"handle":"history:marker"},[row.x,row.y-7.0]],"steps":12})).unwrap();
        assert_eq!(h.app.session.doc.marker, Some(0));
        assert!(h.app.session.world_state().bodies.is_empty());
        h.frames(3);
        let row = crate::scenario::handle_at(&format!("history:{id}")).unwrap();
        h.step(&json!({"drag":[{"handle":"history:marker"},[row.x,row.y+7.0]],"steps":12})).unwrap();
        assert_eq!(h.app.session.doc.marker, None);
        assert_eq!(h.app.session.world_state().bodies.len(), 1);
    }
}
