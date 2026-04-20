use anyhow::{anyhow, Result};
use font_core::GlyphPathChunk;

#[derive(Debug, Clone, PartialEq)]
pub struct PathEditSnapshot {
    pub glyph_key: String,
    pub selected_path_index: usize,
    pub chunks: Vec<GlyphPathChunk>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasEditSnapshot {
    pub objects: Vec<GlyphPathChunk>,
}

#[derive(Debug, Default)]
pub struct PathHistory {
    undo: Vec<PathEditSnapshot>,
    redo: Vec<PathEditSnapshot>,
}

#[derive(Debug, Default)]
pub struct CanvasHistory {
    undo: Vec<CanvasEditSnapshot>,
    redo: Vec<CanvasEditSnapshot>,
}

impl PathHistory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, snapshot: PathEditSnapshot) {
        self.undo.push(snapshot);
        self.redo.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self, current: PathEditSnapshot) -> Result<PathEditSnapshot> {
        let previous = self
            .undo
            .pop()
            .ok_or_else(|| anyhow!("no undo snapshot available"))?;
        self.redo.push(current);
        Ok(previous)
    }

    pub fn redo(&mut self, current: PathEditSnapshot) -> Result<PathEditSnapshot> {
        let next = self
            .redo
            .pop()
            .ok_or_else(|| anyhow!("no redo snapshot available"))?;
        self.undo.push(current);
        Ok(next)
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

impl CanvasHistory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, snapshot: CanvasEditSnapshot) {
        self.undo.push(snapshot);
        self.redo.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self, current: CanvasEditSnapshot) -> Result<CanvasEditSnapshot> {
        let previous = self
            .undo
            .pop()
            .ok_or_else(|| anyhow!("no undo snapshot available"))?;
        self.redo.push(current);
        Ok(previous)
    }

    pub fn redo(&mut self, current: CanvasEditSnapshot) -> Result<CanvasEditSnapshot> {
        let next = self
            .redo
            .pop()
            .ok_or_else(|| anyhow!("no redo snapshot available"))?;
        self.undo.push(current);
        Ok(next)
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}
