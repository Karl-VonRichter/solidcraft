use super::{Cursor, MAX_RECORDS, MAX_TOKENS, invalid};
use crate::Result;
use solidcraft_geom::Vec3;

#[derive(Clone, Debug)]
pub(super) enum Token {
    Int(i32),
    Number(f64),
    Pointer(i32),
    Vector(Vec3),
    Position(Vec3),
    Bool(bool),
    Ident(String),
    Prefix(String),
    Text(String),
    Enum(i32),
    Object(usize),
    End,
}

#[derive(Debug)]
pub(super) struct Record {
    pub kind: String,
    pub fields: Vec<Token>,
}
impl Record {
    pub fn field(&self, n: usize) -> Result<&Token> {
        self.fields.get(n).ok_or_else(|| invalid(format!("{}: missing field {n}", self.kind)))
    }
    pub fn ptr(&self, n: usize) -> Result<Option<usize>> {
        match self.field(n)? {
            Token::Pointer(-1) => Ok(None),
            Token::Pointer(i) if *i >= 0 => Ok(Some(*i as usize)),
            _ => Err(invalid(format!("{}: invalid pointer {n}", self.kind))),
        }
    }
    pub fn link(&self, n: usize) -> Result<usize> {
        self.ptr(n)?.ok_or_else(|| invalid(format!("{}: null required pointer {n}", self.kind)))
    }
    pub fn number(&self, n: usize) -> Result<f64> {
        match self.field(n)? {
            Token::Number(x) => Ok(*x),
            _ => Err(invalid("expected a number")),
        }
    }
    pub fn boolean(&self, n: usize) -> Result<bool> {
        match self.field(n)? {
            Token::Bool(x) => Ok(*x),
            _ => Err(invalid("expected a boolean")),
        }
    }
    pub fn vector(&self, n: usize) -> Result<Vec3> {
        match self.field(n)? {
            Token::Vector(v) => Ok(*v),
            _ => Err(invalid("expected a vector")),
        }
    }
    pub fn position(&self, n: usize) -> Result<Vec3> {
        match self.field(n)? {
            Token::Position(v) => Ok(*v),
            _ => Err(invalid("expected a position")),
        }
    }
}

pub(super) struct Model {
    pub records: Vec<Record>,
    pub objects: Vec<Vec<Token>>,
    pub scale: f64,
    pub version: i32,
}
impl Model {
    pub fn record(&self, id: usize, kind: &str) -> Result<&Record> {
        let r = self.records.get(id).ok_or_else(|| invalid(format!("record {id}: dangling pointer")))?;
        if r.kind != kind {
            return Err(invalid(format!("record {id}: expected {kind}, found {}", r.kind)));
        }
        Ok(r)
    }
}

