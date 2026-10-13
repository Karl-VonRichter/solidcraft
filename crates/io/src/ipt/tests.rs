use super::asm::{Model, Record, Token};
use super::*;
use solidcraft_geom::Vec3;
use std::{
    collections::HashMap,
    io::{Cursor as IoCursor, Write},
};

fn rec(kind: &str, fields: Vec<Token>) -> Record {
    let mut all = vec![Token::Pointer(-1), Token::Int(-1), Token::Pointer(-1)];
    all.extend(fields);
    Record { kind: kind.into(), fields: all }
}
fn ptr(i: usize) -> Token {
    Token::Pointer(i as i32)
}
fn add(model: &mut Model, r: Record) -> usize {
    let i = model.records.len();
    model.records.push(r);
    i
}

/// Original synthetic 1 x 2 x 3 model-unit box. Analytic volume is 6000 mm³.
fn box_model() -> Model {
    use Token::{Bool as B, Int as I, Number as N, Pointer as P, Position as Pos, Text as S, Vector as V};
    let mut m = Model {
        records: vec![Record { kind: "asmheader".into(), fields: vec![P(-1), I(-1), S("Synthetic".into())] }],
        objects: Vec::new(),
        scale: 10.0,
        version: 23100,
    };
    add(&mut m, rec("body", vec![ptr(2), P(-1), P(-1)]));
    add(&mut m, rec("lump", vec![P(-1), ptr(3), ptr(1)]));
    add(&mut m, rec("shell", vec![P(-1), P(-1), P(-1), P(-1), ptr(2)]));
    let points: Vec<_> = (0..8).map(|i| Vec3::new((i & 1) as f64, ((i >> 1) & 1) as f64 * 2.0, ((i >> 2) & 1) as f64 * 3.0)).collect();
    let mut vertices = Vec::new();
    for p in &points {
        let p = add(&mut m, rec("point", vec![Pos(*p)]));
        vertices.push(add(&mut m, rec("vertex", vec![P(-1), I(0), ptr(p)])));
    }
    let quads = [[0, 2, 3, 1], [4, 5, 7, 6], [0, 1, 5, 4], [2, 6, 7, 3], [0, 4, 6, 2], [1, 3, 7, 5]];
    let mut edges = HashMap::new();
    let mut faces = Vec::new();
    let mut partners: HashMap<usize, usize> = HashMap::new();
    for q in quads {
        let u = points[q[1]] - points[q[0]];
        let v = points[q[2]] - points[q[1]];
        let surface = add(&mut m, rec("plane-surface", vec![Pos(points[q[0]]), V(u.cross(v).normalized().unwrap()), V(u.normalized().unwrap())]));
        let face = add(&mut m, rec("face", vec![P(-1), P(-1), ptr(3), P(-1), ptr(surface), B(false), B(false)]));
        faces.push(face);
        let lp = add(&mut m, rec("loop", vec![P(-1), P(-1), ptr(face)]));
        m.records[face].fields[4] = ptr(lp);
        let mut coedges = Vec::new();
        for i in 0..4 {
            let a = q[i];
            let b = q[(i + 1) % 4];
            let key = (a.min(b), a.max(b));
            let (edge, reversed) = if let Some(&(edge, start)) = edges.get(&key) {
                (edge, start != a)
            } else {
                let d = points[b] - points[a];
                let curve = add(&mut m, rec("straight-curve", vec![Pos(points[a]), V(d.normalized().unwrap()), B(false), B(false)]));
                let edge = add(
                    &mut m,
                    rec("edge", vec![ptr(vertices[a]), N(0.), ptr(vertices[b]), N(d.len()), P(-1), ptr(curve), B(false), S("unknown".into())]),
                );
                m.records[vertices[a]].fields[3] = ptr(edge);
                m.records[vertices[b]].fields[3] = ptr(edge);
                edges.insert(key, (edge, a));
                (edge, false)
            };
            let co = add(&mut m, rec("coedge", vec![P(-1), P(-1), P(-1), ptr(edge), B(reversed), ptr(lp), I(0), P(-1)]));
            if let Some(&other) = partners.get(&edge) {
                m.records[co].fields[5] = ptr(other);
                m.records[other].fields[5] = ptr(co);
            } else {
                partners.insert(edge, co);
            }
            m.records[edge].fields[7] = ptr(co);
            coedges.push(co);
        }
        m.records[lp].fields[4] = ptr(coedges[0]);
        for i in 0..4 {
            m.records[coedges[i]].fields[3] = ptr(coedges[(i + 1) % 4]);
            m.records[coedges[i]].fields[4] = ptr(coedges[(i + 3) % 4]);
        }
    }
    m.records[3].fields[5] = ptr(faces[0]);
    for pair in faces.windows(2) {
        m.records[pair[0]].fields[3] = ptr(pair[1]);
    }
    m
}

