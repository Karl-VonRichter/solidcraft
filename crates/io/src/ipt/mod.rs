//! Native, bounded IPT solid import from the owner-supplied clean-room specification.
//! See docs/ipt-import.md. No Autodesk runtime or research implementation is used.
mod asm;
mod container;
mod step;
#[cfg(test)]
mod tests;

use crate::{IoError, Result, StepFeature};

/// Upper bound for an IPT input and for any expanded model segment.
pub const MAX_IPT_BYTES: usize = 64 << 20;
const MAX_RECORDS: usize = 200_000;
const MAX_TOKENS: usize = 2_000_000;

pub fn is_ipt_path(path: &str) -> bool {
    std::path::Path::new(path).extension().and_then(|s| s.to_str()).is_some_and(|s| s.eq_ignore_ascii_case("ipt"))
}

fn invalid(message: impl std::fmt::Display) -> IoError {
    IoError::Invalid(format!("IPT: {message}"))
}

/// Import a complete supported part as a base feature. Unsupported or damaged geometry
/// fails the entire import; STEP's otherwise useful partial-import fallback is not used.
pub fn import_feature(bytes: &[u8], file: &str) -> Result<Option<StepFeature>> {
    let data = container::read(bytes)?;
    let model = asm::parse(&data.brep)?;
    let converted = step::convert(&model)?;
    if converted.bodies == 0 {
        return Ok(None);
    }
    let imported = solidcraft_kernel::step_import_shared(&converted.text)?;
    if imported.bodies.len() != converted.bodies || imported.bodies.iter().any(|b| !b.closed) || !imported.warnings.is_empty() {
        return Err(invalid(format!("geometry conversion incomplete: {}", imported.warnings.join("; "))));
    }
    let mut warnings = converted.warnings;
    warnings.push("IPT imported as solid bodies; original feature history, constraints and expressions are not preserved.".into());
    if !matches!(model.version, 22900 | 23100 | 23200) {
        warnings.push(format!("IPT ASM version {} is unverified", model.version));
    }
    let name = std::path::Path::new(file).file_stem().and_then(|s| s.to_str()).filter(|s| !s.is_empty()).unwrap_or("IPT import").to_string();
    let file = std::path::Path::new(file).file_name().and_then(|s| s.to_str()).unwrap_or("part.ipt").to_string();
    Ok(Some(StepFeature {
        name,
        body_names: imported.bodies.iter().map(|b| b.name.clone()).collect(),
        kind: solidcraft_doc::FeatureKind::Import { file, step: converted.text, components: imported.tree.clone() },
        warnings,
    }))
}

/// Checked cursor shared by the binary layers. No offsets or counts from a file index slices.
struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}
impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or_else(|| invalid("length overflow"))?;
        let out = self.bytes.get(self.pos..end).ok_or_else(|| invalid("truncated binary data"))?;
        self.pos = end;
        Ok(out)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(*self.take(1)?.first().ok_or_else(|| invalid("missing byte"))?)
    }
    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().map_err(|_| invalid("missing integer"))?))
    }
    fn f64(&mut self) -> Result<f64> {
        let n = f64::from_le_bytes(self.take(8)?.try_into().map_err(|_| invalid("missing number"))?);
        if !n.is_finite() || n.abs() > 1e100 {
            return Err(invalid("non-finite or excessive number"));
        }
        Ok(n)
    }
}
