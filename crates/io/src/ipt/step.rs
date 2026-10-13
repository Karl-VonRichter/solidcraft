//! Restate validated IPT topology as STEP, in memory, for SolidCraft's existing kernel boundary.
use super::{
    MAX_IPT_BYTES,
    asm::{Model, Record, Token},
    invalid,
};
use crate::Result;
use solidcraft_geom::Vec3;
use std::collections::{HashMap, HashSet};

pub(super) struct Converted {
    pub text: String,
    pub bodies: usize,
    pub warnings: Vec<String>,
}
struct Writer<'a> {
    model: &'a Model,
    text: String,
    next: usize,
    vertices: HashMap<usize, usize>,
    edges: HashMap<usize, usize>,
    surfaces: HashMap<usize, usize>,
    warnings: Vec<String>,
}
fn logical(v: bool) -> &'static str {
    if v { ".T." } else { ".F." }
}
fn refs(ids: &[usize]) -> String {
    ids.iter().map(|id| format!("#{id}")).collect::<Vec<_>>().join(",")
}
fn real(n: f64) -> String {
    format!("{n:.16e}")
}
fn numbers(ns: &[f64]) -> String {
    ns.iter().map(|n| real(*n)).collect::<Vec<_>>().join(",")
}
fn integers(ns: &[usize]) -> String {
    ns.iter().map(usize::to_string).collect::<Vec<_>>().join(",")
}
fn unit(v: Vec3) -> Result<Vec3> {
    let n = v.len();
    if !n.is_finite() || n < 1e-12 {
        return Err(invalid("zero or invalid geometry direction"));
    }
    Ok(v / n)
}
fn null(r: &Record, n: usize, reason: &str) -> Result<()> {
    if r.ptr(n)?.is_some() {
        return Err(invalid(format!("unsupported geometry: {reason}")));
    }
    Ok(())
}

