//! Workflow defaults supplied by the host, independent of any design file.

use serde::{Deserialize, Serialize};

/// Initial display and snapping options for new sketches. Slice and 3D sketch remain
/// individual sketch options; changing these defaults never edits an existing sketch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SketchDefaults {
    pub grid: bool,
    pub snap: bool,
    pub show_profile: bool,
    pub show_points: bool,
    pub show_dimensions: bool,
    pub show_constraints: bool,
    pub show_projected: bool,
}

impl Default for SketchDefaults {
    fn default() -> Self {
        Self { grid: true, snap: true, show_profile: true, show_points: true, show_dimensions: true, show_constraints: true, show_projected: true }
    }
}

impl SketchDefaults {
    pub(crate) fn view(&self) -> solidcraft_sketch::SketchView {
        solidcraft_sketch::SketchView {
            hide_grid: !self.grid,
            no_snap: !self.snap,
            hide_profile: !self.show_profile,
            hide_points: !self.show_points,
            hide_dimensions: !self.show_dimensions,
            hide_constraints: !self.show_constraints,
            hide_projected: !self.show_projected,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Session;

    #[test]
    fn defaults_initialize_new_sketches_without_editing_existing_ones() {
        let mut s = Session::default();
        let first = s.execute("sketch.create", &json!({})).unwrap()["sketch"].as_u64().unwrap();
        s.execute("sketch.finish", &json!({})).unwrap();
        let before = s.doc.clone();
        let undo = s.undo.len();
        s.execute("sketch.defaults", &json!({"grid":false,"snap":false,"show_dimensions":false})).unwrap();
        assert_eq!(s.doc, before);
        assert_eq!(s.undo.len(), undo);
        assert!(!s.doc.sketch(first).unwrap().view.hide_dimensions);
        let second = s.execute("sketch.create", &json!({})).unwrap()["sketch"].as_u64().unwrap();
        let view = &s.doc.sketch(second).unwrap().view;
        assert!(view.hide_grid && view.no_snap && view.hide_dimensions);
        assert!(!view.slice && !view.three_d);
        s.execute("sketch.options", &json!({"show_dimensions":true,"slice":true})).unwrap();
        assert!(!s.doc.sketch(second).unwrap().view.hide_dimensions);
        assert!(!s.sketch_defaults.show_dimensions);
        s.execute("edit.undo", &json!({})).unwrap();
        s.execute("edit.undo", &json!({})).unwrap();
        assert!(s.doc.feature(second).is_none(), "creating a sketch with defaults is one undo step");
        assert_eq!(s.scratch().sketch_defaults, s.sketch_defaults);
    }

    #[test]
    fn defaults_are_validated_atomically_and_survive_new_documents() {
        let mut s = Session::default();
        s.execute("sketch.auto_project", &json!({"value":false})).unwrap();
        s.execute("sketch.defaults", &json!({"snap":false})).unwrap();
        let defaults = s.sketch_defaults.clone();
        for bad in [json!({"grid":false,"slice":true}), json!({"grid":null}), json!({"snap":"false"}), json!([])] {
            assert!(s.execute("sketch.defaults", &bad).is_err());
            assert_eq!(s.sketch_defaults, defaults);
        }
        s.execute("file.new", &json!({})).unwrap();
        assert!(!s.auto_project);
        assert_eq!(s.sketch_defaults, defaults);
        assert_eq!(s.execute("sketch.defaults", &json!({})).unwrap()["snap"], false);
        assert_eq!(s.execute("sketch.defaults", &serde_json::Value::Null).unwrap()["snap"], false);
        assert!(!s.is_dirty());
        let serialized = serde_json::to_value(&*s.doc).unwrap();
        assert!(serialized.get("sketch_defaults").is_none());
    }
}
