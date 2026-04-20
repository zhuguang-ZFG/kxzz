use anyhow::{anyhow, Result};
use crate::font::save_font;
use font_core::{GfontFile, GlyphData, GlyphPathChunk};

const MAX_STYLE_COUNT: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlyphPathSlot {
    pub index: usize,
    pub has_data: bool,
}

#[derive(Debug)]
pub struct FontEditorState {
    pub font: GfontFile,
    pub visible_glyph_keys: Vec<String>,
    pub path_slots: Vec<GlyphPathSlot>,
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
            path_slots: Vec::new(),
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
        self.rebuild_path_slots();
        self.selected_path_index = self.path_slots.first().map(|slot| slot.index);
        Ok(())
    }

    pub fn select_path(&mut self, index: usize) -> Result<()> {
        if self.selected_glyph.is_none() {
            return Err(anyhow!("no glyph selected"));
        }
        if !self.path_slots.iter().any(|slot| slot.index == index) {
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
        self.select_first_visible_glyph();
    }

    pub fn clear_search(&mut self) {
        self.visible_glyph_keys = self.font.list_glyph_keys();
        self.search_mode = false;
        self.select_first_visible_glyph();
    }

    pub fn add_missing_chars_from_text(&mut self, text: &str) {
        self.font.add_missing_tokens_from_text(text);
        if !self.search_mode {
            self.visible_glyph_keys = self.font.list_glyph_keys();
        }
    }

    pub fn selected_glyph(&self) -> Result<&GlyphData> {
        let key = self
            .selected_glyph
            .as_deref()
            .ok_or_else(|| anyhow!("no glyph selected"))?;
        self.font
            .get_glyph(key)
            .ok_or_else(|| anyhow!("glyph disappeared: {key}"))
    }

    pub fn selected_glyph_mut(&mut self) -> Result<&mut GlyphData> {
        let key = self
            .selected_glyph
            .clone()
            .ok_or_else(|| anyhow!("no glyph selected"))?;
        self.font
            .get_glyph_mut(&key)
            .ok_or_else(|| anyhow!("glyph disappeared: {key}"))
    }

    pub fn selected_path(&self) -> Result<Option<&GlyphPathChunk>> {
        let Some(index) = self.selected_path_index else {
            return Ok(None);
        };
        Ok(self.selected_glyph()?.get_path(index))
    }

    pub fn selected_path_mut(&mut self) -> Result<Option<&mut GlyphPathChunk>> {
        let Some(index) = self.selected_path_index else {
            return Ok(None);
        };
        Ok(self.selected_glyph_mut()?.get_path_mut(index))
    }

    pub fn replace_selected_path(&mut self, chunk: GlyphPathChunk) -> Result<()> {
        let index = self
            .selected_path_index
            .ok_or_else(|| anyhow!("no path selected"))?;
        let glyph = self.selected_glyph_mut()?;
        if index < glyph.path_count() {
            glyph.chunks[index] = chunk;
        } else if index == glyph.path_count() {
            glyph.push_path(chunk);
        } else {
            return Err(anyhow!("path index out of range: {index}"));
        }
        self.rebuild_path_slots();
        self.selected_path_index = Some(index);
        Ok(())
    }

    pub fn clear_selected_path(&mut self) -> Result<()> {
        let index = self
            .selected_path_index
            .ok_or_else(|| anyhow!("no path selected"))?;
        let glyph = self.selected_glyph_mut()?;
        if index < glyph.path_count() {
            glyph.chunks.remove(index);
        }
        self.rebuild_path_slots();
        self.selected_path_index = self.path_slots.first().map(|slot| slot.index);
        Ok(())
    }

    pub fn append_style_from_path(&mut self, source_index: usize) -> Result<usize> {
        let source = {
            let glyph = self.selected_glyph()?;
            let source = glyph
                .get_path(source_index)
                .ok_or_else(|| anyhow!("path index out of range: {source_index}"))?;
            source.clone()
        };

        let next_index = {
            let glyph = self.selected_glyph_mut()?;
            if glyph.path_count() >= MAX_STYLE_COUNT {
                return Err(anyhow!("path count limit reached: {MAX_STYLE_COUNT}"));
            }
            glyph.push_path(source);
            glyph.path_count() - 1
        };

        self.rebuild_path_slots();
        self.selected_path_index = Some(next_index);
        Ok(next_index)
    }

    pub fn finish_selected_glyph_and_select_next_unfinished(&mut self) -> Result<Option<String>> {
        let current = self
            .selected_glyph
            .clone()
            .ok_or_else(|| anyhow!("no glyph selected"))?;

        let start = self
            .visible_glyph_keys
            .iter()
            .position(|key| key == &current)
            .map(|idx| idx + 1)
            .unwrap_or(0);

        let candidates: Vec<String> = self.visible_glyph_keys.iter().skip(start).cloned().collect();
        for next_key in candidates {
            let is_unfinished = match self.font.load_glyph(&next_key)? {
                Some(glyph) => glyph.path_count() == 0,
                None => true,
            };
            if is_unfinished {
                self.select_glyph(&next_key)?;
                return Ok(Some(next_key));
            }
        }

        Ok(None)
    }

    pub fn selected_path_count(&self) -> Result<usize> {
        Ok(self.selected_glyph()?.path_count())
    }

    pub fn save_to(&self, _path: &std::path::Path) -> Result<()> {
        save_font(&self.font, _path)
    }

    fn rebuild_path_slots(&mut self) {
        let path_count = self
            .selected_glyph
            .as_deref()
            .and_then(|key| self.font.get_glyph(key))
            .map(|glyph| glyph.path_count())
            .unwrap_or(0);

        let slot_count = path_count.max(1);
        self.path_slots = (0..slot_count)
            .map(|index| GlyphPathSlot {
                index,
                has_data: index < path_count,
            })
            .collect();
    }

    fn select_first_visible_glyph(&mut self) {
        self.selected_glyph = None;
        self.selected_path_index = None;
        self.path_slots.clear();

        if let Some(first) = self.visible_glyph_keys.first().cloned() {
            let _ = self.select_glyph(&first);
        }
    }
}