impl Writer<'_> {
    fn add(&mut self, s: String) -> Result<usize> {
        if self.text.len().saturating_add(s.len()) > MAX_IPT_BYTES || self.next > 1_000_000 {
            return Err(invalid("converted geometry exceeds limit"));
        }
        let id = self.next;
        self.next += 1;
        self.text.push_str(&format!("#{id}={s};\n"));
        Ok(id)
    }
    fn point(&mut self, p: Vec3) -> Result<usize> {
        let p = p * self.model.scale;
        if ![p.x, p.y, p.z].iter().all(|n| n.is_finite() && n.abs() <= 1e9) {
            return Err(invalid("coordinate outside supported range"));
        }
        self.add(format!("CARTESIAN_POINT('',({}))", numbers(&[p.x, p.y, p.z])))
    }
    fn dir(&mut self, p: Vec3) -> Result<usize> {
        let p = unit(p)?;
        self.add(format!("DIRECTION('',({}))", numbers(&[p.x, p.y, p.z])))
    }
    fn axis(&mut self, origin: Vec3, z: Vec3, x: Vec3) -> Result<usize> {
        let z = unit(z)?;
        let x = unit(x)?;
        if z.dot(x).abs() > 1e-6 {
            return Err(invalid("non-perpendicular surface axes"));
        }
        let p = self.point(origin)?;
        let z = self.dir(z)?;
        let x = self.dir(x)?;
        self.add(format!("AXIS2_PLACEMENT_3D('',#{p},#{z},#{x})"))
    }
    fn vertex(&mut self, id: usize) -> Result<usize> {
        if let Some(n) = self.vertices.get(&id) {
            return Ok(*n);
        }
        let r = self.model.record(id, "vertex")?;
        let p = self.model.record(r.link(5)?, "point")?.position(3)?;
        let p = self.point(p)?;
        let n = self.add(format!("VERTEX_POINT('',#{p})"))?;
        self.vertices.insert(id, n);
        Ok(n)
    }
    fn ellipse(&mut self, center: Vec3, normal: Vec3, major: Vec3, ratio: f64) -> Result<usize> {
        let radius = major.len() * self.model.scale;
        if !((1e-9..=1e9).contains(&radius) && ratio > 0.0 && ratio <= 1.0) {
            return Err(invalid("invalid ellipse radii"));
        }
        let axis = self.axis(center, normal, major)?;
        if (ratio - 1.0).abs() < 1e-12 {
            self.add(format!("CIRCLE('',#{axis},{})", real(radius)))
        } else {
            self.add(format!("ELLIPSE('',#{axis},{},{})", real(radius), real(radius * ratio)))
        }
    }
    fn curve(&mut self, id: usize) -> Result<usize> {
        let r = self.model.records.get(id).ok_or_else(|| invalid("dangling curve"))?;
        match r.kind.as_str() {
            "straight-curve" => {
                let p = self.point(r.position(3)?)?;
                let d = self.dir(r.vector(4)?)?;
                let v = self.add(format!("VECTOR('',#{d},1.)"))?;
                self.add(format!("LINE('',#{p},#{v})"))
            }
            "ellipse-curve" => self.ellipse(r.position(3)?, r.vector(4)?, r.vector(5)?, r.number(6)?),
            "intcurve-curve" => {
                let (u, _, points, tolerance) = spline(self.model, r.field(4)?, false)?;
                self.fit_warning(tolerance)?;
                let points = points.into_iter().map(|p| self.point(p)).collect::<Result<Vec<_>>>()?;
                self.add(format!(
                    "B_SPLINE_CURVE_WITH_KNOTS('',{},({}),.UNSPECIFIED.,.F.,.F.,({}),({}),.UNSPECIFIED.)",
                    u.degree,
                    refs(&points),
                    integers(&u.mults),
                    numbers(&u.knots)
                ))
            }
            other => Err(invalid(format!("unsupported geometry: curve {other} at record {id}"))),
        }
    }
    fn fit_warning(&mut self, tolerance: f64) -> Result<()> {
        if tolerance < 0.0 || tolerance * self.model.scale > 0.01 {
            return Err(invalid("spline fit tolerance exceeds 0.01 mm"));
        }
        if tolerance > 0.0 {
            let warning = format!("IPT spline approximation: recorded fit tolerance {} mm", real(tolerance * self.model.scale));
            if !self.warnings.contains(&warning) {
                self.warnings.push(warning);
            }
        }
        Ok(())
    }
    fn edge(&mut self, id: usize) -> Result<usize> {
        if let Some(n) = self.edges.get(&id) {
            return Ok(*n);
        }
        let r = self.model.record(id, "edge")?;
        self.validate_edge_endpoints(r)?;
        let start = self.vertex(r.link(3)?)?;
        let end = self.vertex(r.link(5)?)?;
        let curve_id = r.link(8)?;
        let curve = self.curve(curve_id)?;
        let c = self.model.records.get(curve_id).ok_or_else(|| invalid("missing edge curve"))?;
        let reversed_curve = c.kind == "intcurve-curve" && c.boolean(3)?;
        let sense = !r.boolean(9)? ^ reversed_curve;
        let n = self.add(format!("EDGE_CURVE('',#{start},#{end},#{curve},{})", logical(sense)))?;
        self.edges.insert(id, n);
        Ok(n)
    }

    fn validate_edge_endpoints(&self, edge: &Record) -> Result<()> {
        let curve = self.model.records.get(edge.link(8)?).ok_or_else(|| invalid("dangling edge curve"))?;
        for (vertex_field, parameter_field) in [(3, 4), (5, 6)] {
            let v = self.model.record(edge.link(vertex_field)?, "vertex")?;
            let point = self.model.record(v.link(5)?, "point")?.position(3)?;
            let t = edge.number(parameter_field)?;
            let evaluated = match curve.kind.as_str() {
                "straight-curve" => Some(curve.position(3)? + curve.vector(4)? * t),
                "ellipse-curve" => {
                    let major = curve.vector(5)?;
                    Some(curve.position(3)? + major * t.cos() + curve.vector(4)?.cross(major) * (curve.number(6)? * t.sin()))
                }
                // The kernel validates spline endpoint placement while trimming the
                // curve; its supported numerical search handles the stored fit tolerance.
                _ => None,
            };
            if evaluated.is_some_and(|p| !p.is_finite() || (p - point).len() * self.model.scale > 1e-5) {
                return Err(invalid("invalid geometry: edge endpoint is not on its curve"));
            }
        }
        Ok(())
    }
    fn surface(&mut self, id: usize) -> Result<usize> {
        if let Some(n) = self.surfaces.get(&id) {
            return Ok(*n);
        }
        let r = self.model.records.get(id).ok_or_else(|| invalid("dangling surface"))?;
        let n = match r.kind.as_str() {
            "plane-surface" => {
                let axis = self.axis(r.position(3)?, r.vector(4)?, r.vector(5)?)?;
                self.add(format!("PLANE('',#{axis})"))?
            }
            "cone-surface" => {
                let base = r.position(3)?;
                let normal = r.vector(4)?;
                let major = r.vector(5)?;
                let ratio = r.number(6)?;
                let sin = r.number(9)?;
                let cos = r.number(10)?;
                if cos <= 0.0 || (sin * sin + cos * cos - 1.0).abs() > 1e-6 {
                    return Err(invalid("invalid cone angle"));
                }
                if (ratio - 1.0).abs() > 1e-12 {
                    if sin.abs() > 1e-12 {
                        return Err(invalid("unsupported elliptical cone"));
                    }
                    let e = self.ellipse(base, normal, major, ratio)?;
                    let d = self.dir(normal)?;
                    let v = self.add(format!("VECTOR('',#{d},1.)"))?;
                    self.add(format!("SURFACE_OF_LINEAR_EXTRUSION('',#{e},#{v})"))?
                } else {
                    let axis = self.axis(base, normal, major)?;
                    let radius = major.len() * self.model.scale;
                    if !(1e-9..=1e9).contains(&radius) {
                        return Err(invalid("invalid cone radius"));
                    }
                    if sin.abs() < 1e-12 {
                        self.add(format!("CYLINDRICAL_SURFACE('',#{axis},{})", real(radius)))?
                    } else {
                        self.add(format!("CONICAL_SURFACE('',#{axis},{},{})", real(radius), real(sin.atan2(cos))))?
                    }
                }
            }
            "sphere-surface" => {
                let axis = self.axis(r.position(3)?, r.vector(6)?, r.vector(5)?)?;
                let radius = r.number(4)? * self.model.scale;
                if !(1e-9..=1e9).contains(&radius) {
                    return Err(invalid("invalid sphere radius"));
                }
                self.add(format!("SPHERICAL_SURFACE('',#{axis},{})", real(radius)))?
            }
            "torus-surface" => {
                let axis = self.axis(r.position(3)?, r.vector(4)?, r.vector(7)?)?;
                let big = r.number(5)? * self.model.scale;
                let small = r.number(6)? * self.model.scale;
                if !(big > small && (1e-9..=1e9).contains(&small) && big <= 1e9) {
                    return Err(invalid("unsupported torus radii"));
                }
                self.add(format!("TOROIDAL_SURFACE('',#{axis},{},{})", real(big), real(small)))?
            }
            "spline-surface" => {
                let (u, v, points, tolerance) = spline(self.model, r.field(4)?, true)?;
                self.fit_warning(tolerance)?;
                let v = v.ok_or_else(|| invalid("missing spline v knots"))?;
                let mut rows = Vec::new();
                for i in 0..u.count {
                    let mut row = Vec::new();
                    for j in 0..v.count {
                        row.push(self.point(*points.get(j * u.count + i).ok_or_else(|| invalid("missing control point"))?)?);
                    }
                    rows.push(format!("({})", refs(&row)));
                }
                self.add(format!(
                    "B_SPLINE_SURFACE_WITH_KNOTS('',{},{},({}),.UNSPECIFIED.,.F.,.F.,.F.,({}),({}),({}),({}),.UNSPECIFIED.)",
                    u.degree,
                    v.degree,
                    rows.join(","),
                    integers(&u.mults),
                    integers(&v.mults),
                    numbers(&u.knots),
                    numbers(&v.knots)
                ))?
            }
            other => return Err(invalid(format!("unsupported geometry: surface {other} at record {id}"))),
        };
        self.surfaces.insert(id, n);
        Ok(n)
    }
    fn chain(&self, first: Option<usize>, kind: &str, owner_field: usize, owner: usize) -> Result<Vec<usize>> {
        let mut ids = Vec::new();
        let mut seen = HashSet::new();
        let mut next = first;
        while let Some(id) = next {
            if !seen.insert(id) || ids.len() >= 100_000 {
                return Err(invalid(format!("invalid topology: cyclic or oversized {kind} chain")));
            }
            let r = self.model.record(id, kind)?;
            if r.link(owner_field)? != owner {
                return Err(invalid(format!("invalid topology: {kind} owner mismatch")));
            }
            ids.push(id);
            next = r.ptr(3)?;
        }
        Ok(ids)
    }
    fn bound(&mut self, id: usize) -> Result<usize> {
        let lp = self.model.record(id, "loop")?;
        let start = lp.link(4)?;
        let mut at = start;
        let mut seen = HashSet::new();
        let mut edges = Vec::new();
        loop {
            if !seen.insert(at) || seen.len() > 100_000 {
                return Err(invalid("invalid topology: coedge cycle"));
            }
            let c = self.model.record(at, "coedge")?;
            let next = c.link(3)?;
            if c.link(8)? != id || self.model.record(next, "coedge")?.link(4)? != at {
                return Err(invalid("invalid topology: coedge owner or previous link"));
            }
            let partner = self.model.record(c.link(5)?, "coedge")?;
            if partner.link(5)? != at || partner.link(6)? != c.link(6)? {
                return Err(invalid("invalid topology: inconsistent partners"));
            }
            let edge = self.edge(c.link(6)?)?;
            edges.push(self.add(format!("ORIENTED_EDGE('',*,*,#{edge},{})", logical(!c.boolean(7)?)))?);
            at = next;
            if at == start {
                break;
            }
        }
        let lp = self.add(format!("EDGE_LOOP('',({}))", refs(&edges)))?;
        self.add(format!("FACE_BOUND('',#{lp},.T.)"))
    }
    fn face(&mut self, id: usize) -> Result<usize> {
        let f = self.model.record(id, "face")?;
        null(f, 6, "subshells")?;
        if f.boolean(9)? {
            return Err(invalid("unsupported double-sided face"));
        }
        let loops = self.chain(f.ptr(4)?, "loop", 5, id)?;
        if loops.is_empty() {
            return Err(invalid("unsupported boundaryless face"));
        }
        let s = self.model.records.get(f.link(7)?).ok_or_else(|| invalid("missing surface"))?;
        if s.kind == "cone-surface" && (s.number(6)? - 1.0).abs() > 1e-12 && s.number(9)?.abs() < 1e-12 && loops.len() == 2 {
            return self.elliptical_band(f, s, &loops);
        }
        let bounds = loops.into_iter().map(|i| self.bound(i)).collect::<Result<Vec<_>>>()?;
        let surface = self.surface(f.link(7)?)?;
        self.add(format!("ADVANCED_FACE('',({}),#{surface},{})", refs(&bounds), logical(!f.boolean(8)?)))
    }

    /// A periodic elliptical band needs a seam for the kernel's non-periodic NURBS
    /// parameterisation. Join its two closed rings with an exact straight seam, and
    /// represent the analytic ellipse as four rational quadratic spans (no fitting).
    fn elliptical_band(&mut self, face: &Record, surface: &Record, loops: &[usize]) -> Result<usize> {
        let mut rings = Vec::new();
        for &lp in loops {
            self.bound(lp)?; // Validate partners, owners and the complete source loop.
            let co = self.model.record(lp, "loop")?.link(4)?;
            let c = self.model.record(co, "coedge")?;
            if c.link(3)? != co {
                return Err(invalid("unsupported segmented elliptical ring"));
            }
            let e = self.model.record(c.link(6)?, "edge")?;
            if e.link(3)? != e.link(5)? {
                return Err(invalid("elliptical band requires closed ring edges"));
            }
            let vertex = e.link(3)?;
            let position = self.model.record(self.model.record(vertex, "vertex")?.link(5)?, "point")?.position(3)?;
            let edge = self.edge(c.link(6)?)?;
            let oriented = self.add(format!("ORIENTED_EDGE('',*,*,#{edge},{})", logical(!c.boolean(7)?)))?;
            rings.push((self.vertex(vertex)?, position, oriented));
        }
        let (Some(&(v0, p0, r0)), Some(&(v1, p1, r1))) = (rings.first(), rings.get(1)) else {
            return Err(invalid("missing elliptical rings"));
        };
        let z = unit(surface.vector(4)?)?;
        let base = surface.position(3)?;
        let major = surface.vector(5)?;
        let ratio = surface.number(6)?;
        if !(ratio > 0.0 && ratio <= 1.0 && (1e-9..=1e9).contains(&(major.len() * self.model.scale))) {
            return Err(invalid("invalid ellipse radii"));
        }
        let x = unit(major)?;
        if z.dot(x).abs() > 1e-6 {
            return Err(invalid("non-perpendicular surface axes"));
        }
        let y = z.cross(x);
        let delta = p1 - p0;
        if delta.cross(z).len() > 1e-7 || delta.len() < 1e-9 {
            return Err(invalid("elliptical ring seams are not aligned"));
        }
        let q = p0 - base;
        let angle = (q.dot(y) / (major.len() * ratio)).atan2(q.dot(x) / major.len());
        let height0 = (p0 - base).dot(z);
        let height1 = (p1 - base).dot(z);
        let (lo, hi) = (height0.min(height1), height0.max(height1));
        let mut rows = Vec::new();
        let mut weights = Vec::new();
        for i in 0..9 {
            let theta = angle + i as f64 * std::f64::consts::FRAC_PI_4;
            let weight = if i % 2 == 0 { 1.0 } else { std::f64::consts::FRAC_1_SQRT_2 };
            let p = base + (major * theta.cos() + y * (major.len() * ratio * theta.sin())) / weight;
            let a = self.point(p + z * lo)?;
            let b = self.point(p + z * hi)?;
            rows.push(format!("(#{a},#{b})"));
            weights.push(format!("({0},{0})", real(weight)));
        }
        let s = self.add(format!("(BOUNDED_SURFACE() B_SPLINE_SURFACE(2,1,({}),.UNSPECIFIED.,.F.,.F.,.F.) B_SPLINE_SURFACE_WITH_KNOTS((3,2,2,2,3),(2,2),(0.,1.,2.,3.,4.),(0.,1.),.UNSPECIFIED.) GEOMETRIC_REPRESENTATION_ITEM() RATIONAL_B_SPLINE_SURFACE(({})) REPRESENTATION_ITEM('') SURFACE())",rows.join(","),weights.join(",")))?;
        let p = self.point(p0)?;
        let direction = self.dir(delta)?;
        let vector = self.add(format!("VECTOR('',#{direction},1.)"))?;
        let line = self.add(format!("LINE('',#{p},#{vector})"))?;
        let edge = self.add(format!("EDGE_CURVE('',#{v0},#{v1},#{line},.T.)"))?;
        let forward = self.add(format!("ORIENTED_EDGE('',*,*,#{edge},.T.)"))?;
        let backward = self.add(format!("ORIENTED_EDGE('',*,*,#{edge},.F.)"))?;
        let lp = self.add(format!("EDGE_LOOP('',(#{r0},#{forward},#{r1},#{backward}))"))?;
        let bound = self.add(format!("FACE_BOUND('',#{lp},.T.)"))?;
        self.add(format!("ADVANCED_FACE('',(#{bound}),#{s},{})", logical(!face.boolean(8)?)))
    }
}