struct Parser<'a> {
    cursor: Cursor<'a>,
    objects: Vec<Vec<Token>>,
    tokens: usize,
}
impl Parser<'_> {
    fn token(&mut self, depth: usize) -> Result<Token> {
        self.tokens += 1;
        if self.tokens > MAX_TOKENS || depth > 64 {
            return Err(invalid("geometry token or nesting limit exceeded"));
        }
        let tag = self.cursor.u8()?;
        Ok(match tag {
            0x04 => Token::Int(self.cursor.i32()?),
            0x06 => Token::Number(self.cursor.f64()?),
            0x0c => Token::Pointer(self.cursor.i32()?),
            0x15 => Token::Enum(self.cursor.i32()?),
            0x0a => Token::Bool(true),
            0x0b => Token::Bool(false),
            0x11 => Token::End,
            0x13 | 0x14 => {
                let v = Vec3::new(self.cursor.f64()?, self.cursor.f64()?, self.cursor.f64()?);
                if tag == 0x13 { Token::Position(v) } else { Token::Vector(v) }
            }
            0x07 | 0x0d | 0x0e => {
                let n = self.cursor.u8()? as usize;
                let bytes = self.cursor.take(n)?;
                if tag != 7 && !bytes.is_ascii() {
                    return Err(invalid("non-ASCII geometry identifier"));
                }
                let text: String = bytes.iter().map(|b| char::from(*b)).collect();
                match tag {
                    7 => Token::Text(text),
                    13 => Token::Ident(text),
                    _ => Token::Prefix(text),
                }
            }
            0x0f => {
                let first = self.token(depth + 1)?;
                if matches!(&first, Token::Ident(s) if s == "ref") {
                    let Token::Int(n) = self.token(depth + 1)? else {
                        return Err(invalid("invalid subtype reference"));
                    };
                    if n < 0 || n as usize >= self.objects.len() || self.objects.get(n as usize).is_none_or(Vec::is_empty) {
                        return Err(invalid("dangling or recursive subtype reference"));
                    }
                    if self.cursor.u8()? != 0x10 {
                        return Err(invalid("unterminated subtype reference"));
                    }
                    Token::Object(n as usize)
                } else {
                    let id = self.objects.len();
                    self.objects.push(Vec::new());
                    let mut fields = vec![first];
                    while self.cursor.bytes.get(self.cursor.pos) != Some(&0x10) {
                        fields.push(self.token(depth + 1)?);
                    }
                    self.cursor.u8()?;
                    if let Some(slot) = self.objects.get_mut(id) {
                        *slot = fields;
                    }
                    Token::Object(id)
                }
            }
            _ => return Err(invalid(format!("unsupported geometry token 0x{tag:02x}"))),
        })
    }
    fn kind(&mut self) -> Result<String> {
        let mut parts = Vec::new();
        loop {
            if parts.len() > 16 {
                return Err(invalid("geometry type name too long"));
            }
            match self.token(0)? {
                Token::Prefix(s) => parts.push(s),
                Token::Ident(s) => {
                    parts.push(s);
                    return Ok(parts.join("-"));
                }
                _ => return Err(invalid("expected a geometry record type")),
            }
        }
    }
}

pub(super) fn parse(bytes: &[u8]) -> Result<Model> {
    let start = bytes.windows(14).position(|s| s == b"ASM BinaryFile").ok_or_else(|| invalid("missing ASM geometry"))?;
    let mut parser =
        Parser { cursor: Cursor::new(bytes.get(start..).ok_or_else(|| invalid("invalid geometry offset"))?), objects: Vec::new(), tokens: 0 };
    if parser.cursor.take(15)? != b"ASM BinaryFile4" {
        return Err(invalid("unsupported ASM header"));
    }
    let version = parser.cursor.i32()?;
    let count = parser.cursor.i32()?;
    let entities = parser.cursor.i32()?;
    let flags = parser.cursor.i32()?;
    if count < 0 || count as usize > MAX_RECORDS || entities < 1 || entities as usize > MAX_RECORDS || flags != 3 {
        return Err(invalid("unsupported ASM counts or flags"));
    }
    for _ in 0..3 {
        if !matches!(parser.token(0)?, Token::Text(s) if !s.is_empty()) {
            return Err(invalid("invalid ASM header string"));
        }
    }
    let Token::Number(scale) = parser.token(0)? else {
        return Err(invalid("missing model scale"));
    };
    if !(1e-9..=1e9).contains(&scale) {
        return Err(invalid("invalid model scale"));
    }
    for _ in 0..2 {
        if !matches!(parser.token(0)?, Token::Number(n) if n > 0.0 && n < 1.0) {
            return Err(invalid("invalid model tolerance"));
        }
    }
    let mut records = Vec::new();
    let mut history = false;
    loop {
        let kind = parser.kind()?;
        if kind == "End-of-ASM-data" {
            break;
        }
        if kind == "End-of-ASM-History-Section" {
            continue;
        }
        if kind == "Begin-of-ASM-History-Data" {
            history = true;
        }
        let mut fields = Vec::new();
        loop {
            let t = parser.token(0)?;
            if matches!(t, Token::End) {
                break;
            }
            fields.push(t);
        }
        if !history {
            if records.len() >= MAX_RECORDS {
                return Err(invalid("too many geometry records"));
            }
            records.push(Record { kind, fields });
        }
    }
    if records.first().is_none_or(|r| r.kind != "asmheader") {
        return Err(invalid("missing ASM header record"));
    }
    if records.iter().filter(|r| r.kind == "body").count() + 1 != entities as usize {
        return Err(invalid("ASM body count mismatch"));
    }
    Ok(Model { records, objects: parser.objects, scale, version })
}
