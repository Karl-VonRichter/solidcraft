//! Application bar, the workspace toolbar (tabs and panels) and keyboard shortcuts.

use egui::{Align2, Color32, FontId, Rect, Sense, Stroke, pos2, vec2};
use serde_json::json;

use crate::theme::Tokens;
use crate::{SolveApp, icons};

pub const TABS: &[&str] = &["SOLID", "SKETCH", "ASSEMBLY", "INSPECT", "UTILITIES", "SHEET METAL", "PLASTIC", "SURFACE", "MESH"];

/// Display names do not change command identifiers, scripts or saved toolbar preferences.
fn tab_label(tab: &str) -> &str {
    match tab {
        "SOLID" => "3D Model",
        "SKETCH" => "Sketch",
        "ASSEMBLY" => "Assemble",
        "INSPECT" => "Inspect",
        "UTILITIES" => "Manage",
        "SHEET METAL" => "Sheet Metal",
        "PLASTIC" => "Plastic",
        "SURFACE" => "Surface",
        "MESH" => "Mesh",
        _ => tab,
    }
}

fn panel_label(panel: &str) -> &str {
    match panel {
        "SKETCH" => "Sketch",
        "CREATE" => "Create",
        "MODIFY" => "Modify",
        "CONSTRUCT" => "Work Features",
        "PATTERN" => "Pattern",
        "CONSTRAINTS" => "Constrain",
        "CONFIGURE" => "Parameters",
        "ASSEMBLE" => "Relationships",
        "INSPECT" => "Measure",
        "INSERT" => "Insert",
        "SELECT" => "Select",
        "FINISH SKETCH" => "Finish",
        "MATERIAL" => "Appearance",
        _ => panel,
    }
}

fn command_label<'a>(id: &str, fallback: &'a str) -> &'a str {
    match id {
        "sketch.create" => "Start 2D Sketch",
        "sketch.finish" => "Finish Sketch",
        "sketch.rectangle.two_point" => "Rectangle",
        "sketch.circle.center" => "Circle",
        "sketch.arc.three_point" => "Arc",
        "sketch.dimension" => "Dimension",
        "sketch.constraint.horizontal_vertical" => "Horizontal / Vertical",
        "parameters.change" => "Parameters",
        "config.table" => "Configurations",
        "construct.plane.offset" => "Plane",
        "construct.axis.edge" => "Axis",
        "construct.point.vertex" => "Point",
        "solid.pattern.rectangular" | "sketch.pattern.rectangular" => "Rectangular",
        "solid.pattern.circular" | "sketch.pattern.circular" => "Circular",
        _ => fallback,
    }
}

/// Primary tools are tall buttons; supporting tools occupy three-row columns.
fn large_buttons(tab: &str, panel: &str) -> usize {
    match (tab, panel) {
        (_, "SKETCH" | "FINISH SKETCH" | "CONSTRUCT" | "PATTERN" | "CONSTRAINTS") => 1,
        ("SKETCH", "CREATE") => 4,
        ("SKETCH", "MODIFY") => 0,
        _ => 2,
    }
}

/// Panels shown on each tab (in order). Commands come from the registry by (tab, panel).
pub fn panels(tab: &str) -> &'static [&'static str] {
    if let Some(p) = crate::workspace::panels(tab) {
        return p;
    }
    match tab {
        "SOLID" => &["CREATE", "MODIFY", "ASSEMBLE", "CONSTRUCT", "INSPECT", "INSERT", "SELECT"],
        "SKETCH" => &["CREATE", "MODIFY", "CONSTRAINTS", "INSPECT", "INSERT", "SELECT", "FINISH SKETCH"],
        "SURFACE" => &["CREATE", "MODIFY", "ASSEMBLE", "CONSTRUCT", "INSPECT", "INSERT", "SELECT"],
        "MESH" => &["CREATE", "PREPARE", "MODIFY", "ASSEMBLE", "CONSTRUCT", "INSPECT", "INSERT", "SELECT"],
        "SHEET METAL" => &["CREATE", "MODIFY", "ASSEMBLE", "CONSTRUCT", "INSPECT", "INSERT", "SELECT"],
        "PLASTIC" => &["CREATE", "MODIFY", "ASSEMBLE", "CONSTRUCT", "INSPECT", "INSERT", "SELECT"],
        _ => &["MAKE", "ADD-INS", "UTILITY", "INSPECT", "SELECT"],
    }
}

/// How many commands of a panel get a big button.
pub fn promoted(tab: &str, panel: &str) -> usize {
    match (tab, panel) {
        ("SOLID", "CREATE") => 4,
        ("SOLID", "MODIFY") => 4,
        ("SKETCH", "CREATE") => 6,
        ("SKETCH", "CONSTRAINTS") => 6,
        (_, "FINISH SKETCH") => 1,
        _ => 2,
    }
}

fn report_rect(bar: Rect, captions: f32, tabs_start: f32) -> (Rect, bool) {
    let right = bar.right() - captions - 90.0;
    let labeled = right - tabs_start >= 340.0;
    let width = if labeled { 124.0 } else { 28.0 };
    (Rect::from_min_size(pos2(right - width, bar.top() + 4.0), vec2(width, 26.0)), labeled)
}