struct Knots {
    degree: usize,
    count: usize,
    knots: Vec<f64>,
    mults: Vec<usize>,
}
struct Fields<'a> {
    fields: &'a [Token],
    at: usize,
}
impl Fields<'_> {
    fn next(&mut self) -> Result<&Token> {
        let t = self.fields.get(self.at).ok_or_else(|| invalid("truncated spline"))?;
        self.at += 1;
        Ok(t)
    }
    fn int(&mut self) -> Result<usize> {
        match self.next()? {
            Token::Int(n) if *n >= 0 && *n <= 100_000 => Ok(*n as usize),
            _ => Err(invalid("invalid spline count")),
        }
    }
    fn number(&mut self) -> Result<f64> {
        match self.next()? {
            Token::Number(n) => Ok(*n),
            _ => Err(invalid("invalid spline number")),
        }
    }
    fn knots(&mut self, degree: usize, n: usize) -> Result<Knots> {
        if !(1..=16).contains(&degree) || !(2..=4096).contains(&n) {
            return Err(invalid("spline degree or knot count exceeds limit"));
        }
        let mut knots = Vec::new();
        let mut mults = Vec::new();
        let mut sum = 0usize;
        for i in 0..n {
            let k = self.number()?;
            let m = self.int()?;
            if m == 0 || m > degree + 1 || knots.last().is_some_and(|last| k <= *last) {
                return Err(invalid("invalid spline knots"));
            }
            knots.push(k);
            mults.push(m + usize::from(i == 0 || i + 1 == n));
            sum += m;
        }
        let count = sum
            .checked_add(1)
            .and_then(|n| n.checked_sub(degree))
            .filter(|n| *n > degree && *n <= 100_000)
            .ok_or_else(|| invalid("invalid spline control count"))?;
        Ok(Knots { degree, count, knots, mults })
    }
}
fn spline(model: &Model, t: &Token, surface: bool) -> Result<(Knots, Option<Knots>, Vec<Vec3>, f64)> {
    let Token::Object(id) = t else {
        return Err(invalid("missing spline subtype"));
    };
    let object = model.objects.get(*id).ok_or_else(|| invalid("invalid spline reference"))?;
    let at = object
        .iter()
        .position(|t| matches!(t, Token::Ident(s) if s=="nubs" || s=="nurbs"))
        .ok_or_else(|| invalid("unsupported spline subtype: no approximation"))?;
    if matches!(object.get(at),Some(Token::Ident(s)) if s=="nurbs") {
        return Err(invalid("unsupported rational spline encoding"));
    }
    let mut f = Fields { fields: object, at: at + 1 };
    let u_degree = f.int()?;
    let v_degree = if surface { Some(f.int()?) } else { None };
    for _ in 0..if surface { 4 } else { 1 } {
        if !matches!(f.next()?, Token::Enum(0)) {
            return Err(invalid("unsupported spline closure"));
        }
    }
    let nu = f.int()?;
    let nv = if surface { f.int()? } else { 0 };
    let u = f.knots(u_degree, nu)?;
    let v = v_degree.map(|d| f.knots(d, nv)).transpose()?;
    let count =
        u.count.checked_mul(v.as_ref().map_or(1, |v| v.count)).filter(|n| *n <= 100_000).ok_or_else(|| invalid("spline control net too large"))?;
    let mut points = Vec::new();
    for _ in 0..count {
        points.push(Vec3::new(f.number()?, f.number()?, f.number()?));
    }
    let tolerance = f.number()?;
    Ok((u, v, points, tolerance))
}

