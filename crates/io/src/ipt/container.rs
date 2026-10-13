use super::{Cursor, MAX_IPT_BYTES, invalid};
use crate::Result;
use std::{
    collections::HashSet,
    io::{Cursor as IoCursor, Read},
    path::Path,
};

pub(super) struct Data {
    pub brep: Vec<u8>,
}

fn expand(method: u8, bytes: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    match method {
        1 => {
            let mut decoder = flate2::Decompress::new(true);
            let mut block = [0; 16384];
            loop {
                let before_in = decoder.total_in();
                let before_out = decoder.total_out();
                let input = bytes.get(before_in as usize..).ok_or_else(|| invalid("invalid zlib position"))?;
                let status = decoder.decompress(input, &mut block, flate2::FlushDecompress::None).map_err(|_| invalid("invalid zlib compression"))?;
                let produced = (decoder.total_out() - before_out) as usize;
                if out.len().saturating_add(produced) > MAX_IPT_BYTES {
                    return Err(invalid("decompressed segment exceeds 64 MiB"));
                }
                out.extend(block.get(..produced).ok_or_else(|| invalid("invalid decompressor output"))?);
                if status == flate2::Status::StreamEnd {
                    if decoder.total_in() != bytes.len() as u64 {
                        return Err(invalid("trailing zlib data"));
                    }
                    break;
                }
                if decoder.total_in() == before_in && produced == 0 {
                    return Err(invalid("incomplete zlib data"));
                }
            }
        }
        2 => {
            let mut decoder = ruzstd::decoding::StreamingDecoder::new_with_max_window_size(bytes, MAX_IPT_BYTES as u64)
                .map_err(|_| invalid("invalid Zstandard header"))?;
            decoder.by_ref().take(MAX_IPT_BYTES as u64 + 1).read_to_end(&mut out).map_err(|_| invalid("invalid Zstandard compression"))?;
            if !decoder.decoder.is_finished() || !decoder.into_inner().is_empty() {
                return Err(invalid("trailing or incomplete Zstandard frame"));
            }
        }
        _ => return Err(invalid(format!("unsupported compression method {method}"))),
    }
    if out.len() > MAX_IPT_BYTES {
        return Err(invalid("decompressed segment exceeds 64 MiB"));
    }
    Ok(out)
}

