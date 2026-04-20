use crate::canvas::{CanvasDocument, CurveHandleHit};
use crate::history::{CanvasEditSnapshot, CanvasHistory};
use anyhow::{anyhow, Result};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PointerButton {
    Primary,
    Middle,
    Secondary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DragTarget {
    Pan,
    Bounds { object_index: usize },
    CurveAnchor { object_index: usize, point_index: usize },
    CurveControl { object_index: usize, point_index: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum HoverTarget {
    Bounds { object_index: usize },
    CurveAnchor { object_index: usize, point_index: usize },
    CurveControl { object_index: usize, point_index: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasInteractionState {
    pub active_drag: Option<DragTarget>,
    pub selected_object: Option<usize>,
    pub selected_target: Option<HoverTarget>,
    pub hovered_object: Option<usize>,
    pub hovered_target: Option<HoverTarget>,
    pub last_pointer: Option<(f32, f32)>,
    drag_snapshot_active: bool,
}

impl Default for CanvasInteractionState {
    fn default() -> Self {
        Self {
            active_drag: None,
            selected_object: None,
            selected_target: None,
            hovered_object: None,
            hovered_target: None,
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

        if let Some((object_index, hit)) = find_curve_control_hit(
            document,
            self.selected_object,
            self.selected_target,
            x,
            y,
        ) {
            history.push(capture_canvas_snapshot(document));
            self.drag_snapshot_active = true;
            self.selected_object = Some(object_index);
            self.selected_target = Some(HoverTarget::CurveAnchor {
                object_index,
                point_index: hit.linked_anchor_index,
            });
            self.hovered_object = Some(object_index);
            self.hovered_target = Some(HoverTarget::CurveControl {
                object_index,
                point_index: hit.point_index,
            });
            self.active_drag = Some(DragTarget::CurveControl {
                object_index,
                point_index: hit.point_index,
            });
            return;
        }

        if let Some((object_index, point_index)) =
            find_curve_anchor_hit(document, self.selected_object, self.selected_target, x, y)
        {
            history.push(capture_canvas_snapshot(document));
            self.drag_snapshot_active = true;
            self.selected_object = Some(object_index);
            self.selected_target = Some(HoverTarget::CurveAnchor {
                object_index,
                point_index,
            });
            self.hovered_object = Some(object_index);
            self.hovered_target = Some(HoverTarget::CurveAnchor {
                object_index,
                point_index,
            });
            self.active_drag = Some(DragTarget::CurveAnchor {
                object_index,
                point_index,
            });
            return;
        }

        if let Some(object_index) = find_bounds_hit(document, self.selected_object, x, y) {
            history.push(capture_canvas_snapshot(document));
            self.drag_snapshot_active = true;
            self.selected_object = Some(object_index);
            self.selected_target = Some(HoverTarget::Bounds { object_index });
            self.hovered_object = Some(object_index);
            self.hovered_target = Some(HoverTarget::Bounds { object_index });
            self.active_drag = Some(DragTarget::Bounds { object_index });
            return;
        }

        self.selected_object = None;
        self.selected_target = None;
        self.hovered_object = None;
        self.hovered_target = None;
        self.active_drag = None;
    }

    pub fn hover_at(&mut self, document: &CanvasDocument, x: f32, y: f32) -> bool {
        let preferred_target = self.hovered_target.or(self.selected_target);
        let preferred_object = preferred_target
            .map(hover_target_object_index)
            .or(self.selected_object);
        let next = if let Some((object_index, hit)) = find_curve_control_hit(
            document,
            preferred_object,
            preferred_target,
            x,
            y,
        )
        {
            Some(HoverTarget::CurveControl {
                object_index,
                point_index: hit.point_index,
            })
        } else if let Some((object_index, point_index)) =
            find_curve_anchor_hit(document, preferred_object, preferred_target, x, y)
        {
            Some(HoverTarget::CurveAnchor {
                object_index,
                point_index,
            })
        } else {
            find_bounds_hit(document, preferred_object, x, y)
                .map(|object_index| HoverTarget::Bounds { object_index })
        };

        let changed = self.hovered_target != next;
        self.hovered_target = next;
        self.hovered_object = match next {
            Some(HoverTarget::Bounds { object_index })
            | Some(HoverTarget::CurveAnchor { object_index, .. })
            | Some(HoverTarget::CurveControl { object_index, .. }) => Some(object_index),
            None => None,
        };
        changed
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
                self.selected_object = Some(object_index);
                self.selected_target = Some(HoverTarget::Bounds { object_index });
                self.hovered_object = Some(object_index);
                self.hovered_target = Some(HoverTarget::Bounds { object_index });
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
                self.selected_target = Some(HoverTarget::CurveAnchor {
                    object_index,
                    point_index,
                });
                self.hovered_object = Some(object_index);
                self.hovered_target = Some(HoverTarget::CurveAnchor {
                    object_index,
                    point_index,
                });
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
                self.hovered_object = Some(object_index);
                self.hovered_target = Some(HoverTarget::CurveControl {
                    object_index,
                    point_index,
                });
                Ok(None)
            }
            None => Ok(None),
        }
    }

    pub fn pointer_released(&mut self) {
        self.hovered_object = None;
        self.hovered_target = None;
        self.active_drag = None;
        self.last_pointer = None;
        self.drag_snapshot_active = false;
    }

    pub fn clear_hover(&mut self) {
        self.hovered_object = None;
        self.hovered_target = None;
    }

    pub fn delete_selected_object(
        &mut self,
        document: &mut CanvasDocument,
        history: &mut CanvasHistory,
    ) -> Result<bool> {
        let Some(object_index) = self.selected_object else {
            return Ok(false);
        };
        history.push(capture_canvas_snapshot(document));
        let _ = document.remove_object(object_index)?;
        self.selected_object = None;
        self.selected_target = None;
        self.hovered_object = None;
        self.hovered_target = None;
        self.active_drag = None;
        self.last_pointer = None;
        self.drag_snapshot_active = false;
        Ok(true)
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
        apply_canvas_snapshot(document, previous)?;
        self.clear_document_dependent_state();
        Ok(())
    }

    pub fn redo(&mut self, document: &mut CanvasDocument, history: &mut CanvasHistory) -> Result<()> {
        let current = capture_canvas_snapshot(document);
        let next = history.redo(current)?;
        apply_canvas_snapshot(document, next)?;
        self.clear_document_dependent_state();
        Ok(())
    }

    fn clear_document_dependent_state(&mut self) {
        self.active_drag = None;
        self.selected_object = None;
        self.selected_target = None;
        self.hovered_object = None;
        self.hovered_target = None;
        self.last_pointer = None;
        self.drag_snapshot_active = false;
    }
}

fn hover_target_object_index(target: HoverTarget) -> usize {
    match target {
        HoverTarget::Bounds { object_index }
        | HoverTarget::CurveAnchor { object_index, .. }
        | HoverTarget::CurveControl { object_index, .. } => object_index,
    }
}

fn find_curve_control_hit(
    document: &CanvasDocument,
    preferred_object: Option<usize>,
    preferred_target: Option<HoverTarget>,
    x: f32,
    y: f32,
) -> Option<(usize, CurveHandleHit)> {
    for index in prioritized_object_indices(document, preferred_object) {
        let object = &document.objects[index];
        let preferred_anchor = match preferred_target {
            Some(HoverTarget::CurveAnchor {
                object_index,
                point_index,
            }) if object_index == index => Some(point_index),
            _ => None,
        };
        if let Some(hit) = object.hit_curve_control_with_preferred_anchor(x, y, preferred_anchor) {
            return Some((index, hit));
        }
    }
    None
}

fn find_curve_anchor_hit(
    document: &CanvasDocument,
    preferred_object: Option<usize>,
    preferred_target: Option<HoverTarget>,
    x: f32,
    y: f32,
) -> Option<(usize, usize)> {
    for index in prioritized_object_indices(document, preferred_object) {
        let object = &document.objects[index];
        let preferred_anchor = match preferred_target {
            Some(HoverTarget::CurveAnchor {
                object_index,
                point_index,
            }) if object_index == index => Some(point_index),
            _ => None,
        };
        if let Some(point_index) = object.hit_curve_anchor_with_preferred_anchor(x, y, preferred_anchor) {
            return Some((index, point_index));
        }
    }
    None
}

fn find_bounds_hit(
    document: &CanvasDocument,
    preferred_object: Option<usize>,
    x: f32,
    y: f32,
) -> Option<usize> {
    for index in prioritized_object_indices(document, preferred_object) {
        let object = &document.objects[index];
        if object.hit_bounds(x, y) {
            return Some(index);
        }
    }
    None
}

fn prioritized_object_indices(
    document: &CanvasDocument,
    preferred_object: Option<usize>,
) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..document.objects.len()).collect();
    if let Some(preferred_index) = preferred_object.filter(|index| *index < document.objects.len()) {
        indices.retain(|index| *index != preferred_index);
        indices.insert(0, preferred_index);
    }
    indices
}

fn capture_canvas_snapshot(document: &CanvasDocument) -> CanvasEditSnapshot {
    CanvasEditSnapshot {
        objects: document.to_chunks(),
    }
}

fn apply_canvas_snapshot(document: &mut CanvasDocument, snapshot: CanvasEditSnapshot) -> Result<()> {
    document.load_chunks(&snapshot.objects)
}