pub(super) fn convert(model: &Model) -> Result<Converted> {
    let body_count = model.records.iter().filter(|r| r.kind == "body").count();
    if body_count > 1024 {
        return Err(invalid("more than 1024 solid bodies"));
    }
    let mut w = Writer {
        model,
        text: String::new(),
        next: 1,
        vertices: HashMap::new(),
        edges: HashMap::new(),
        surfaces: HashMap::new(),
        warnings: Vec::new(),
    };
    let mut solids = Vec::new();
    for (id, body) in model.records.iter().enumerate().filter(|(_, r)| r.kind == "body") {
        null(body, 4, "wire bodies")?;
        null(body, 5, "transformed bodies")?;
        let lumps = w.chain(body.ptr(3)?, "lump", 5, id)?;
        if lumps.len() != 1 {
            return Err(invalid("unsupported body with zero or multiple lumps"));
        }
        for lump in lumps {
            let r = model.record(lump, "lump")?;
            let shells = w.chain(r.ptr(4)?, "shell", 7, lump)?;
            if shells.len() != 1 {
                return Err(invalid("unsupported body with zero or multiple shells"));
            }
            for shell in shells {
                let r = model.record(shell, "shell")?;
                null(r, 4, "nested shells")?;
                null(r, 6, "wire shells")?;
                let faces = w.chain(r.ptr(5)?, "face", 5, shell)?;
                // A complete sphere has no edge loops. Build the equivalent exact analytic
                // primitive; SolidCraft's STEP exporter supplies the kernel's required seams.
                if faces.len() == 1 && body_count == 1 {
                    let f = model.record(*faces.first().ok_or_else(|| invalid("missing face"))?, "face")?;
                    if f.ptr(4)?.is_none() {
                        null(f, 6, "subshells")?;
                        if f.boolean(8)? || f.boolean(9)? {
                            return Err(invalid("unsupported reversed or double-sided full sphere"));
                        }
                        let s = model.record(f.link(7)?, "sphere-surface")?;
                        let radius = s.number(4)? * model.scale;
                        w.point(s.position(3)?)?;
                        if !(1e-9..=1e9).contains(&radius) {
                            return Err(invalid("invalid sphere radius"));
                        }
                        let body = solidcraft_kernel::sphere(s.position(3)? * model.scale, radius)?;
                        return Ok(Converted {
                            text: solidcraft_kernel::step_export(&[&body], "SolidCraft IPT import")?,
                            bodies: 1,
                            warnings: Vec::new(),
                        });
                    }
                }
                let faces = faces.into_iter().map(|f| w.face(f)).collect::<Result<Vec<_>>>()?;
                if faces.is_empty() {
                    return Err(invalid("empty solid shell"));
                }
                let shell = w.add(format!("CLOSED_SHELL('',({}))", refs(&faces)))?;
                solids.push(w.add(format!("MANIFOLD_SOLID_BREP('Body{}',#{shell})", solids.len() + 1))?);
            }
        }
    }
    let unit = w.add("(LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.))".into())?;
    let angle = w.add("(NAMED_UNIT(*) PLANE_ANGLE_UNIT() SI_UNIT($,.RADIAN.))".into())?;
    let solidangle = w.add("(NAMED_UNIT(*) SI_UNIT($,.STERADIAN.) SOLID_ANGLE_UNIT())".into())?;
    let tolerance = w.add(format!("UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-5),#{unit},'','')"))?;
    let context=w.add(format!("(GEOMETRIC_REPRESENTATION_CONTEXT(3) GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#{tolerance})) GLOBAL_UNIT_ASSIGNED_CONTEXT((#{unit},#{angle},#{solidangle})) REPRESENTATION_CONTEXT('',''))"))?;
    w.add(format!("ADVANCED_BREP_SHAPE_REPRESENTATION('IPT import',({}),#{context})", refs(&solids)))?;
    Ok(Converted {
        text: format!(
            "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('IPT solid import'),'2;1');\nFILE_NAME('','',(''),(''),'SolidCraft','SolidCraft','');\nFILE_SCHEMA(('AUTOMOTIVE_DESIGN'));\nENDSEC;\nDATA;\n{}ENDSEC;\nEND-ISO-10303-21;\n",
            w.text
        ),
        bodies: solids.len(),
        warnings: w.warnings,
    })
}
