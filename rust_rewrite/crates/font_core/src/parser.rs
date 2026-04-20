use crate::crypto::{decrypt_header, encrypt_header};
use crate::error::FontCoreError;
use crate::io::{
    read_f32, read_i32, read_utf, split_preview_and_zip, write_f32, write_i32, write_utf,
};
use crate::model::{FontKind, FontMeta, GfontCompatibility, GfontFile, GlyphData, GlyphPathChunk};
use anyhow::{bail, Result};
use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::ZipArchive;
use zip::ZipWriter;

pub fn compatibility_for_version(version: i32) -> Result<GfontCompatibility> {
    match version {
        1..=4 => Ok(GfontCompatibility::V1To4Plain),
        5..=8 => Ok(GfontCompatibility::V5To8Encrypted),
        9 => Ok(GfontCompatibility::V9Encrypted),
        other => Err(FontCoreError::UnsupportedVersion(other).into()),
    }
}

pub fn parse_gfont(bytes: &[u8]) -> Result<GfontFile> {
    let mut r = Cursor::new(bytes);
    let version = read_i32(&mut r)?;
    let compatibility = compatibility_for_version(version)?;
    let mut header = match compatibility {
        GfontCompatibility::V1To4Plain => HeaderFields::read_plain(&mut r, version)?,
        GfontCompatibility::V5To8Encrypted => {
            let header_len = read_i32(&mut r)?;
            if header_len <= 0 {
                bail!("invalid encrypted header length: {header_len}");
            }
            let mut encrypted = vec![0u8; header_len as usize];
            r.read_exact(&mut encrypted)?;
            let plain = decrypt_header(&encrypted)?;
            let mut header_reader = Cursor::new(plain);
            HeaderFields::read_plain(&mut header_reader, version)?
        }
        GfontCompatibility::V9Encrypted => {
            return Err(FontCoreError::EncryptedHeader("v9 gfont header is not implemented yet".into()).into())
        }
    };
    let preview_count = read_i32(&mut r)?;

    let mut glyphs = HashMap::new();
    for _ in 0..preview_count.max(0) {
        let glyph = parse_glyph(&mut r, version, true)?;
        glyphs.insert(glyph.key.clone(), glyph);
    }

    let mut rest = Vec::new();
    r.read_to_end(&mut rest)?;
    let (_, zip_data) = split_preview_and_zip(&rest)?;

    Ok(GfontFile {
        meta: FontMeta {
            version,
            kind: header.kind,
            name: header.name,
            author: header.author,
            description: header.description,
            internal_name: header.internal_name,
            size: header.size,
            glyph_count: header.glyph_count,
            vendor: header.vendor.take(),
            password: header.password.take(),
            uuid: header.uuid.take(),
            file_path: None,
        },
        glyphs,
        zip_blob: Some(zip_data.to_vec()),
    })
}

pub fn parse_gfont_file(path: &Path) -> Result<GfontFile> {
    let bytes = fs::read(path)?;
    let mut file = parse_gfont(&bytes)?;
    file.meta.file_path = Some(path.to_path_buf());
    Ok(file)
}

pub fn write_gfont(_file: &GfontFile) -> Result<Vec<u8>> {
    let file = _file;
    let compatibility = compatibility_for_version(file.meta.version)?;

    let mut out = Vec::new();
    write_i32(&mut out, file.meta.version)?;
    match compatibility {
        GfontCompatibility::V1To4Plain => write_plain_header(&mut out, file)?,
        GfontCompatibility::V5To8Encrypted => {
            let mut header = Vec::new();
            write_plain_header(&mut header, file)?;
            let encrypted = encrypt_header(&header);
            write_i32(&mut out, encrypted.len() as i32)?;
            out.extend_from_slice(&encrypted);
        }
        GfontCompatibility::V9Encrypted => bail!("v9 gfont writing is not implemented currently"),
    }

    let mut glyphs: Vec<&GlyphData> = file.glyphs.values().collect();
    glyphs.sort_by(|a, b| a.key.cmp(&b.key));

    let preview_count = glyphs.len().min(30);
    write_i32(&mut out, preview_count as i32)?;
    for glyph in glyphs.iter().take(preview_count) {
        write_glyph(&mut out, glyph, file.meta.version, true)?;
    }

    let mut zip_cursor = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut zip_cursor);
        let options = SimpleFileOptions::default();
        for glyph in glyphs {
            let entry_name = encode_glyph_entry_name(&glyph.key, file.meta.version);
            zip.start_file(entry_name, options)?;
            write_glyph(&mut zip, glyph, file.meta.version, false)?;
        }
        zip.finish()?;
    }
    out.extend(zip_cursor.into_inner());
    Ok(out)
}