pub fn app_bar(app: &mut SolveApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    egui::Panel::top("sc_appbar").exact_size(34.0).show_separator_line(false).frame(egui::Frame::NONE.fill(t.app_bar)).show(ui, |ui| {
        let r = ui.max_rect();
        // The bar is the window's title bar: its empty space moves the window.
        if app.custom_titlebar || app.integrated_titlebar {
            crate::titlebar::drag_area(ui, r);
        }
        let left = r.left() + if app.integrated_titlebar { crate::titlebar::TRAFFIC_LIGHTS } else { 0.0 };
        // Keep the current header controls and product name when merging the new mark.
        let compact_actions = r.right() - left < 640.0;
        // Brand mark: the SolidCraft logo, drawn in code from the SVG's own coordinates
        // (viewBox 357 x 412). Same 20 px height and centre as the old block.
        {
            let c = pos2(left + 20.0, r.center().y);
            let s = 20.0 / 412.217;
            let m =
                |pts: &[(f32, f32)]| -> Vec<egui::Pos2> { pts.iter().map(|&(x, y)| pos2(c.x + (x - 178.5) * s, c.y + (y - 206.1) * s)).collect() };
            let p = ui.painter();
            // egui only fills convex shapes, so each face is a few overlapping convex pieces.
            let face = |col: Color32, pts: &[(f32, f32)]| {
                p.add(egui::Shape::convex_polygon(m(pts), col, Stroke::NONE));
            };
            let (light, mid, dark) = (Color32::from_rgb(96, 158, 236), Color32::from_rgb(58, 136, 238), Color32::from_rgb(55, 108, 197));
            face(light, &[(3.5, 105.9), (177.0, 5.8), (263.4, 55.6), (263.8, 156.0), (177.0, 406.4)]);
            face(light, &[(177.0, 206.1), (263.8, 156.0), (350.5, 206.2), (350.5, 306.3), (177.0, 406.4)]);
            face(mid, &[(3.5, 105.9), (177.0, 206.1), (177.0, 406.4)]);
            face(mid, &[(3.5, 105.9), (3.5, 205.7), (90.3, 256.2), (177.0, 206.1)]);
            face(mid, &[(177.0, 206.1), (90.3, 256.2), (90.0, 356.2), (177.0, 406.4)]);
            face(dark, &[(264.1, 155.8), (177.0, 206.1), (177.0, 406.4), (263.8, 256.2)]);
            face(dark, &[(263.8, 256.2), (350.5, 206.2), (350.5, 306.3), (177.0, 406.4)]);
            face(dark, &[(177.0, 206.1), (177.0, 406.4), (350.5, 306.3)]);
            face(dark, &[(263.8, 55.9), (263.8, 156.0), (177.1, 105.9)]);
            // White edge lines.
            let edge = Stroke::new(10.0 * s, Color32::WHITE);
            let line = |pts: &[(f32, f32)]| {
                p.add(egui::Shape::line(m(pts), edge));
            };
            line(&[(263.8, 55.9), (177.1, 105.9), (263.8, 156.0), (263.8, 256.2), (350.6, 206.1)]);
            line(&[(5.0, 105.9), (178.5, 206.1), (178.5, 406.4)]);
            line(&[(177.0, 206.1), (263.8, 156.0)]);
            p.add(egui::Shape::closed_line(
                m(&[
                    (178.5, 5.8),
                    (5.0, 105.9),
                    (5.0, 206.1),
                    (91.7, 256.2),
                    (91.7, 356.4),
                    (178.5, 406.4),
                    (352.0, 306.3),
                    (352.0, 206.1),
                    (265.2, 156.0),
                    (265.2, 55.9),
                ]),
                edge,
            ));
        }
        if !compact_actions {
            ui.painter().text(pos2(left + 36.0, r.center().y), Align2::LEFT_CENTER, "SolidCraft", FontId::proportional(14.0), t.app_bar_text);
        }
        let mut x = left + if compact_actions { 60.0 } else { 128.0 };
        let mut click = |ui: &mut egui::Ui, icon: &str, tip: &str| -> bool {
            let width = if compact_actions { 22.0 } else { 26.0 };
            let br = Rect::from_center_size(pos2(x, r.center().y), vec2(width, 26.0));
            x += if compact_actions { 22.0 } else { 30.0 };
            let resp = ui.interact(br, ui.id().with(("ab", icon)), Sense::click());
            crate::scenario::publish_handle(&format!("appbar:{icon}"), br.center());
            if resp.hovered() {
                ui.painter().rect_filled(br, 4.0, Color32::from_white_alpha(30));
            }
            icons::paint(
                ui.painter(),
                br.shrink(5.0),
                icon,
                Color32::from_rgb(225, 230, 238),
                Color32::from_rgb(120, 140, 170),
                Color32::from_rgb(120, 190, 255),
            );
            resp.on_hover_text(tip).clicked()
        };
        let file = click(ui, "folder", app.tr("ui.file"));
        if click(ui, "save", &format!("{} (Ctrl+S)", app.tr("ui.save"))) {
            save(app);
        }
        if click(ui, "undo", &format!("{} (Ctrl+Z)", app.tr("ui.undo"))) {
            let _ = app.run("edit.undo", json!({}));
        }
        if click(ui, "redo", &format!("{} (Ctrl+Y)", app.tr("ui.redo"))) {
            let _ = app.run("edit.redo", json!({}));
        }
        if click(ui, "home", app.tr("ui.home")) {
            let rev = app.session.revision;
            app.home.show(rev);
        }
        let theme_tip = if app.ui.dark { app.tr("ui.light") } else { app.tr("ui.dark") };
        if !compact_actions && click(ui, if app.ui.dark { "sun" } else { "moon" }, theme_tip) {
            app.ui.dark = !app.ui.dark;
            app.preferences.theme = if app.ui.dark { "dark" } else { "light" }.into();
        }
        if click(ui, "help", app.tr("ui.help")) {
            app.help.menu = !app.help.menu;
            let at = ui.ctx().input(|i| i.pointer.latest_pos()).map_or(300.0, |p| p.x);
            ui.ctx().data_mut(|d| {
                d.insert_temp(egui::Id::new("sc_help_opened"), true);
                d.insert_temp(egui::Id::new("sc_help_anchor"), at);
            });
        }
        if file {
            app.ui.palette_open = false;
            let ctx = ui.ctx().clone();
            let open = ctx.data(|d| d.get_temp::<bool>(egui::Id::new("sc_file_menu")).unwrap_or(false));
            let pass = ctx.cumulative_pass_nr();
            // The folder button toggles the menu. Remember the pass it opened on so the same
            // click isn't also treated as a click outside the menu (which would close it at once).
            ctx.data_mut(|d| {
                d.insert_temp(egui::Id::new("sc_file_menu"), !open);
                d.insert_temp(egui::Id::new("sc_file_menu_opened"), pass);
            });
        }
        // The title bar keeps the active document name; document tabs live below the view.
        let captions = if app.custom_titlebar { crate::titlebar::WIDTH } else { 0.0 };
        let tabs_start = x + 16.0;
        let (report, report_label) = report_rect(r, captions, tabs_start);
        let tabs_end = report.left() - 8.0;
        let tabs_rect = Rect::from_min_max(pos2(tabs_start.min(tabs_end), r.top()), pos2(tabs_end, r.bottom()));
        ui.painter().with_clip_rect(tabs_rect).text(
            tabs_rect.center(),
            Align2::CENTER_CENTER,
            &app.session.doc.name,
            FontId::proportional(13.0),
            t.app_bar_text,
        );
        let response = ui.interact(report, ui.id().with("sc_report_button"), Sense::click());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), "Report a bug"));
        crate::scenario::publish_handle("titlebar:report", report.center());
        ui.painter().rect_filled(report, 4.0, if response.hovered() || response.has_focus() { t.hover } else { t.app_bar });
        ui.painter().rect_stroke(report, 4.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
        let icon = Rect::from_center_size(pos2(report.left() + 14.0, report.center().y), vec2(18.0, 18.0));
        icons::paint(ui.painter(), icon, "bug_report", t.app_bar_text, t.app_bar_text, t.app_bar);
        if report_label {
            ui.painter().text(
                pos2(report.left() + 29.0, report.center().y),
                Align2::LEFT_CENTER,
                "Report a bug",
                FontId::proportional(13.0),
                t.app_bar_text,
            );
        }
        if response.on_hover_text("Report a bug — review a screenshot and open a GitHub draft").clicked() {
            crate::help::run(app, &ui.ctx().clone(), "report");
        }
        // Preferences, at the top right like Fusion's profile menu (also Ctrl+,).
        let gear = Rect::from_center_size(pos2(r.right() - captions - 70.0, r.center().y), vec2(24.0, 24.0));
        let gresp = ui.interact(gear, ui.id().with("sc_prefs_button"), Sense::click());
        if gresp.hovered() {
            ui.painter().rect_filled(gear, 4.0, Color32::from_white_alpha(30));
        }
        icons::paint(ui.painter(), gear.shrink(4.0), "settings", t.app_bar_text, t.app_bar_text, t.app_bar);
        crate::scenario::publish_handle("titlebar:preferences", gear.center());
        let prefs_tip = if cfg!(target_os = "macos") {
            format!("{} (Cmd+,)", app.tr("ui.preferences"))
        } else {
            format!("{} (Ctrl+,)", app.tr("ui.preferences"))
        };
        if gresp.on_hover_text(prefs_tip).clicked() {
            crate::prefs::open_at(app, 0);
        }
        ui.painter().text(
            pos2(r.right() - 12.0 - captions, r.center().y),
            Align2::RIGHT_CENTER,
            format!("v{}", env!("CARGO_PKG_VERSION")),
            FontId::proportional(11.0),
            Color32::from_rgb(160, 168, 182),
        );
        if app.custom_titlebar {
            crate::titlebar::caption_buttons(app, ui, r);
        }
    });
    file_menu(app, ui.ctx());
    let help_x = ui.ctx().data(|d| d.get_temp::<f32>(egui::Id::new("sc_help_anchor")).unwrap_or(300.0));
    crate::help::show(app, &ui.ctx().clone(), pos2(help_x - 14.0, 34.0));
}

