//! Validate the separately supplied IPT handoff; no proprietary fixtures are committed.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{path::Path, process::ExitCode};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args().nth(1).ok_or("usage: ipt_validate <ipt-spec-handoff> [name-filter] [--conservative-bounds]")?;
    let filter = std::env::args().nth(2).unwrap_or_default();
    let conservative_bounds = std::env::args().any(|s| s == "--conservative-bounds");
    let root = Path::new(&root);
    let manifest: Value = serde_json::from_slice(&std::fs::read(root.join("manifest.json"))?)?;
    let rows = manifest["rows"].as_array().ok_or("manifest rows missing")?;
    let mut failed = 0;
    for row in rows {
        let file = row["file"].as_str().ok_or("manifest file missing")?;
        if filter != "--conservative-bounds" && !file.contains(&filter) {
            continue;
        }
        let path = root.join(file);
        let bytes = std::fs::read(&path)?;
        if format!("{:x}", Sha256::digest(&bytes)) != row["sha256"].as_str().ok_or("hash missing")? {
            return Err("fixture hash mismatch".into());
        }
        let result = validate(&bytes, file, row, conservative_bounds);
        match result {
            Ok(result) => println!("{}", json!({"file":file,"result":result})),
            Err(e) => {
                failed += 1;
                println!("{}", json!({"file":file,"error":e.to_string()}));
            }
        }
    }
    if failed > 0 {
        return Err(format!("{failed} fixture(s) failed").into());
    }
    Ok(())
}
fn validate(bytes: &[u8], file: &str, expected: &Value, conservative_bounds: bool) -> Result<Value, Box<dyn std::error::Error>> {
    let feature = solidcraft_io::ipt::import_feature(bytes, file)?;
    let mut doc = solidcraft_doc::Document::new("IPT validation");
    let mut warnings = Vec::new();
    if let Some(f) = feature {
        warnings = f.warnings;
        doc.add_feature(f.kind, None)?;
    }
    let mut model = solidcraft_doc::Model::new();
    model.evaluate(&doc);
    for r in &model.results {
        if let Some(error) = &r.error {
            return Err(error.clone().into());
        }
    }
    let state = model.state();
    let mut volume = 0.0;
    let mut bounds = solidcraft_geom::Aabb3::EMPTY;
    for body in &state.bodies {
        let measured = solidcraft_kernel::measure(&body.body)?;
        volume += measured.volume;
        bounds = bounds.union(&measured.bbox);
        if body.body.display_mesh(0.05)?.triangles.is_empty() {
            return Err("solid has no display triangles".into());
        }
    }
    let want = expected["volume_mm3"].as_f64().ok_or("expected volume missing")?;
    let count = expected["bodies"].as_array().ok_or("body counts missing")?.len();
    if state.bodies.len() != count {
        return Err(format!("expected {count} bodies, got {}", state.bodies.len()).into());
    }
    if (volume - want).abs() > want.abs().max(1.0) * 0.001 {
        return Err(format!("volume mismatch: {volume}, expected {want}").into());
    }
    let mut bounds_differences = Vec::new();
    if count > 0 {
        for (i, point) in [bounds.min, bounds.max].iter().enumerate() {
            for (j, actual) in [point.x, point.y, point.z].iter().enumerate() {
                let expected = expected["range_mm"][i][j].as_f64().ok_or("missing expected bounds")?;
                if (actual - expected).abs() > 0.01 {
                    let inside = if i == 0 { *actual >= expected } else { *actual <= expected };
                    if !conservative_bounds || !inside {
                        return Err(format!("bounds mismatch at [{i}][{j}]: {actual}, expected {expected}").into());
                    }
                    bounds_differences.push(json!({"bound":i,"axis":j,"actual_mm":actual,"reference_mm":expected}));
                }
            }
        }
    }
    let saved = solidcraft_io::write_design_container(&doc)?;
    if solidcraft_io::read_design(&saved)? != doc {
        return Err("save/reopen changed the document".into());
    }
    Ok(
        json!({"bodies":count,"volume_mm3":volume,"relative_error":(volume-want).abs()/want.abs().max(1.0),"bounds_differences":bounds_differences,"warnings":warnings}),
    )
}
