use crate::editor::{EditorCanvasState, EditorPointerResult};
use crate::history::CanvasHistory;
use crate::interaction::PointerButton;
use crate::state::FontEditorState;
use crate::tools::ToolKind;
use crate::trace::{trace_and_apply_to_selected_glyph, AppliedTrace, TraceSelectionMode};
use anyhow::Result;
use font_core::GlyphPathChunk;
use native_algorithms::{AutoTraceRequest, NativeAlgorithms};

#[derive(Debug)]
pub struct FontGlyphSession {
    pub editor_state: FontEditorState,
    pub canvas_state: EditorCanvasState,
    pub canvas_history: CanvasHistory,
}

impl FontGlyphSession {
    pub fn new(mut editor_state: FontEditorState) -> Result<Self> {
        if editor_state.selected_glyph.is_none() {
            if let Some(first) = editor_state.visible_glyph_keys.first().cloned() {
                editor_state.select_glyph(&first)?;
            }
        }

        let document = create_document_or_empty(&editor_state)?;
        Ok(Self {
            editor_state,
            canvas_state: EditorCanvasState::new(document),
            canvas_history: CanvasHistory::new(),
        })
    }

    pub fn select_glyph(&mut self, key: &str) -> Result<()> {
        self.editor_state.select_glyph(key)?;
        self.reload_canvas_from_selected_glyph()?;
        Ok(())
    }

    pub fn set_tool(&mut self, tool: ToolKind) {
        self.canvas_state.set_tool(tool);
    }

    pub fn set_polygon_sides(&mut self, sides: usize) {
        self.canvas_state.polygon_sides = sides.max(3);
    }

    pub fn visible_glyph_keys(&self) -> &[String] {
        &self.editor_state.visible_glyph_keys
    }

    pub fn selected_glyph_key(&self) -> Option<&str> {
        self.editor_state.selected_glyph.as_deref()
    }

    pub fn selected_path_index(&self) -> Option<usize> {
        self.editor_state.selected_path_index
    }

    pub fn select_next_visible_glyph(&mut self) -> Result<Option<String>> {
        let next = self.editor_state.select_next_visible_glyph()?;
        if next.is_some() {
            self.reload_canvas_from_selected_glyph()?;
        }
        Ok(next)
    }

    pub fn select_previous_visible_glyph(&mut self) -> Result<Option<String>> {
        let previous = self.editor_state.select_previous_visible_glyph()?;
        if previous.is_some() {
            self.reload_canvas_from_selected_glyph()?;
        }
        Ok(previous)
    }

    pub fn display_document(&self) -> crate::canvas::CanvasDocument {
        self.canvas_state.display_document()
    }

    pub fn display_state(&self) -> crate::editor::EditorDisplayState {
        self.canvas_state.display_state()
    }

    pub fn tool_preview(&self) -> Option<&crate::canvas::CanvasPathObject> {
        self.canvas_state.tool_preview()
    }

    pub fn search(&mut self, text: &str) -> Result<()> {
        self.editor_state.search(text);
        self.reload_canvas_from_selected_glyph()
    }

    pub fn clear_search(&mut self) -> Result<()> {
        self.editor_state.clear_search();
        self.reload_canvas_from_selected_glyph()
    }

    pub fn add_missing_chars_from_text(&mut self, text: &str) -> usize {
        let before = self.editor_state.font.glyphs.len();
        self.editor_state.add_missing_chars_from_text(text);
        self.editor_state.font.glyphs.len().saturating_sub(before)
    }

    pub fn select_path(&mut self, index: usize) -> Result<()> {
        self.editor_state.select_path(index)
    }

    pub fn append_style_from_selected_path(&mut self) -> Result<usize> {
        let source_index = self
            .editor_state
            .selected_path_index
            .ok_or_else(|| anyhow::anyhow!("no path selected"))?;
        let next_index = self.editor_state.append_style_from_path(source_index)?;
        self.reload_canvas_from_selected_glyph()?;
        self.editor_state.select_path(next_index)?;
        Ok(next_index)
    }

    pub fn replace_selected_path(&mut self, chunk: GlyphPathChunk) -> Result<()> {
        self.editor_state.replace_selected_path(chunk)?;
        self.reload_canvas_from_selected_glyph()
    }

    pub fn clear_selected_path(&mut self) -> Result<()> {
        self.editor_state.clear_selected_path()?;
        self.reload_canvas_from_selected_glyph()
    }

    pub fn clear_all_paths(&mut self) -> Result<()> {
        self.editor_state.clear_all_paths()?;
        self.reload_canvas_from_selected_glyph()
    }

