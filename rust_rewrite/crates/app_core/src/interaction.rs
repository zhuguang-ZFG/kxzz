use crate::canvas::{CanvasDocument, CurveHandleHit};
use crate::history::{CanvasEditSnapshot, CanvasHistory};
use anyhow::{anyhow, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerButton {
    Primary,
    Middle,
    Secondary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragTarget {
    Pan,
    Bounds { object_index: usize },
    CurveAnchor { object_index: usize, point_index: usize },
    CurveControl { object_index: usize, point_index: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasInteractionState {
    pub active_drag: Option<DragTarget>,
    pub selected_object: Option<usize>,
    pub hovered_object: Option<usize>,
    pub last_pointer: Option<(f32, f32)>,
    drag_snapshot_active: bool,
}

impl Default for CanvasInteractionState {
    fn default() -> Self {
        Self {
            active_drag: None,
            selected_object: None,
            hovered_object: None,
            last_pointer: None,
            drag_snapshot_active: false,
        }
    }
}

impl CanvasInteractionState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pointer_pressed(
        &mut self,
        document: &CanvasDocument,
        history: &mut CanvasHistory,
        x: f32,
        y: f32,
        button: PointerButton,
    ) {
        self.last_pointer = Some((x, y));
        self.drag_snapshot_active = false;

        if button == PointerButton::Middle {
            self.active_drag = Some(DragTarget::Pan);
            return;
        }

        if let Some((object_index, hit)) = find_curve_control_hit(document, x, y) {
            history.push(capture_canvas_snapshot(document));
            self.drag_snapshot_active = true;
            self.selected_object = Some(object_index);
            self.hovered_object = None;
            self.active_drag = Some(DragTarget::CurveControl {
                object_index,
                point_index: hit.point_index,
            });
            return;
        }

        if let Some((object_index, point_index)) = find_curve_anchor_hit(document, x, y) {
            history.push(capture_canvas_snapshot(document));
            self.drag_snapshot_active = true;
            self.selected_object = Some(object_index);
            self.hovered_object = None;
            self.active_drag = Some(DragTarget::CurveAnchor {
                object_index,
                point_index,
            });
            return;
        }

        if let Some(object_index) = find_bounds_hit(document, x, y) {
            history.push(capture_canvas_snapshot(document));
            self.drag_snapshot_active = true;
            self.hovered_object = Some(object_index);
            self.active_drag = Some(DragTarget::Bounds { object_index });
            return;
        }

        self.hovered_object = None;
        self.active_drag = None;
    }

    pub fn pointer_dragged(
        &mut self,
        document: &mut CanvasDocument,
        x: f32,
        y: f32,
    ) -> Result<Option<(f32, f32)>> {
        let previous = self
            .last_pointer
            .replace((x, y))
            .ok_or_else(|| anyhow!("drag started without a prior pointer position"))?;
        let dx = x - previous.0;
        let dy = y - previous.1;

        match self.active_drag {
            Some(DragTarget::Pan) => Ok(Some((dx, dy))),
            Some(DragTarget::Bounds { object_index }) => {
                let object = document
                    .object_mut(object_index)
                    .ok_or_else(|| anyhow!("canvas object index out of range: {object_index}"))?;
                object.translate_all_points(dx, dy);
                self.hovered_object = Some(object_index);
                Ok(None)
            }
            Some(DragTarget::CurveAnchor {
                object_index,
                point_index,
            }) => {
                let object = document
                    .object_mut(object_index)
                    .ok_or_else(|| anyhow!("canvas object index out of range: {object_index}"))?;
                object.move_curve_anchor_with_neighbors(point_index, dx, dy)?;
                self.selected_object = Some(object_index);
                Ok(None)
            }
            Some(DragTarget::CurveControl {
                object_index,
                point_index,
            }) => {
                let object = document
                    .object_mut(object_index)
                    .ok_or_else(|| anyhow!("canvas object index out of range: {object_index}"))?;
                object.move_point(point_index, dx, dy)?;
                self.selected_object = Some(object_index);
                Ok(None)
            }
            None => Ok(None),
        }
    }

    pub fn pointer_released(&mut self) {
        self.active_drag = None;
        self.last_pointer = None;
        self.drag_snapshot_active = false;
    }

    pub fn clear_hover(&mut self) {
        self.hovered_object = None;
    }

    pub fn can_undo(&self, history: &CanvasHistory) -> bool {
        history.can_undo()
    }

    pub fn can_redo(&self, history: &CanvasHistory) -> bool {
        history.can_redo()
    }

    pub fn undo(&mut self, document: &mut CanvasDocument, history: &mut CanvasHistory) -> Result<()> {
        let current = capture_canvas_snapshot(document);
        let previous = history.undo(current)?;
        apply_canvas_snapshot(document, previous)
    }

    pub fn redo(&mut self, document: &mut CanvasDocument, history: &mut CanvasHistory) -> Result<()> {
        let current = capture_canvas_snapshot(document);
        let next = history.redo(current)?;
        apply_canvas_snapshot(document, next)
    }
}

fn find_curve_control_hit(
    document: &CanvasDocument,
    x: f32,
    y: f32,
) -> Option<(usize, CurveHandleHit)> {
    for (index, object) in document.objects.iter().enumerate() {
        if let Some(hit) = object.hit_curve_control(x, y) {
            return Some((index, hit));
        }
    }
    None
}

fn find_curve_anchor_hit(document: &CanvasDocument, x: f32, y: f32) -> Option<(usize, usize)> {
    for (index, object) in document.objects.iter().enumerate() {
        if let Some(point_index) = object.hit_curve_anchor(x, y) {
            return Some((index, point_index));
        }
    }
    None
}

fn find_bounds_hit(document: &CanvasDocument, x: f32, y: f32) -> Option<usize> {
    for (index, object) in document.objects.iter().enumerate() {
        if object.hit_bounds(x, y) {
            return Some(index);
        }
    }
    None
}

fn capture_canvas_snapshot(document: &CanvasDocument) -> CanvasEditSnapshot {
    CanvasEditSnapshot {
        objects: document.to_chunks(),
    }
}

fn apply_canvas_snapshot(document: &mut CanvasDocument, snapshot: CanvasEditSnapshot) -> Result<()> {
    document.load_chunks(&snapshot.objects)
}