fn string(out: &mut Vec<u8>, tag: u8, s: &str) {
    out.push(tag);
    out.push(s.len() as u8);
    out.extend(s.as_bytes());
}
fn token(out: &mut Vec<u8>, t: &Token) {
    match t {
        Token::Int(n) | Token::Enum(n) | Token::Pointer(n) => {
            out.push(match t {
                Token::Int(_) => 4,
                Token::Enum(_) => 21,
                _ => 12,
            });
            out.extend(n.to_le_bytes());
        }
        Token::Number(n) => {
            out.push(6);
            out.extend(n.to_le_bytes());
        }
        Token::Vector(p) | Token::Position(p) => {
            out.push(if matches!(t, Token::Vector(_)) { 20 } else { 19 });
            for n in [p.x, p.y, p.z] {
                out.extend(n.to_le_bytes());
            }
        }
        Token::Text(s) => string(out, 7, s),
        Token::Ident(s) => string(out, 13, s),
        Token::Prefix(s) => string(out, 14, s),
        Token::Bool(b) => out.push(if *b { 10 } else { 11 }),
        Token::End => out.push(17),
        Token::Object(_) => panic!("synthetic box has no subtype"),
    }
}
fn encode(model: &Model) -> Vec<u8> {
    let mut bytes = b"ASM BinaryFile4".to_vec();
    for n in [model.version, 0, model.records.iter().filter(|r| r.kind == "body").count() as i32 + 1, 3] {
        bytes.extend(n.to_le_bytes());
    }
    for s in ["SolidCraft test", "Synthetic", "2026"] {
        string(&mut bytes, 7, s);
    }
    for n in [model.scale, 1e-6, 1e-10] {
        token(&mut bytes, &Token::Number(n));
    }
    for r in &model.records {
        string(&mut bytes, 13, &r.kind);
        for t in &r.fields {
            token(&mut bytes, t);
        }
        bytes.push(17);
    }
    string(&mut bytes, 13, "End-of-ASM-data");
    bytes
}

fn container(brep: &[u8]) -> Vec<u8> {
    let mut file = cfb::CompoundFile::create_with_version(cfb::Version::V3, IoCursor::new(Vec::new())).unwrap();
    file.set_storage_clsid("/", "4d29b490-49b2-11d0-93c3-7e0706000000".parse().unwrap()).unwrap();
    file.create_storage("/RSeStorage").unwrap();
    for (i, name) in [
        "PmBRepSegment",
        "PmDCSegment",
        "PmBrowserSegment",
        "PmResultSegment",
        "PmGraphicsSegment",
        "PmAppSegment",
        "DesignViewSegment",
        "NBNotebookSegment",
    ]
    .iter()
    .enumerate()
    {
        let mut meta = 25i32.to_le_bytes().to_vec();
        meta.extend(b"RSe Meta Stream Version 8");
        meta.extend([0; 18]);
        meta.extend((name.len() as i32).to_le_bytes());
        for c in name.encode_utf16() {
            meta.extend(c.to_le_bytes());
        }
        file.create_stream(format!("/RSeStorage/M{i}")).unwrap().write_all(&meta).unwrap();
        let mut data = vec![0x9e, 0xc2, 0x2b, 0xa4, 0xd4, 0x11, 0xe8, 0x01, 0x60, 0x00, 0x2d, 0xb3, 0xee, 0x29, 0xfb, 0xb0, 0x04, 1];
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        z.write_all(if i == 0 { brep } else { b"unused" }).unwrap();
        data.extend(z.finish().unwrap());
        file.create_stream(format!("/RSeStorage/B{i}")).unwrap().write_all(&data).unwrap();
    }
    file.into_inner().into_inner()
}

#[test]
fn synthetic_box_imports_as_a_closed_solid_and_reopens() {
    let bytes = container(&encode(&box_model()));
    let f = import_feature(&bytes, "Box.IPT").unwrap().unwrap();
    let mut doc = solidcraft_doc::Document::new("box");
    doc.add_feature(f.kind, None).unwrap();
    let reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
    assert_eq!(reopened, doc);
    let mut model = solidcraft_doc::Model::new();
    model.evaluate(&reopened);
    assert!(model.results.iter().all(|r| r.error.is_none()));
    let state = model.state();
    assert_eq!(state.bodies.len(), 1);
    let measure = solidcraft_kernel::measure(&state.bodies[0].body).unwrap();
    assert!((measure.volume - 6000.0).abs() < 1e-6, "{measure:?}");
    assert_eq!(measure.bbox.min, Vec3::ZERO);
    assert!((measure.bbox.max - Vec3::new(10., 20., 30.)).len() < 1e-8);
}

#[test]
fn malformed_container_and_geometry_never_panic() {
    let bytes = container(&encode(&box_model()));
    for end in (0..bytes.len()).step_by(113) {
        assert!(import_feature(&bytes[..end], "bad.ipt").is_err());
    }
    let asm = encode(&box_model());
    for end in 0..asm.len() {
        assert!(asm::parse(&asm[..end]).is_err());
    }
    let mut m = box_model();
    m.records[2].fields[3] = ptr(2);
    assert!(step::convert(&m).is_err());
    let mut m = box_model();
    let r = m.records.iter_mut().find(|r| r.kind == "coedge").unwrap();
    r.fields[5] = ptr(usize::MAX >> 1);
    assert!(step::convert(&m).is_err());
    let mut m = box_model();
    m.scale = f64::NAN;
    assert!(asm::parse(&encode(&m)).is_err());
    let mut m = box_model();
    m.records[1].fields[5] = ptr(1);
    assert!(step::convert(&m).is_err());
    let mut m = box_model();
    let r = m.records.iter_mut().find(|r| r.kind == "edge").unwrap();
    r.fields[4] = Token::Number(0.5);
    assert!(step::convert(&m).err().unwrap().to_string().contains("endpoint"));
}