fn file_menu(app: &mut SolveApp, ctx: &egui::Context) {
    let id = egui::Id::new("sc_file_menu");
    if !ctx.data(|d| d.get_temp::<bool>(id).unwrap_or(false)) {
        return;
    }
    let mut close = false;
    let resp = crate::frame::area(egui::Id::new("sc_file_area"))
        .fixed_pos(pos2(100.0 + if app.integrated_titlebar { crate::titlebar::TRAFFIC_LIGHTS } else { 0.0 }, 34.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(190.0);
                ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                ui.visuals_mut().widgets.inactive.bg_stroke = Stroke::NONE;
                ui.visuals_mut().widgets.hovered.weak_bg_fill = Tokens::get().hover;
                let item = |ui: &mut egui::Ui, label: &str, key: &str| {
                    ui.add(egui::Button::new(label).shortcut_text(key).min_size(vec2(180.0, 22.0))).clicked()
                };
                if item(ui, app.tr("ui.new_design"), "Ctrl+N") {
                    crate::documents::new_design(app);
                    close = true;
                }
                if item(ui, &format!("{}…", app.tr("ui.open")), "Ctrl+O") {
                    if let Some(p) = app.services.pick_open.as_ref().and_then(|f| f()) {
                        app.open_path_async(&p);
                    }
                    close = true;
                }
                ui.menu_button(app.tr("ui.open_recent"), |ui| {
                    for path in app.home.recent.clone() {
                        let missing = !solvecraft_engine::io::vfs::exists(&path);
                        let label = format!("{path}{}", if missing { " (missing)" } else { "" });
                        if ui.button(label).clicked() {
                            crate::home::open_recent(app, &path);
                            close = true;
                            ui.close();
                        }
                    }
                    ui.separator();
                    if ui.button(app.tr("ui.clear_recent")).clicked() {
                        app.home.recent.clear();
                        app.home.opened_at.clear();
                        close = true;
                        ui.close();
                    }
                });
                if item(ui, "Insert STEP…", "") {
                    app.start("file.insert_step");
                    close = true;
                }
                if item(ui, "Insert Mesh…", "") {
                    app.start("file.insert_mesh");
                    close = true;
                }
                if item(ui, app.tr("ui.save"), "Ctrl+S") {
                    save(app);
                    close = true;
                }
                if item(ui, &format!("{}…", app.tr("ui.save_as")), "") {
                    save_as(app);
                    close = true;
                }
                ui.separator();
                if item(ui, "Export STL…", "") {
                    export(app, "stl");
                    close = true;
                }
                if item(ui, "Export STEP…", "") {
                    export(app, "step");
                    close = true;
                }
                if item(ui, "Export 3MF…", "") {
                    export(app, "3mf");
                    close = true;
                }
                if item(ui, "Export OBJ…", "") {
                    export(app, "obj");
                    close = true;
                }
                ui.separator();
                if item(ui, &format!("{}…", app.tr("ui.preferences")), "") {
                    app.prefs_window.open = true;
                    close = true;
                }
                if item(ui, &format!("{}…", app.tr("ui.keyboard_shortcuts")), "") {
                    app.keymap.open = true;
                    app.keymap.read_only = false;
                    close = true;
                }
                if item(ui, app.tr("ui.quit"), "") {
                    crate::documents::request_quit(app);
                    close = true;
                }
            });
        });
    let just_opened = ctx.data(|d| d.get_temp::<u64>(egui::Id::new("sc_file_menu_opened"))) == Some(ctx.cumulative_pass_nr());
    if close || (!just_opened && resp.response.clicked_elsewhere()) {
        ctx.data_mut(|d| d.insert_temp(id, false));
    }
}