fn parse_glyph<R: Read>(r: &mut R, version: i32, preview_only: bool) -> Result<GlyphData> {
    let key = if version <= 7 {
        let mut b = [0u8; 2];
        r.read_exact(&mut b)?;
        let u = u16::from_be_bytes(b);
        char::from_u32(u as u32).unwrap_or('\u{FFFD}').to_string()
    } else {
        read_utf(r)?
    };

    if version >= 9 {
        let _ = read_f32(r)?;
    }

    let mut chunks = Vec::new();
    loop {
        let count = match read_i32(r) {
            Ok(n) => n,
            Err(_) => break,
        };
        if count < 0 || count > 10_000 {
            break;
        }
        let mut points = Vec::with_capacity(count as usize);
        for _ in 0..count {
            points.push(read_f32(r)?);
        }

        let verb_n = read_i32(r)?;
        if verb_n < 0 || verb_n > 10_000 {
            bail!("invalid glyph verb count: {verb_n}");
        }
        let mut verbs = vec![0u8; verb_n as usize];
        r.read_exact(&mut verbs)?;
        chunks.push(GlyphPathChunk { points, verbs });
        if preview_only {
            break;
        }
    }

    Ok(GlyphData {
        key,
        chunks,
        cached: true,
    })
}

impl GfontFile {
    pub fn load_glyph(&mut self, key: &str) -> Result<Option<&GlyphData>> {
        if self.glyphs.contains_key(key) {
            return Ok(self.glyphs.get(key));
        }
        let Some(zip_blob) = &self.zip_blob else {
            return Ok(None);
        };
        let mut zip = ZipArchive::new(Cursor::new(zip_blob.clone()))?;
        let entry_name = encode_glyph_entry_name(key, self.meta.version);
        let mut file = match zip.by_name(&entry_name) {
            Ok(file) => file,
            Err(_) => return Ok(None),
        };
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        let mut cursor = Cursor::new(bytes);
        let glyph = parse_glyph(&mut cursor, self.meta.version, false)?;
        let key_owned = glyph.key.clone();
        self.glyphs.insert(key_owned.clone(), glyph);
        Ok(self.glyphs.get(&key_owned))
    }
}

fn write_plain_header<W: Write>(w: &mut W, file: &GfontFile) -> Result<()> {
    write_utf(w, &file.meta.internal_name)?;
    write_i32(w, file.meta.kind.to_raw())?;
    write_utf(w, &file.meta.name)?;
    write_utf(w, &file.meta.author)?;
    write_utf(w, &file.meta.description)?;
    write_i32(w, file.meta.size)?;
    write_i32(w, file.glyphs.len() as i32)?;
    if file.meta.version >= 2 {
        write_utf(w, file.meta.vendor.as_deref().unwrap_or(""))?;
    }
    if file.meta.version >= 4 {
        write_utf(w, file.meta.password.as_deref().unwrap_or(""))?;
    }
    if file.meta.version >= 7 {
        write_utf(w, file.meta.uuid.as_deref().unwrap_or(""))?;
    }
    Ok(())
}

fn write_glyph<W: Write>(w: &mut W, glyph: &GlyphData, version: i32, preview_only: bool) -> Result<()> {
    if version <= 7 {
        let ch = glyph.key.chars().next().unwrap_or('\0') as u16;
        w.write_all(&ch.to_be_bytes())?;
    } else {
        write_utf(w, &glyph.key)?;
    }

    if version >= 9 {
        write_f32(w, 0.0)?;
    }

    for (idx, chunk) in glyph.chunks.iter().enumerate() {
        write_i32(w, chunk.points.len() as i32)?;
        for &point in &chunk.points {
            write_f32(w, point)?;
        }
        write_i32(w, chunk.verbs.len() as i32)?;
        w.write_all(&chunk.verbs)?;
        if preview_only && idx == 0 {
            break;
        }
    }
    Ok(())
}

fn encode_glyph_entry_name(key: &str, version: i32) -> String {
    if version < 3 {
        key.to_string()
    } else {
        key.chars()
            .map(|ch| ch.to_string())
            .collect::<Vec<_>>()
            .join("_")
    }
}

struct HeaderFields {
    internal_name: String,
    kind: FontKind,
    name: String,
    author: String,
    description: String,
    size: i32,
    glyph_count: i32,
    vendor: Option<String>,
    password: Option<String>,
    uuid: Option<String>,
}

impl HeaderFields {
    fn read_plain<R: Read>(r: &mut R, version: i32) -> Result<Self> {
        Ok(Self {
            internal_name: read_utf(r)?,
            kind: FontKind::from_raw(read_i32(r)?),
            name: read_utf(r)?,
            author: read_utf(r)?,
            description: read_utf(r)?,
            size: read_i32(r)?,
            glyph_count: read_i32(r)?,
            vendor: if version >= 2 { Some(read_utf(r)?) } else { None },
            password: if version >= 4 { Some(read_utf(r)?) } else { None },
            uuid: if version >= 7 { Some(read_utf(r)?) } else { None },
        })
    }
}
