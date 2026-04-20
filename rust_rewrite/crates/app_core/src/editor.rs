use crate::canvas::CanvasDocument;
use crate::history::CanvasHistory;
use crate::interaction::{CanvasInteractionState, DragTarget, HoverTarget, PointerButton};
use crate::tools::{ToolKind, ToolPointerButton, ToolSession};
use anyhow::Result;

#[derive(Debug, Clone, PartialEq)]
pub struct EditorCanvasState {
    pub document: CanvasDocument,
    pub interaction: CanvasInteractionState,
    pub active_tool: ToolSession,
    pub polygon_sides: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditorDisplayState {
    pub document: CanvasDocument,
    pub preview: Option<crate::canvas::CanvasPathObject>,
    pub selected_object: Option<usize>,
    pub hovered_object: Option<usize>,
    pub hovered_target: Option<HoverTarget>,
    pub selected_handles: Vec<crate::canvas::CurveHandlePoint>,
    pub selected_guides: Vec<crate::canvas::CurveGuideLine>,
    pub active_drag: Option<DragTarget>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EditorPointerResult {
    None,
    CanvasChanged,
    PreviewChanged,
    ViewPan { dx: f32, dy: f32 },
}

impl EditorCanvasState {
    pub fn new(document: CanvasDocument) -> Self {
        Self {
            document,
            interaction: CanvasInteractionState::new(),
            active_tool: ToolSession::new(ToolKind::Select),
            polygon_sides: 6,
        }
    }

    pub fn set_tool(&mut self, tool: ToolKind) {
        self.active_tool = ToolSession::new(tool);
        if tool != ToolKind::Select {
            self.interaction.pointer_released();
            self.interaction.clear_hover();
        }
    }

    pub fn tool_preview(&self) -> Option<&crate::canvas::CanvasPathObject> {
        self.active_tool.preview()
    }

    pub fn display_state(&self) -> EditorDisplayState {
        let selected_handles = self
            .interaction
            .selected_object
            .and_then(|index| self.document.object(index))
            .map(|object| object.curve_handle_points())
            .unwrap_or_default();
        let selected_guides = self
            .interaction
            .selected_object
            .and_then(|index| self.document.object(index))
            .map(|object| object.curve_guide_lines())
            .unwrap_or_default();

        EditorDisplayState {
            document: self.document.clone(),
            preview: self.active_tool.preview().cloned(),
            selected_object: self.interaction.selected_object,
            hovered_object: self.interaction.hovered_object,
            hovered_target: self.interaction.hovered_target,
            selected_handles,
            selected_guides,
            active_drag: self.interaction.active_drag,
        }
    }

    pub fn display_document(&self) -> CanvasDocument {
        let mut document = self.document.clone();
        if let Some(preview) = self.active_tool.preview() {
            document.add_object(preview.clone());
        }
        document
    }

    pub fn pointer_pressed(
        &mut self,
        history: &mut CanvasHistory,
        x: f32,
        y: f32,
        button: PointerButton,
    ) -> Result<EditorPointerResult> {
        match self.active_tool.tool {
            ToolKind::Select => {
                self.interaction
                    .pointer_pressed(&self.document, history, x, y, button);
                Ok(EditorPointerResult::None)
            }
            _ => {
                let tool_button = map_button(button);
                let created = self.active_tool.pointer_pressed(
                    x,
                    y,
                    tool_button,
                    Some(self.polygon_sides),
                )?;
                if let Some(object) = created {
                    if self.active_tool.tool != ToolKind::Line && self.active_tool.tool != ToolKind::Pen {
                        history.push(snapshot_canvas(&self.document));
                        return Ok(EditorPointerResult::PreviewChanged);
                    }
                    if self.active_tool.tool == ToolKind::Line || self.active_tool.tool == ToolKind::Pen {
                        if matches!(tool_button, ToolPointerButton::Secondary) {
                            history.push(snapshot_canvas(&self.document));
                            self.document.add_object(object);
                            return Ok(EditorPointerResult::CanvasChanged);
                        }
                        return Ok(EditorPointerResult::PreviewChanged);
                    }
                }
                Ok(EditorPointerResult::None)
            }
        }
    }

    pub fn pointer_moved(
        &mut self,
        x: f32,
        y: f32,
        button_down: bool,
    ) -> Result<EditorPointerResult> {
        match self.active_tool.tool {
            ToolKind::Select => {
                if !button_down {
                    let changed = self.interaction.hover_at(&self.document, x, y);
                    return if changed {
                        Ok(EditorPointerResult::PreviewChanged)
                    } else {
                        Ok(EditorPointerResult::None)
                    };
                }
                if self.interaction.active_drag.is_none() {
                    return Ok(EditorPointerResult::None);
                }
                match self.interaction.pointer_dragged(&mut self.document, x, y)? {
                    Some((dx, dy)) => Ok(EditorPointerResult::ViewPan { dx, dy }),
                    None => Ok(EditorPointerResult::CanvasChanged),
                }
            }
            _ => {
                let changed = self.active_tool.pointer_moved(x, y, button_down)?.is_some();
                if changed {
                    Ok(EditorPointerResult::PreviewChanged)
                } else {
                    Ok(EditorPointerResult::None)
                }
            }
        }
    }

    pub fn pointer_released(
        &mut self,
        history: &mut CanvasHistory,
        x: f32,
        y: f32,
    ) -> Result<EditorPointerResult> {
        match self.active_tool.tool {
            ToolKind::Select => {
                self.interaction.pointer_released();
                Ok(EditorPointerResult::None)
            }
            _ => {
                if let Some(object) = self.active_tool.pointer_released(x, y)? {
                    history.push(snapshot_canvas(&self.document));
                    self.document.add_object(object);
                    return Ok(EditorPointerResult::CanvasChanged);
                }
                if self.active_tool.preview().is_some() {
                    Ok(EditorPointerResult::PreviewChanged)
                } else {
                    Ok(EditorPointerResult::None)
                }
            }
        }
    }

    pub fn undo(&mut self, history: &mut CanvasHistory) -> Result<bool> {
        if self.active_tool.tool != ToolKind::Select {
            self.active_tool.cancel();
        }
        if !history.can_undo() {
            return Ok(false);
        }
        self.interaction.undo(&mut self.document, history)?;
        Ok(true)
    }

    pub fn redo(&mut self, history: &mut CanvasHistory) -> Result<bool> {
        if self.active_tool.tool != ToolKind::Select {
            self.active_tool.cancel();
        }
        if !history.can_redo() {
            return Ok(false);
        }
        self.interaction.redo(&mut self.document, history)?;
        Ok(true)
    }
}

fn map_button(button: PointerButton) -> ToolPointerButton {
    match button {
        PointerButton::Primary => ToolPointerButton::Primary,
        PointerButton::Middle => ToolPointerButton::Middle,
        PointerButton::Secondary => ToolPointerButton::Secondary,
    }
}

fn snapshot_canvas(document: &CanvasDocument) -> crate::history::CanvasEditSnapshot {
    crate::history::CanvasEditSnapshot {
        objects: document.to_chunks(),
    }
}
