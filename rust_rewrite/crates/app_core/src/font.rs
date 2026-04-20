use anyhow::Result;
use font_core::{parse_gfont_file, write_gfont, FontKind, FontMeta, GfontFile};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontDraft {
    pub name: String,
    pub kind: FontKind,
    pub author: String,
    pub description: String,
    pub password: Option<String>,
}

pub fn create_font(draft: FontDraft) -> GfontFile {
    GfontFile {
        meta: FontMeta {
            version: 8,
            kind: draft.kind,
            name: draft.name,
            author: draft.author,
            description: draft.description,
            internal_name: "kvenjoy".to_string(),
            size: 300,
            glyph_count: 0,
            vendor: Some("kvenjoy".to_string()),
            password: draft.password,
            uuid: None,
            file_path: None,
        },
        glyphs: HashMap::new(),
        zip_blob: None,
    }
}

pub fn open_font(path: &Path) -> Result<GfontFile> {
    parse_gfont_file(path)
}

pub fn save_font(_font: &GfontFile, _path: &Path) -> Result<()> {
    let bytes = write_gfont(_font)?;
    fs::write(_path, bytes)?;
    Ok(())
}
