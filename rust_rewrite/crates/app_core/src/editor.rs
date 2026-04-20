use crate::canvas::CanvasDocument;
use crate::history::CanvasHistory;
use crate::interaction::{CanvasInteractionState, PointerButton};
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
pub enum EditorPointerResult {
    None,
    CanvasChanged,
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
                    }
                    if self.active_tool.tool == ToolKind::Line || self.active_tool.tool == ToolKind::Pen {
                        if matches!(tool_button, ToolPointerButton::Secondary) {
                            history.push(snapshot_canvas(&self.document));
                            self.document.add_object(object);
                            return Ok(EditorPointerResult::CanvasChanged);
                        }
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
                if !button_down || self.interaction.active_drag.is_none() {
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
                    Ok(EditorPointerResult::CanvasChanged)
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
                Ok(EditorPointerResult::None)
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