#[test]
fn empty_part_is_valid_and_extensions_are_exact() {
    let mut model = box_model();
    model.records.truncate(1);
    assert!(import_feature(&container(&encode(&model)), "empty.ipt").unwrap().is_none());
    assert!(is_ipt_path("a.IPT"));
    assert!(!is_ipt_path("a.ipt.json"));
    assert!(!is_ipt_path("ipt"));
}

/// Original analytic elliptical cylinder, with two closed rings and no seam in
/// the source. This specifically exercises the IPT-to-kernel seam construction.
fn elliptical_model() -> Model {
    use Token::{Bool as B, Int as I, Number as N, Pointer as P, Position as Pos, Text as S, Vector as V};
    let mut m = box_model();
    m.records.truncate(4);
    let mut edge_ids = Vec::new();
    for height in [0.0, 3.0] {
        let p = add(&mut m, rec("point", vec![Pos(Vec3::new(-1., 0., height))]));
        let v = add(&mut m, rec("vertex", vec![P(-1), I(0), ptr(p)]));
        let c = add(&mut m, rec("ellipse-curve", vec![Pos(Vec3::new(0., 0., height)), V(Vec3::Z), V(Vec3::X), N(0.5), B(false), B(false)]));
        let e = add(
            &mut m,
            rec("edge", vec![ptr(v), N(-std::f64::consts::PI), ptr(v), N(std::f64::consts::PI), P(-1), ptr(c), B(false), S("unknown".into())]),
        );
        m.records[v].fields[3] = ptr(e);
        edge_ids.push(e);
    }
    let bottom = add(&mut m, rec("plane-surface", vec![Pos(Vec3::ZERO), V(-Vec3::Z), V(Vec3::X)]));
    let top = add(&mut m, rec("plane-surface", vec![Pos(Vec3::new(0., 0., 3.)), V(Vec3::Z), V(Vec3::X)]));
    let side = add(&mut m, rec("cone-surface", vec![Pos(Vec3::ZERO), V(Vec3::Z), V(Vec3::X), N(0.5), B(false), B(false), N(0.), N(1.), N(1.)]));
    let mut partners = HashMap::new();
    let mut faces = Vec::new();
    for (s, edges) in
        [(bottom, vec![(edge_ids[0], true)]), (top, vec![(edge_ids[1], false)]), (side, vec![(edge_ids[0], false), (edge_ids[1], true)])]
    {
        let f = add(&mut m, rec("face", vec![P(-1), P(-1), ptr(3), P(-1), ptr(s), B(false), B(false)]));
        faces.push(f);
        let mut loops = Vec::new();
        for (e, reversed) in edges {
            let lp = add(&mut m, rec("loop", vec![P(-1), P(-1), ptr(f)]));
            loops.push(lp);
            let co = add(&mut m, rec("coedge", vec![P(-1), P(-1), P(-1), ptr(e), B(reversed), ptr(lp), I(0), P(-1)]));
            m.records[lp].fields[4] = ptr(co);
            m.records[co].fields[3] = ptr(co);
            m.records[co].fields[4] = ptr(co);
            m.records[e].fields[7] = ptr(co);
            if let Some(&other) = partners.get(&e) {
                m.records[co].fields[5] = ptr(other);
                m.records[other].fields[5] = ptr(co);
            } else {
                partners.insert(e, co);
            }
        }
        m.records[f].fields[4] = ptr(loops[0]);
        for pair in loops.windows(2) {
            m.records[pair[0]].fields[3] = ptr(pair[1]);
        }
    }
    m.records[3].fields[5] = ptr(faces[0]);
    for pair in faces.windows(2) {
        m.records[pair[0]].fields[3] = ptr(pair[1]);
    }
    m
}

#[test]
fn elliptical_cylinder_has_a_complete_mesh_and_analytic_volume() {
    let f = import_feature(&container(&encode(&elliptical_model())), "ellipse.ipt").unwrap().unwrap();
    let solidcraft_doc::FeatureKind::Import { step, .. } = f.kind else { panic!("expected base feature") };
    let imported = solidcraft_kernel::step_import(&step).unwrap();
    assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
    let body = &imported.bodies[0];
    assert!(body.closed);
    let measured = solidcraft_kernel::measure(&body.body).unwrap();
    let expected = std::f64::consts::PI * 10.0 * 5.0 * 30.0;
    assert!((measured.volume - expected).abs() / expected < 0.0002, "{measured:?}");
    assert!(!body.body.display_mesh(0.05).unwrap().triangles.is_empty());
}
