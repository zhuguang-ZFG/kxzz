use anyhow::{anyhow, Result};
use crate::font::save_font;
use crate::history::{PathEditSnapshot, PathHistory};
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
    pub path_history: PathHistory,
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
            path_history: PathHistory::new(),
        }
    }

    pub fn select_glyph(&mut self, key: &str) -> Result<()> {
        if !self.font.has_glyph(key) {
            let _ = self.font.load_glyph(key)?;
        }
        if !self.font.has_glyph(key) {
            return Err(anyhow!("glyph not found: {key}"));
        }
        let changed = self.selected_glyph.as_deref() != Some(key);
        self.selected_glyph = Some(key.to_string());
        self.rebuild_path_slots();
        self.selected_path_index = self.path_slots.first().map(|slot| slot.index);
        if changed {
            self.path_history.clear();
        }
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
        self.push_history_snapshot()?;
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
        self.push_history_snapshot()?;
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

        self.push_history_snapshot()?;
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

    pub fn append_path(&mut self, chunk: GlyphPathChunk) -> Result<usize> {
        self.push_history_snapshot()?;
        let glyph = self.selected_glyph_mut()?;
        if glyph.path_count() >= MAX_STYLE_COUNT {
            return Err(anyhow!("path count limit reached: {MAX_STYLE_COUNT}"));
        }

        glyph.push_path(chunk);
        let next_index = glyph.path_count() - 1;
        self.rebuild_path_slots();
        self.selected_path_index = Some(next_index);
        Ok(next_index)
    }

    pub fn clear_all_paths(&mut self) -> Result<()> {
        self.push_history_snapshot()?;
        let glyph = self.selected_glyph_mut()?;
        glyph.chunks.clear();
        self.rebuild_path_slots();
        self.selected_path_index = self.path_slots.first().map(|slot| slot.index);
        Ok(())
    }

    pub fn can_undo_path_edit(&self) -> bool {
        self.path_history.can_undo()
    }

    pub fn can_redo_path_edit(&self) -> bool {
        self.path_history.can_redo()
    }

    pub fn undo_path_edit(&mut self) -> Result<()> {
        let current = self.capture_path_snapshot()?;
        let previous = self.path_history.undo(current)?;
        self.apply_path_snapshot(previous)
    }

    pub fn redo_path_edit(&mut self) -> Result<()> {
        let current = self.capture_path_snapshot()?;
        let next = self.path_history.redo(current)?;
        self.apply_path_snapshot(next)
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

    fn push_history_snapshot(&mut self) -> Result<()> {
        let snapshot = self.capture_path_snapshot()?;
        self.path_history.push(snapshot);
        Ok(())
    }

    fn capture_path_snapshot(&self) -> Result<PathEditSnapshot> {
        let glyph_key = self
            .selected_glyph
            .clone()
            .ok_or_else(|| anyhow!("no glyph selected"))?;
        let glyph = self
            .font
            .get_glyph(&glyph_key)
            .ok_or_else(|| anyhow!("glyph disappeared: {glyph_key}"))?;
        Ok(PathEditSnapshot {
            glyph_key,
            selected_path_index: self.selected_path_index.unwrap_or(0),
            chunks: glyph.chunks.clone(),
        })
    }

    fn apply_path_snapshot(&mut self, snapshot: PathEditSnapshot) -> Result<()> {
        let glyph = self
            .font
            .get_glyph_mut(&snapshot.glyph_key)
            .ok_or_else(|| anyhow!("glyph disappeared: {}", snapshot.glyph_key))?;
        glyph.chunks = snapshot.chunks;
        self.selected_glyph = Some(snapshot.glyph_key);
        self.rebuild_path_slots();
        let fallback_index = self.path_slots.first().map(|slot| slot.index);
        let target_index = if self.path_slots.iter().any(|slot| slot.index == snapshot.selected_path_index) {
            Some(snapshot.selected_path_index)
        } else {
            fallback_index
        };
        self.selected_path_index = target_index;
        Ok(())
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