pub(super) fn read(bytes: &[u8]) -> Result<Data> {
    if bytes.len() > MAX_IPT_BYTES {
        return Err(invalid("file exceeds 64 MiB"));
    }
    if bytes.get(..8) != Some(&[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]) || bytes.get(26..28) != Some(&[3, 0]) {
        return Err(invalid("expected a version 3 Compound File container"));
    }
    let mut cfb = cfb::CompoundFile::open_strict(IoCursor::new(bytes)).map_err(|_| invalid("invalid Compound File container"))?;
    if cfb.root_entry().clsid().as_u128() != 0x4d29b49049b211d093c37e0706000000 {
        return Err(invalid("unsupported document kind (only standard parts are supported)"));
    }
    let entries: Vec<_> = cfb.read_storage("/RSeStorage").map_err(|_| invalid("missing RSeStorage"))?.take(1025).collect();
    if entries.len() > 1024 {
        return Err(invalid("too many model streams"));
    }
    let mut types = HashSet::new();
    let mut brep = None;
    let mut total = 0usize;
    for entry in entries.iter().filter(|e| e.is_stream() && e.name().starts_with('M')) {
        if entry.len() > MAX_IPT_BYTES as u64 {
            return Err(invalid("metadata stream too large"));
        }
        let mut meta = Vec::new();
        cfb.open_stream(entry.path())
            .map_err(|_| invalid("invalid metadata stream"))?
            // Only the fixed header and bounded type name are needed. The opaque
            // compressed metadata can be very large and is deliberately not copied.
            .take(512)
            .read_to_end(&mut meta)
            .map_err(|_| invalid("damaged metadata stream"))?;
        let mut cursor = Cursor::new(&meta);
        let version_len = cursor.i32()?;
        let version = cursor.take(25)?;
        if version_len != 25 || version != b"RSe Meta Stream Version 8" {
            return Err(invalid("unsupported segment metadata version"));
        }
        cursor.take(18)?;
        let len = cursor.i32()?;
        if !(1..=128).contains(&len) {
            return Err(invalid("invalid segment name length"));
        }
        let utf16 = cursor
            .take(len as usize * 2)?
            .chunks_exact(2)
            .map(|b| b.try_into().map(u16::from_le_bytes).map_err(|_| invalid("truncated UTF-16 code unit")))
            .collect::<Result<Vec<_>>>()?;
        let name = String::from_utf16(&utf16).map_err(|_| invalid("invalid segment name"))?;
        if !types.insert(name.clone()) {
            return Err(invalid(format!("duplicate segment {name}")));
        }
        let suffix = entry.name().strip_prefix('M').ok_or_else(|| invalid("invalid stream pair"))?;
        let path = format!("/RSeStorage/B{suffix}");
        let data_entry = cfb.entry(Path::new(&path)).map_err(|_| invalid("missing paired model stream"))?;
        if data_entry.len() > MAX_IPT_BYTES as u64 {
            return Err(invalid("model stream too large"));
        }
        // Only decode the authoritative B-rep. Other segments contain design intent and display caches.
        if name != "PmBRepSegment" {
            continue;
        }
        let mut data = Vec::new();
        cfb.open_stream(&path)
            .map_err(|_| invalid("invalid model stream"))?
            .take(MAX_IPT_BYTES as u64 + 1)
            .read_to_end(&mut data)
            .map_err(|_| invalid("damaged model stream"))?;
        let mut cursor = Cursor::new(&data);
        if cursor.take(17)? != [0x9e, 0xc2, 0x2b, 0xa4, 0xd4, 0x11, 0xe8, 0x01, 0x60, 0x00, 0x2d, 0xb3, 0xee, 0x29, 0xfb, 0xb0, 0x04] {
            return Err(invalid("unsupported model segment layout"));
        }
        let method = cursor.u8()?;
        let expanded = expand(method, cursor.take(data.len().saturating_sub(18))?)?;
        total += expanded.len();
        if total > MAX_IPT_BYTES {
            return Err(invalid("model exceeds memory limit"));
        }
        brep = Some(expanded);
    }
    for name in [
        "PmBRepSegment",
        "PmDCSegment",
        "PmBrowserSegment",
        "PmResultSegment",
        "PmGraphicsSegment",
        "PmAppSegment",
        "DesignViewSegment",
        "NBNotebookSegment",
    ] {
        if !types.contains(name) {
            return Err(invalid(format!("missing segment {name}")));
        }
    }
    Ok(Data { brep: brep.ok_or_else(|| invalid("missing solid geometry"))? })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn compressed_streams_require_their_end_and_no_trailing_bytes() {
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(b"geometry").unwrap();
        let zlib = encoder.finish().unwrap();
        assert_eq!(expand(1, &zlib).unwrap(), b"geometry");
        for end in 0..zlib.len() {
            assert!(expand(1, &zlib[..end]).is_err());
        }
        let mut trailing = zlib;
        trailing.push(0);
        assert!(expand(1, &trailing).is_err());
        // RFC 8878: single-segment frame, eight-byte content, one last raw block.
        let mut zstd = vec![0x28, 0xb5, 0x2f, 0xfd, 0x20, 8, 65, 0, 0];
        zstd.extend(b"geometry");
        assert_eq!(expand(2, &zstd).unwrap(), b"geometry");
        for end in 0..zstd.len() {
            assert!(expand(2, &zstd[..end]).is_err());
        }
        zstd.push(0);
        assert!(expand(2, &zstd).is_err());
        assert!(expand(99, b"anything").is_err());
    }
}