pub fn save(app: &mut SolveApp) {
    if crate::browser::needs_capture(app, crate::browser::SAVE) {
        app.tree.capture_prompt = Some(crate::browser::SAVE.into());
        return;
    }
    if app.session.path.is_some() {
        let _ = app.run("file.save", json!({}));
    } else {
        save_as(app);
    }
}

fn save_as(app: &mut SolveApp) {
    let name = format!("{}.{}", app.session.doc.name, solvecraft_engine::io::DESIGN_EXT);
    if let Some(p) = app.services.pick_save.as_ref().and_then(|f| f(&name, &[solvecraft_engine::io::DESIGN_EXT])) {
        let _ = app.run("file.save_as", json!({"path": p}));
    }
}

fn export(app: &mut SolveApp, ext: &str) {
    let name = format!("{}.{ext}", app.session.doc.name);
    if let Some(p) = app.services.pick_save.as_ref().and_then(|f| f(&name, &[ext]))
        && app.run("file.export", json!({"path": p})).is_ok()
    {
        app.set_status(format!("Exported {p}"), false);
    }
}

pub fn toolbar(app: &mut SolveApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    // Short windows (including enlarged displays) retain room for navigation and forms.
    let scale = if ui.ctx().content_rect().height() < 500.0 { 0.70 } else { 1.0 };
    let height = if scale < 1.0 { 92.0 } else { 120.0 };
    egui::Panel::top("sc_toolbar").exact_size(height).frame(egui::Frame::NONE.fill(t.toolbar)).show(ui, |ui| {
        let r = ui.max_rect();
        let painter = ui.painter().clone();
        // File and environment tabs span the ribbon, leaving the whole tool row available.
        let tabs_y = r.top() + 2.0;
        let ws = Rect::from_min_size(pos2(r.left() + 4.0, tabs_y), vec2(48.0, 22.0));
        let wresp = ui.interact(ws, ui.id().with("workspace_switcher"), Sense::click());
        painter.rect_filled(ws, 2.0, t.accent);
        painter.text(ws.center(), Align2::CENTER_CENTER, app.tr("ui.file"), FontId::proportional(12.0), t.app_bar_text);
        if wresp.clicked() {
            let pass = ui.ctx().cumulative_pass_nr();
            ui.ctx().data_mut(|d| {
                let id = egui::Id::new("sc_file_menu");
                d.insert_temp(id, !d.get_temp::<bool>(id).unwrap_or(false));
                d.insert_temp(egui::Id::new("sc_file_menu_opened"), pass);
            });
        }
        crate::scenario::publish_rect("toolbar", r);
        crate::scenario::publish_rect("workspace_switcher", ws);
        let mut x = r.left() + 58.0;
        for tab in TABS.iter().copied() {
            let display_tab = app.tr_label(tab_label(tab));
            let text_width = painter.layout_no_wrap(display_tab.to_string(), FontId::proportional(12.0), t.text).size().x;
            let w = 12.0 + text_width.max(28.0);
            let tr = Rect::from_min_size(pos2(x, tabs_y), vec2(w, 22.0));
            if tr.right() > r.right() {
                break;
            }
            let active = app.ui.tab == tab;
            let resp = ui.interact(tr, ui.id().with(("tab", tab)), Sense::click());
            crate::scenario::publish_handle(&format!("ribbon:tab:{tab}"), tr.center());
            if active {
                let c = if tab == "SKETCH" { t.sketch_accent } else { t.accent };
                painter.line_segment([pos2(tr.left() + 4.0, tr.bottom() - 1.0), pos2(tr.right() - 4.0, tr.bottom() - 1.0)], Stroke::new(2.0, c));
            } else if resp.hovered() {
                painter.rect_filled(tr, 4.0, t.hover);
            }
            let col = if tab == "SKETCH" {
                t.sketch_accent
            } else if active {
                t.text
            } else {
                t.text_dim
            };
            painter.text(tr.center(), Align2::CENTER_CENTER, display_tab, FontId::proportional(12.0), col);
            if resp.clicked() {
                app.ui.tab = tab.to_string();
            }
            x += w + 4.0;
        }
        // Panels.
        let tab = app.ui.tab.clone();
        // The command whose tool or dialog is open right now, shown pressed in the toolbar.
        let running: Option<String> = match (&app.tool, &app.dialog) {
            (Some(tool), _) => Some(tool.cmd.to_string()),
            // Create Sketch never lands in last_command (it is not repeatable).
            (None, Some(d)) if matches!(d.kind, crate::dialogs::Kind::Sketch) => Some("sketch.create".to_string()),
            (None, Some(_)) => app.last_command.as_ref().map(|(id, _)| id.clone()),
            _ => None,
        };
        let mut px = r.left() + 4.0;
        let top = r.top() + 26.0;
        let mut drop: Option<(&'static str, &'static str, Option<&'static str>)> = None;
        let mut menu: Option<(egui::Pos2, &'static str)> = None;
        let mut overflow = Vec::new();
        for (panel, cmds, promote) in crate::toolbar_custom::panel_lists(&tab) {
            let panel: &&str = &panel;
            let shown = crate::toolbar_custom::buttons(app, &tab, panel, &cmds, promote);
            let large = large_buttons(&tab, panel).min(shown.len());
            let small = shown.len().saturating_sub(large);
            let large_width = shown
                .iter()
                .take(large)
                .filter(|c| !matches!(c.id, "sketch.create" | "sketch.finish"))
                .map(|c| {
                    painter.layout_no_wrap(app.tr_label(command_label(c.id, c.label)).to_string(), FontId::proportional(11.0), t.text).size().x + 8.0
                })
                .fold(62.0_f32, f32::max)
                .min(96.0);
            let label_width =
                painter.layout_no_wrap(app.tr_label(panel_label(panel)).to_string(), FontId::proportional(10.5), t.text).size().x + 22.0;
            let width = (large as f32 * large_width + small.div_ceil(3) as f32 * 104.0 + 8.0).max(label_width).max(56.0);
            let finish = *panel == "FINISH SKETCH";
            let reserved = if tab == "SKETCH" { 116.0 } else { 40.0 };
            if finish {
                px = r.right() - width - 4.0;
            } else if !overflow.is_empty() || px + width > r.right() - reserved {
                overflow.push((*panel, cmds));
                continue;
            }
            let enabled_panel = !cmds.is_empty() || *panel == "SELECT";
            // Dropped on the panel's free space: the button goes to its end.
            let free = Rect::from_min_size(pos2(px + width - 5.0, top + 2.0), vec2(5.0, 70.0 * scale));
            let fresp = ui.interact(free, ui.id().with(("panel_free", panel)), Sense::hover());
            if fresp.dnd_hover_payload::<ToolbarDrag>().is_some() {
                painter.line_segment([pos2(free.left(), free.top() + 2.0), pos2(free.left(), free.bottom() - 2.0)], Stroke::new(2.0, t.accent));
            }
            if let Some(d) = fresp.dnd_release_payload::<ToolbarDrag>() {
                drop = Some((d.0, *panel, None));
            }
            for (i, c) in shown.iter().enumerate() {
                let big = i < large;
                let j = i.saturating_sub(large);
                let br = if big {
                    Rect::from_min_size(pos2(px + 4.0 + i as f32 * large_width, top + 2.0), vec2(large_width - 2.0, 70.0 * scale))
                } else {
                    Rect::from_min_size(
                        pos2(px + 4.0 + large as f32 * large_width + (j / 3) as f32 * 104.0, top + 2.0 + (j % 3) as f32 * 24.0 * scale),
                        vec2(102.0, 22.0 * scale),
                    )
                };
                let info = c.info(&app.session);
                let resp = ui.interact(br, ui.id().with(("cmd", c.id)), Sense::click_and_drag());
                crate::scenario::publish_handle(&format!("toolbar:{}", c.id), br.center());
                if resp.drag_started() {
                    resp.dnd_set_drag_payload(ToolbarDrag(c.id));
                }
                if let Some(d) = resp.dnd_hover_payload::<ToolbarDrag>()
                    && d.0 != c.id
                {
                    painter
                        .line_segment([pos2(br.left() - 1.0, br.top() + 2.0), pos2(br.left() - 1.0, br.bottom() - 2.0)], Stroke::new(2.0, t.accent));
                }
                if let Some(d) = resp.dnd_release_payload::<ToolbarDrag>()
                    && d.0 != c.id
                {
                    drop = Some((d.0, *panel, Some(c.id)));
                }
                if resp.secondary_clicked() {
                    menu = Some((resp.interact_pointer_pos().unwrap_or(br.left_bottom()), c.id));
                }
                if running.as_deref() == Some(c.id) {
                    painter.rect_filled(br, 4.0, t.border.gamma_multiply(0.7));
                } else if resp.hovered() && info.enabled {
                    painter.rect_filled(br, 4.0, t.hover);
                }
                let (ink, fill, acc) = if info.enabled {
                    (t.icon, t.icon_fill, if tab == "SKETCH" { t.sketch_accent } else { t.accent })
                } else {
                    (t.border, t.panel_header, t.border)
                };
                let icon = match c.id {
                    "sketch.finish" => "finish",
                    "config.table" => "configuration_table",
                    _ => c.icon,
                };
                let ir = if big {
                    Rect::from_center_size(pos2(br.center().x, br.top() + 21.0 * scale), vec2(30.0, 30.0) * scale)
                } else {
                    Rect::from_center_size(pos2(br.left() + 11.0, br.center().y), vec2(18.0, 18.0) * scale)
                };
                icons::paint(&painter, ir, icon, ink, fill, acc);
                let label = command_label(c.id, c.label);
                let tr_label = app.tr_label(label);
                let text_rect = if big {
                    Rect::from_min_max(pos2(br.left() + 2.0, br.top() + 42.0 * scale), br.right_bottom())
                } else {
                    Rect::from_min_max(pos2(br.left() + 24.0, br.top()), br.right_bottom())
                };
                let mut job = egui::text::LayoutJob::simple(
                    tr_label.to_string(),
                    FontId::proportional(11.0 * scale.max(0.85)),
                    if info.enabled { t.text } else { t.text_dim },
                    text_rect.width(),
                );
                job.wrap.max_rows = if matches!(c.id, "sketch.create" | "sketch.finish") { 2 } else { 1 };
                job.halign = if big { egui::Align::Center } else { egui::Align::Min };
                let galley = painter.layout_job(job);
                painter.galley(
                    pos2(if big { text_rect.center().x } else { text_rect.left() }, text_rect.center().y - galley.size().y / 2.0),
                    galley,
                    t.text,
                );
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled() && info.enabled, tr_label));
                let tip = match &info.disabled_reason {
                    Some(why) => format!("{}\n{why}", tr_label),
                    None => format!("{}{}", tr_label, c.shortcut.map(|k| format!("  ({k})")).unwrap_or_default()),
                };
                if resp.on_hover_text(tip).clicked() && info.enabled {
                    let id = c.id;
                    app.start(id);
                }
            }
            if shown.is_empty() {
                let br = Rect::from_min_size(pos2(px + 4.0, top + 2.0), vec2(width - 8.0, 70.0 * scale));
                painter.text(br.center(), Align2::CENTER_CENTER, "—", FontId::proportional(14.0), t.border);
            }
            // Panel label with drop-down.
            let lr = Rect::from_min_size(pos2(px, top + 74.0 * scale), vec2(width, 18.0 * scale));
            let label = app.tr_label(panel_label(panel)).to_string();
            let lresp = ui.interact(lr, ui.id().with(("panel", panel)), Sense::click());
            crate::scenario::publish_handle(&format!("panel:{tab}:{panel}"), lr.center());
            if lresp.hovered() && enabled_panel {
                painter.rect_filled(lr, 3.0, t.hover);
            }
            let lc = if *panel == "FINISH SKETCH" {
                t.sketch_accent
            } else if enabled_panel {
                t.text
            } else {
                t.text_dim
            };
            let g = painter.layout_no_wrap(label, FontId::proportional(10.5), lc);
            let gx = lr.center().x - (g.size().x + 9.0) / 2.0;
            let gy = lr.center().y - g.size().y / 2.0;
            let gw = g.size().x;
            painter.galley(pos2(gx, gy), g, lc);
            caret(&painter, pos2(gx + gw + 6.0, lr.center().y), lc);
            let popup_id = ui.id().with(("panel_menu", panel));
            if lresp.clicked() {
                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("sc_panel_open"), popup_id));
            }
            let open = ui.ctx().data(|d| d.get_temp::<egui::Id>(egui::Id::new("sc_panel_open"))) == Some(popup_id);
            if open {
                let area = crate::frame::area(popup_id).fixed_pos(lr.left_bottom()).order(egui::Order::Foreground).show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.set_min_width(200.0);
                        if *panel == "SELECT" {
                            selection_filter(app, ui);
                        } else if cmds.is_empty() {
                            ui.label(egui::RichText::new("Not available yet").color(t.text_dim));
                        }
                        for c in &cmds {
                            let info = c.info(&app.session);
                            ui.horizontal(|ui| {
                                let (ir, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                                icons::paint(ui.painter(), ir, c.icon, t.icon, t.icon_fill, t.accent);
                                let b = ui.add_enabled(
                                    info.enabled,
                                    egui::Button::new(app.tr_label(c.label)).frame(false).shortcut_text(c.shortcut.unwrap_or("")),
                                );
                                crate::scenario::publish_handle(&format!("toolbar:{}", c.id), b.rect.center());
                                if b.secondary_clicked() {
                                    menu = Some((b.interact_pointer_pos().unwrap_or(b.rect.left_bottom()), c.id));
                                }
                                if b.clicked() {
                                    let id = c.id;
                                    app.start(id);
                                    ui.ctx().data_mut(|d| d.remove::<egui::Id>(egui::Id::new("sc_panel_open")));
                                }
                            });
                        }
                    });
                });
                if area.response.clicked_elsewhere() && !lresp.clicked() {
                    ui.ctx().data_mut(|d| d.remove::<egui::Id>(egui::Id::new("sc_panel_open")));
                }
            }
            px += width;
            painter.line_segment([pos2(px, top + 4.0), pos2(px, top + 90.0 * scale)], Stroke::new(1.0, t.border));
            px += 2.0;
        }
        if !overflow.is_empty() {
            let right = r.right() - if tab == "SKETCH" { 114.0 } else { 40.0 };
            let at = Rect::from_min_size(pos2(px.min(right), top + 4.0), vec2(38.0, 52.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(at), |ui| {
                let response = ui.menu_button("»", |ui| {
                    for (panel, cmds) in &overflow {
                        ui.menu_button(app.tr_label(panel_label(panel)), |ui| {
                            if *panel == "SELECT" {
                                selection_filter(app, ui);
                            }
                            for c in cmds {
                                let info = c.info(&app.session);
                                let response = ui.add_enabled(info.enabled, egui::Button::new(c.label).shortcut_text(c.shortcut.unwrap_or("")));
                                crate::scenario::publish_handle(&format!("toolbar:{}", c.id), response.rect.center());
                                if response.clicked() {
                                    app.start(c.id);
                                    ui.close();
                                }
                            }
                        });
                    }
                });
                crate::scenario::publish_handle("toolbar:overflow", response.response.rect.center());
                response.response.on_hover_text("More toolbar panels");
            });
        }
        painter.line_segment([pos2(r.left(), r.bottom() - 0.5), pos2(r.right(), r.bottom() - 0.5)], Stroke::new(1.0, t.border));
        if let Some((id, panel, before)) = drop {
            crate::toolbar_custom::reorder(app, &tab, id, panel, before);
        }
        if let Some((at, id)) = menu {
            ui.ctx().data_mut(|d| d.remove::<egui::Id>(egui::Id::new("sc_panel_open")));
            crate::context_menu::open_for(app, at, crate::context_menu::Target::ToolbarCommand { id: id.to_string() });
        }
    });
}

