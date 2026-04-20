use anyhow::{anyhow, Result};
use crate::font::save_font;
use font_core::GfontFile;

#[derive(Debug)]
pub struct FontEditorState {
    pub font: GfontFile,
    pub visible_glyph_keys: Vec<String>,
    pub selected_glyph: Option<String>,
    pub selected_path_index: Option<usize>,
    pub search_mode: bool,
}

impl FontEditorState {
    pub fn new(font: GfontFile) -> Self {
        let visible_glyph_keys = font.list_glyph_keys();
        Self {
            font,
            visible_glyph_keys,
            selected_glyph: None,
            selected_path_index: None,
            search_mode: false,
        }
    }

    pub fn select_glyph(&mut self, key: &str) -> Result<()> {
        if !self.font.has_glyph(key) {
            let _ = self.font.load_glyph(key)?;
        }
        if !self.font.has_glyph(key) {
            return Err(anyhow!("glyph not found: {key}"));
        }
        self.selected_glyph = Some(key.to_string());
        self.selected_path_index = None;
        Ok(())
    }

    pub fn select_path(&mut self, index: usize) -> Result<()> {
        let key = self
            .selected_glyph
            .as_deref()
            .ok_or_else(|| anyhow!("no glyph selected"))?;
        let glyph = self
            .font
            .get_glyph(key)
            .ok_or_else(|| anyhow!("glyph disappeared: {key}"))?;
        if index >= glyph.path_count() {
            return Err(anyhow!("path index out of range: {index}"));
        }
        self.selected_path_index = Some(index);
        Ok(())
    }

    pub fn search(&mut self, text: &str) {
        self.visible_glyph_keys = text
            .chars()
            .map(|ch| ch.to_string())
            .filter(|key| self.font.has_glyph(key))
            .collect();
        self.search_mode = true;
    }

    pub fn clear_search(&mut self) {
        self.visible_glyph_keys = self.font.list_glyph_keys();
        self.search_mode = false;
    }

    pub fn add_missing_chars_from_text(&mut self, text: &str) {
        self.font.add_missing_tokens_from_text(text);
        if !self.search_mode {
            self.visible_glyph_keys = self.font.list_glyph_keys();
        }
    }

    pub fn save_to(&self, _path: &std::path::Path) -> Result<()> {
        save_font(&self.font, _path)
    }
}
