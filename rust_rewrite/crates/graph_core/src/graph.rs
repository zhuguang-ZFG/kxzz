use anyhow::Result;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use font_core::GlyphPathChunk;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphDraft {
    pub name: String,
    pub author: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphDocument {
    pub version: i32,
    pub uuid: Option<String>,
    pub name: String,
    pub author: String,
    pub description: String,
    pub paths: Vec<GlyphPathChunk>,
}

pub fn create_graph(draft: GraphDraft) -> GraphDocument {
    GraphDocument {
        version: 2,
        uuid: None,
        name: draft.name,
        author: draft.author,
        description: draft.description,
        paths: Vec::new(),
    }
}

pub fn open_graph(_path: &Path) -> Result<GraphDocument> {
    let bytes = fs::read(_path)?;
    parse_gap(&bytes)
}

pub fn save_graph(_doc: &GraphDocument, _path: &Path) -> Result<()> {
    let bytes = write_gap(_doc)?;
    fs::write(_path, bytes)?;
    Ok(())
}

fn parse_gap(bytes: &[u8]) -> Result<GraphDocument> {
    let mut gz = GzDecoder::new(Cursor::new(bytes));
    let mut raw = Vec::new();
    gz.read_to_end(&mut raw)?;
    let mut r = Cursor::new(raw);

    let version = read_i32(&mut r)?;
    let uuid = read_utf(&mut r)?;
    let name = read_utf(&mut r)?;
    let author = read_utf(&mut r)?;
    let description = read_utf(&mut r)?;

    if version <= 1 {
        let legacy_n = read_i32(&mut r)?;
        for _ in 0..legacy_n {
            let mut skip = [0u8; 8];
            r.read_exact(&mut skip)?;
            let _ = read_utf(&mut r)?;
        }
    }

    let count = read_i32(&mut r)?;
    let mut paths = Vec::with_capacity(count.max(0) as usize);
    for _ in 0..count.max(0) {
        paths.push(read_chunk(&mut r)?);
    }

    Ok(GraphDocument {
        version,
        uuid: Some(uuid),
        name,
        author,
        description,
        paths,
    })
}

fn write_gap(doc: &GraphDocument) -> Result<Vec<u8>> {
    let mut raw = Vec::new();
    write_i32(&mut raw, doc.version)?;
    write_utf(&mut raw, doc.uuid.as_deref().unwrap_or(""))?;
    write_utf(&mut raw, &doc.name)?;
    write_utf(&mut raw, &doc.author)?;
    write_utf(&mut raw, &doc.description)?;
    write_i32(&mut raw, doc.paths.len() as i32)?;
    for path in &doc.paths {
        write_chunk(&mut raw, path)?;
    }

    let mut out = Vec::new();
    {
        let mut gz = GzEncoder::new(&mut out, Compression::default());
        gz.write_all(&raw)?;
        gz.finish()?;
    }
    Ok(out)
}

fn read_i32<R: Read>(r: &mut R) -> Result<i32> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(i32::from_be_bytes(buf))
}

fn read_f32<R: Read>(r: &mut R) -> Result<f32> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(f32::from_bits(u32::from_be_bytes(buf)))
}

fn read_u16<R: Read>(r: &mut R) -> Result<u16> {
    let mut buf = [0u8; 2];
    r.read_exact(&mut buf)?;
    Ok(u16::from_be_bytes(buf))
}

fn read_utf<R: Read>(r: &mut R) -> Result<String> {
    let len = read_u16(r)? as usize;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).to_string())
}

fn write_i32<W: Write>(w: &mut W, value: i32) -> Result<()> {
    w.write_all(&value.to_be_bytes())?;
    Ok(())
}

fn write_f32<W: Write>(w: &mut W, value: f32) -> Result<()> {
    w.write_all(&value.to_bits().to_be_bytes())?;
    Ok(())
}

fn write_u16<W: Write>(w: &mut W, value: u16) -> Result<()> {
    w.write_all(&value.to_be_bytes())?;
    Ok(())
}

fn write_utf<W: Write>(w: &mut W, value: &str) -> Result<()> {
    let bytes = value.as_bytes();
    let len: u16 = bytes
        .len()
        .try_into()
        .map_err(|_| anyhow::anyhow!("utf string too long"))?;
    write_u16(w, len)?;
    w.write_all(bytes)?;
    Ok(())
}

fn read_chunk<R: Read>(r: &mut R) -> Result<GlyphPathChunk> {
    let point_count = read_i32(r)?;
    let mut points = Vec::with_capacity(point_count.max(0) as usize);
    for _ in 0..point_count.max(0) {
        points.push(read_f32(r)?);
    }
    let verb_count = read_i32(r)?;
    let mut verbs = vec![0u8; verb_count.max(0) as usize];
    r.read_exact(&mut verbs)?;
    Ok(GlyphPathChunk { points, verbs })
}

fn write_chunk<W: Write>(w: &mut W, chunk: &GlyphPathChunk) -> Result<()> {
    write_i32(w, chunk.points.len() as i32)?;
    for &point in &chunk.points {
        write_f32(w, point)?;
    }
    write_i32(w, chunk.verbs.len() as i32)?;
    w.write_all(&chunk.verbs)?;
    Ok(())
}