/// A toolbar button being dragged (its command id).
#[derive(Clone, Copy, Debug)]
struct ToolbarDrag(&'static str);

/// The SELECT panel: what a click in the view picks.
fn selection_filter(app: &mut SolveApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    ui.label(egui::RichText::new("Selection filter").color(t.text_dim).size(11.0));
    ui.checkbox(&mut app.ui.pick_bodies, "Select bodies (a face picks its body)");
    for (k, label) in [
        ("faces", "Faces"),
        ("edges", "Edges"),
        ("vertices", "Vertices"),
        ("sketch", "Sketch geometry and profiles"),
        ("sketch_curves", "Sketch curves"),
        ("sketch_points", "Sketch points"),
        ("dimensions", "Sketch dimensions"),
        ("constraints", "Sketch constraints"),
    ] {
        let mut on = !app.ui.pick_off.iter().any(|x| x == k);
        if ui.checkbox(&mut on, label).changed() {
            app.ui.pick_off.retain(|x| x != k);
            if !on {
                app.ui.pick_off.push(k.to_string());
            }
        }
    }
}

/// A small drop-down caret.
fn caret(p: &egui::Painter, c: egui::Pos2, col: Color32) {
    p.add(egui::Shape::convex_polygon(vec![pos2(c.x - 3.5, c.y - 2.0), pos2(c.x + 3.5, c.y - 2.0), pos2(c.x, c.y + 2.5)], col, Stroke::NONE));
}

/// Global keyboard shortcuts (when no text field has focus).
pub fn shortcuts(app: &mut SolveApp, ctx: &egui::Context) {
    if app.file_load.busy() {
        return;
    }
    // The Keyboard Shortcuts window is waiting for a key: it is not a shortcut.
    if crate::keymap::capturing(app) {
        return;
    }
    if ctx.egui_wants_keyboard_input() {
        return;
    }
    let (keys, mods) = ctx.input(|i| {
        let keys: Vec<egui::Key> =
            i.events.iter().filter_map(|e| if let egui::Event::Key { key, pressed: true, .. } = e { Some(*key) } else { None }).collect();
        (keys, i.modifiers)
    });
    for k in keys {
        use egui::Key;
        if mods.command {
            match k {
                Key::Z => drop(app.run("edit.undo", json!({}))),
                Key::Y => drop(app.run("edit.redo", json!({}))),
                Key::S => save(app),
                Key::N => crate::documents::new_design(app),
                Key::Comma => crate::prefs::open_at(app, 0),
                Key::O => {
                    if let Some(p) = app.services.pick_open.as_ref().and_then(|f| f()) {
                        app.open_path_async(&p);
                    }
                }
                _ => {
                    if let Some(id) = crate::keymap::command_for(app, &crate::keymap::key_name(k, mods)) {
                        app.start(id);
                    }
                }
            }
            continue;
        }
        match k {
            Key::Escape => {
                app.esc_handled = true;
                if app.tool.is_some() {
                    crate::tools::finish(app);
                } else if app.dialog.is_some() {
                    crate::dialogs::cancel(app);
                } else if !app.session.selection.is_empty() || app.session.active_sketch.is_none() {
                    let _ = app.run("select.clear", json!({}));
                } else {
                    // Nothing left to cancel: Esc finishes the sketch.
                    app.finish_sketch();
                }
            }
            Key::S if !mods.shift && !mods.alt => {
                let at = ctx.input(|i| i.pointer.latest_pos());
                crate::shortcut_box::open(app, at);
            }
            Key::V => crate::context_menu::toggle_visibility(app),
            Key::F2 => {
                let at = ctx.input(|i| i.pointer.latest_pos()).unwrap_or(ctx.content_rect().center());
                crate::context_menu::rename_selection(app, at);
            }
            Key::F6 => app.animate_view("fit"),
            Key::Delete | Key::Backspace => crate::viewport::delete_selection(app),
            _ => {
                if let Some(id) = crate::keymap::command_for(app, &crate::keymap::key_name(k, mods)) {
                    app.start(id);
                }
            }
        }
    }
}

#[cfg(test)]
mod ribbon_tests {
    use super::*;

    #[test]
    fn sketch_ribbon_enters_and_finishes_sketch_without_losing_the_document() {
        let mut h = crate::scenario::Harness::new();
        h.step(&json!({"click":{"handle":"ribbon:tab:SKETCH"}})).unwrap();
        h.step(&json!({"click":{"handle":"toolbar:sketch.create"}})).unwrap();
        assert!(matches!(h.app.dialog.as_ref().map(|d| &d.kind), Some(crate::dialogs::Kind::Sketch)));
        h.step(&json!({"key":"Escape"})).unwrap();
        h.app.run("sketch.create", json!({"plane":"XY"})).unwrap();
        h.app.run("sketch.line", json!({"points":[[0,0],[10,0]]})).unwrap();
        h.frames(4);
        assert_eq!(h.app.ui.tab, "SKETCH");
        h.step(&json!({"click":{"handle":"toolbar:sketch.finish"}})).unwrap();
        assert!(h.app.session.active_sketch.is_none());
        assert_eq!(h.app.ui.tab, "SOLID");
        assert_eq!(h.app.session.doc.features.len(), 1);
    }

    #[test]
    fn narrow_sketch_ribbon_keeps_finish_visible_and_overflow_separate() {
        let mut h = crate::scenario::Harness::new();
        h.app.run("sketch.create", json!({"plane":"XY"})).unwrap();
        for width in [480.0, 640.0, 900.0] {
            h.call("ui.resize", json!({"width":width,"height":700}));
            let finish = crate::scenario::handle_at("toolbar:sketch.finish").unwrap();
            let overflow = crate::scenario::handle_at("toolbar:overflow").unwrap();
            assert!(finish.x < width && finish.x > width - 90.0);
            assert!(overflow.x < finish.x - 40.0);
            assert!(h.app.session.active_sketch.is_some());
        }
    }
}

#[cfg(test)]
mod header_report_tests {
    use super::*;

    fn captured_frame(h: &mut crate::scenario::Harness) {
        let token = h.app.shot_token;
        crate::issue_report::receive(&mut h.app, token, std::sync::Arc::new(egui::ColorImage::new([2, 2], vec![Color32::GREEN; 4])));
        h.frames(3);
    }

    #[test]
    fn header_report_reserves_controls_at_normal_and_double_scale_widths() {
        for physical_width in [900.0, 1400.0] {
            for scale in [1.0, 2.0] {
                let width = physical_width / scale;
                let bar = Rect::from_min_size(egui::Pos2::ZERO, vec2(width, 34.0));
                for captions in [0.0, crate::titlebar::WIDTH] {
                    let (report, labeled) = report_rect(bar, captions, 354.0);
                    assert!(bar.contains_rect(report));
                    let prefs = Rect::from_center_size(pos2(width - captions - 70.0, 17.0), vec2(24.0, 24.0));
                    assert!(!report.intersects(prefs));
                    assert!(report.right() < width - captions - 82.0);
                    assert_eq!(labeled, width - captions - 90.0 - 354.0 >= 340.0);
                }
            }
        }
    }

    #[test]
    fn header_report_actual_click_opens_capture_without_changing_healthy_design() {
        for width in [450.0, 700.0, 900.0, 1400.0] {
            let mut h = crate::scenario::Harness::new();
            h.app.home.open = false;
            h.app.custom_titlebar = true;
            h.app.run("solid.box", json!({"length":35,"width":12,"height":4})).unwrap();
            h.call("ui.resize", json!({"width":width,"height":720}));
            h.frames(3);
            let before = (h.app.session.doc.to_json(), h.app.session.undo.len(), h.app.session.revision);
            let point = h.widget("Report a bug").expect("compact and full buttons retain an accessible name");
            h.step(&json!({"click":point})).unwrap();
            assert!(h.app.issue_report.draft.is_some(), "width {width}: header click must reach the report");
            assert!(crate::issue_report::pending(&h.app));
            assert_eq!(before, (h.app.session.doc.to_json(), h.app.session.undo.len(), h.app.session.revision));
            captured_frame(&mut h);
            // Existing review content scrolls vertically when the window is narrow.
            if width < 700.0 {
                let [x, y] = h.widget("Close draft").unwrap();
                h.call("ui.scroll", json!({"x":x,"y":y - 60.0,"delta":-1200}));
                // Let the scroll animation finish before locating the click target.
                h.frames(60);
            }
            h.step(&json!({"widget":"Close draft"})).unwrap();
            assert!(h.app.issue_report.draft.is_none(), "width {width}: review closes through its actual button");
            h.step(&json!({"click":{"handle":"appbar:home"}})).unwrap();
            assert!(h.app.home.open, "width {width}: Home remains reachable beside Report");
            assert_eq!(before, (h.app.session.doc.to_json(), h.app.session.undo.len(), h.app.session.revision));
        }
    }

    #[test]
    fn header_report_preserves_new_dimension_draft_and_single_commit() {
        crate::dim_view::cancel();
        let mut h = crate::scenario::Harness::new();
        h.app.home.open = false;
        h.app.run("sketch.create", json!({"plane":"XY"})).unwrap();
        h.app.run("sketch.line", json!({"points":[[0,0],[20,0]]})).unwrap();
        h.call("ui.view", json!({"view":"top"}));
        h.app.cam.distance = 80.0;
        h.app.cam.target = solvecraft_engine::geom::Vec3::new(10.0, 0.0, 0.0);
        h.settle();
        h.step(&json!({"start":"sketch.dimension"})).unwrap();
        for point in [[0, 0, 0], [20, 0, 0], [10, 5, 0]] {
            h.step(&json!({"click":{"world":point}})).unwrap();
        }
        h.step(&json!({"text":"30 mm"})).unwrap();
        let before = (h.app.session.doc.to_json(), h.app.session.undo.len(), h.app.session.revision);
        h.step(&json!({"widget":"Report a bug"})).unwrap();
        h.frames(3);
        assert!(h.app.issue_report.draft.is_some() && crate::dim_view::editing());
        captured_frame(&mut h);
        h.ctx.memory_mut(|m| m.request_focus(egui::Id::new("issue_report_body")));
        h.step(&json!({"text":"Describe this dimension problem."})).unwrap();
        h.step(&json!({"key":"Enter"})).unwrap();
        assert_eq!(before, (h.app.session.doc.to_json(), h.app.session.undo.len(), h.app.session.revision));
        h.step(&json!({"widget":"Close draft"})).unwrap();
        h.frames(2);
        assert_eq!(h.ctx.memory(|m| m.focused()), Some(egui::Id::new("sc_dim_edit")));
        h.step(&json!({"key":"Enter"})).unwrap();
        assert!(!crate::dim_view::editing());
        assert_eq!(h.app.session.undo.len(), before.1 + 1);
        assert_eq!(h.app.session.doc.param("d1").unwrap().expr, "30 mm");
        h.app.run("edit.undo", json!({})).unwrap();
        assert_eq!(h.app.session.doc.to_json(), before.0);
    }
}