    pub fn undo_path_edit(&mut self) -> Result<bool> {
        if !self.editor_state.can_undo_path_edit() {
            return Ok(false);
        }
        self.editor_state.undo_path_edit()?;
        self.reload_canvas_from_selected_glyph()?;
        Ok(true)
    }

    pub fn redo_path_edit(&mut self) -> Result<bool> {
        if !self.editor_state.can_redo_path_edit() {
            return Ok(false);
        }
        self.editor_state.redo_path_edit()?;
        self.reload_canvas_from_selected_glyph()?;
        Ok(true)
    }

    pub fn trace_selected_glyph(
        &mut self,
        native: &dyn NativeAlgorithms,
        argb_pixels: &[i32],
        request: &AutoTraceRequest,
        mode: TraceSelectionMode,
    ) -> Result<AppliedTrace> {
        let applied = trace_and_apply_to_selected_glyph(
            &mut self.editor_state,
            native,
            argb_pixels,
            request,
            mode,
        )?;
        self.reload_canvas_from_selected_glyph()?;
        self.editor_state.select_path(applied.selected_path_index)?;
        Ok(applied)
    }

    pub fn finish_selected_glyph_and_select_next_unfinished(&mut self) -> Result<Option<String>> {
        self.commit_canvas_to_selected_glyph()?;
        let next = self.editor_state.finish_selected_glyph_and_select_next_unfinished()?;
        if next.is_some() {
            self.reload_canvas_from_selected_glyph()?;
        }
        Ok(next)
    }

    pub fn pointer_pressed(
        &mut self,
        x: f32,
        y: f32,
        button: PointerButton,
    ) -> Result<EditorPointerResult> {
        self.canvas_state
            .pointer_pressed(&mut self.canvas_history, x, y, button)
    }

    pub fn pointer_moved(
        &mut self,
        x: f32,
        y: f32,
        button_down: bool,
    ) -> Result<EditorPointerResult> {
        let result = self.canvas_state.pointer_moved(x, y, button_down)?;
        if matches!(result, EditorPointerResult::CanvasChanged) {
            self.editor_state
                .sync_selected_glyph_from_canvas_document(&self.canvas_state.document)?;
        }
        Ok(result)
    }

    pub fn pointer_released(
        &mut self,
        x: f32,
        y: f32,
    ) -> Result<EditorPointerResult> {
        let result = self
            .canvas_state
            .pointer_released(&mut self.canvas_history, x, y)?;
        if matches!(result, EditorPointerResult::CanvasChanged) {
            self.editor_state
                .apply_canvas_document_to_selected_glyph(&self.canvas_state.document)?;
        }
        Ok(result)
    }

    pub fn delete_selected_canvas_object(&mut self) -> Result<bool> {
        let changed = self
            .canvas_state
            .delete_selected_object(&mut self.canvas_history)?;
        if changed {
            self.editor_state
                .apply_canvas_document_to_selected_glyph(&self.canvas_state.document)?;
        }
        Ok(changed)
    }

    pub fn commit_canvas_to_selected_glyph(&mut self) -> Result<()> {
        self.editor_state
            .apply_canvas_document_to_selected_glyph(&self.canvas_state.document)
    }

    pub fn reload_canvas_from_selected_glyph(&mut self) -> Result<()> {
        self.canvas_state.document = create_document_or_empty(&self.editor_state)?;
        self.canvas_history.clear();
        Ok(())
    }

    pub fn undo_canvas(&mut self) -> Result<bool> {
        let changed = self.canvas_state.undo(&mut self.canvas_history)?;
        if changed {
            self.editor_state
                .sync_selected_glyph_from_canvas_document(&self.canvas_state.document)?;
        }
        Ok(changed)
    }

    pub fn redo_canvas(&mut self) -> Result<bool> {
        let changed = self.canvas_state.redo(&mut self.canvas_history)?;
        if changed {
            self.editor_state
                .sync_selected_glyph_from_canvas_document(&self.canvas_state.document)?;
        }
        Ok(changed)
    }

    pub fn save_font_to(&self, path: &std::path::Path) -> Result<()> {
        self.editor_state.save_to(path)
    }
}

fn create_document_or_empty(editor_state: &FontEditorState) -> Result<crate::canvas::CanvasDocument> {
    if editor_state.selected_glyph.is_some() {
        editor_state.create_canvas_document_for_selected_glyph()
    } else {
        Ok(crate::canvas::CanvasDocument::new())
    }
}
