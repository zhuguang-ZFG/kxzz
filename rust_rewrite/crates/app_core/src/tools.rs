use crate::canvas::CanvasPathObject;
use anyhow::{anyhow, Result};
use font_core::{chunk_to_segments, segments_to_chunk, GlyphPathChunk, PathSegment};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    Select,
    Brush,
    Circle,
    Line,
    Polygon,
    Rectangle,
    Pen,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolSession {
    pub tool: ToolKind,
    preview: Option<CanvasPathObject>,
    state: ToolState,
}

#[derive(Debug, Clone, PartialEq)]
enum ToolState {
    Idle,
    Brush {
        last: CanvasPoint,
    },
    Shape {
        origin: CanvasPoint,
        polygon_sides: Option<usize>,
    },
    Line {
        points: Vec<CanvasPoint>,
    },
    Pen {
        segments: Vec<PenSegment>,
        current: Option<PenSegment>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, PartialEq)]
struct PenSegment {
    anchor: CanvasPoint,
    handle_in: Option<CanvasPoint>,
    handle_out: Option<CanvasPoint>,
}

impl ToolSession {
    pub fn new(tool: ToolKind) -> Self {
        let state = match tool {
            ToolKind::Line => ToolState::Line { points: Vec::new() },
            ToolKind::Pen => ToolState::Pen {
                segments: Vec::new(),
                current: None,
            },
            _ => ToolState::Idle,
        };

        Self {
            tool,
            preview: None,
            state,
        }
    }

    pub fn preview(&self) -> Option<&CanvasPathObject> {
        self.preview.as_ref()
    }

    pub fn pointer_pressed(
        &mut self,
        x: f32,
        y: f32,
        button: ToolPointerButton,
        polygon_sides: Option<usize>,
    ) -> Result<Option<CanvasPathObject>> {
        let point = CanvasPoint { x, y };
        match self.tool {
            ToolKind::Select => Ok(None),
            ToolKind::Brush => {
                if button != ToolPointerButton::Primary {
                    return Ok(None);
                }
                let preview = build_brush_seed(point)?;
                self.preview = Some(preview.clone());
                self.state = ToolState::Brush { last: point };
                Ok(Some(preview))
            }
            ToolKind::Circle | ToolKind::Rectangle | ToolKind::Polygon => {
                if button != ToolPointerButton::Primary {
                    return Ok(None);
                }
                let preview = CanvasPathObject::new();
                self.preview = Some(preview.clone());
                self.state = ToolState::Shape {
                    origin: point,
                    polygon_sides,
                };
                Ok(Some(preview))
            }
            ToolKind::Line => {
                match button {
                    ToolPointerButton::Primary => {
                        let ToolState::Line { points } = &mut self.state else {
                            return Err(anyhow!("line tool state mismatch"));
                        };
                        points.push(point);
                        let preview = build_polyline(points)?;
                        self.preview = Some(preview.clone());
                        Ok(Some(preview))
                    }
                    ToolPointerButton::Secondary => {
                        let committed = self.preview.take();
                        self.state = ToolState::Line { points: Vec::new() };
                        Ok(committed)
                    }
                    ToolPointerButton::Middle => Ok(None),
                }
            }
            ToolKind::Pen => self.pen_pressed(point, button),
        }
    }

    pub fn pointer_moved(&mut self, x: f32, y: f32, button_down: bool) -> Result<Option<&CanvasPathObject>> {
        let point = CanvasPoint { x, y };
        let tool = self.tool;
        match (tool, &mut self.state, &mut self.preview) {
            (ToolKind::Brush, ToolState::Brush { last }, Some(preview)) if button_down => {
                update_brush_preview(preview, *last, point)?;
                *last = point;
                Ok(Some(preview))
            }
            (ToolKind::Circle, ToolState::Shape { origin, .. }, Some(preview)) if button_down => {
                *preview = build_circle(*origin, point)?;
                Ok(Some(preview))
            }
            (ToolKind::Rectangle, ToolState::Shape { origin, .. }, Some(preview)) if button_down => {
                *preview = build_rectangle(*origin, point)?;
                Ok(Some(preview))
            }
            (ToolKind::Polygon, ToolState::Shape { origin, polygon_sides }, Some(preview)) if button_down => {
                let sides = polygon_sides.unwrap_or(6).max(3);
                *preview = build_polygon(*origin, point, sides)?;
                Ok(Some(preview))
            }
            (ToolKind::Line, ToolState::Line { points }, Some(preview)) => {
                if !points.is_empty() {
                    let mut temp = points.clone();
                    temp.push(point);
                    *preview = build_polyline(&temp)?;
                    return Ok(Some(preview));
                }
                Ok(None)
            }
            (ToolKind::Pen, ToolState::Pen { segments, current }, Some(preview)) => {
                if button_down {
                    if let Some(active) = current {
                        active.handle_out = Some(mirror_point(point, active.anchor));
                        active.handle_in = Some(point);
                        *preview = build_pen_path(segments, current.as_ref())?;
                        return Ok(Some(preview));
                    }
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    pub fn pointer_released(&mut self, x: f32, y: f32) -> Result<Option<CanvasPathObject>> {
        let point = CanvasPoint { x, y };
        match (&self.tool, &mut self.state, &mut self.preview) {
            (ToolKind::Brush, ToolState::Brush { .. }, preview) => {
                self.state = ToolState::Idle;
                Ok(preview.clone())
            }
            (ToolKind::Circle | ToolKind::Rectangle | ToolKind::Polygon, ToolState::Shape { .. }, preview) => {
                self.state = match self.tool {
                    ToolKind::Polygon => ToolState::Idle,
                    ToolKind::Circle | ToolKind::Rectangle => ToolState::Idle,
                    _ => ToolState::Idle,
                };
                Ok(preview.clone())
            }
            (ToolKind::Pen, ToolState::Pen { segments, current }, Some(preview)) => {
                if let Some(active) = current.take() {
                    let mut finalized = active.clone();
                    if finalized.handle_in.is_none() {
                        finalized.handle_in = Some(point);
                    }
                    segments.push(finalized);
                    *preview = build_pen_path(segments, None)?;
                    return Ok(None);
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    pub fn cancel(&mut self) {
        *self = Self::new(self.tool);
    }

    fn pen_pressed(
        &mut self,
        point: CanvasPoint,
        button: ToolPointerButton,
    ) -> Result<Option<CanvasPathObject>> {
        match button {
            ToolPointerButton::Primary => {
                let ToolState::Pen { segments, current } = &mut self.state else {
                    return Err(anyhow!("pen tool state mismatch"));
                };

                if current.is_some() {
                    return Ok(None);
                }

                let segment = PenSegment {
                    anchor: point,
                    handle_in: None,
                    handle_out: None,
                };
                *current = Some(segment);
                let preview = build_pen_path(segments, current.as_ref())?;
                self.preview = Some(preview.clone());
                Ok(Some(preview))
            }
            ToolPointerButton::Secondary => {
                let committed = self.preview.take();
                self.state = ToolState::Pen {
                    segments: Vec::new(),
                    current: None,
                };
                Ok(committed)
            }
            ToolPointerButton::Middle => Ok(None),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolPointerButton {
    Primary,
    Middle,
    Secondary,
}

fn build_brush_seed(point: CanvasPoint) -> Result<CanvasPathObject> {
    let chunk = segments_to_chunk(&[PathSegment::MoveTo {
        x: point.x,
        y: point.y,
    }]);
    CanvasPathObject::from_chunk(&chunk)
}

fn update_brush_preview(
    preview: &mut CanvasPathObject,
    last: CanvasPoint,
    current: CanvasPoint,
) -> Result<()> {
    let mut segments = preview.segments().to_vec();
    let dx = (current.x - last.x).abs();
    let dy = (current.y - last.y).abs();
    if dx < 5.0 && dy < 5.0 {
        segments.push(PathSegment::LineTo {
            x: current.x,
            y: current.y,
        });
    } else {
        segments.push(PathSegment::CurveTo {
            x1: last.x,
            y1: last.y,
            x2: last.x,
            y2: last.y,
            x3: (last.x + current.x) / 2.0,
            y3: (last.y + current.y) / 2.0,
        });
    }
    preview.set_segments(segments);
    Ok(())
}

fn build_circle(center: CanvasPoint, edge: CanvasPoint) -> Result<CanvasPathObject> {
    let radius = distance(center, edge);
    let steps = radius.max(1.0) as usize;
    let mut points = Vec::with_capacity(steps + 1);
    for step in 0..=steps {
        let angle = std::f32::consts::TAU * (step as f32) / (steps as f32);
        points.push(CanvasPoint {
            x: angle.cos() * radius + center.x,
            y: angle.sin() * radius + center.y,
        });
    }
    build_polyline(&points)
}

fn build_rectangle(start: CanvasPoint, end: CanvasPoint) -> Result<CanvasPathObject> {
    build_polyline(&[
        start,
        CanvasPoint { x: end.x, y: start.y },
        end,
        CanvasPoint { x: start.x, y: end.y },
        start,
    ])
}

fn build_polygon(center: CanvasPoint, edge: CanvasPoint, sides: usize) -> Result<CanvasPathObject> {
    let radius = distance(center, edge);
    let direction = CanvasPoint {
        x: edge.x - center.x,
        y: edge.y - center.y,
    };
    let mut angle = direction.y.atan2(direction.x);
    if angle < 0.0 {
        angle += std::f32::consts::TAU;
    }
    let step = std::f32::consts::TAU / sides as f32;
    let mut points = Vec::with_capacity(sides + 1);
    for i in 0..=sides {
        let a = angle + step * i as f32;
        points.push(CanvasPoint {
            x: a.cos() * radius + center.x,
            y: a.sin() * radius + center.y,
        });
    }
    build_polyline(&points)
}

fn build_polyline(points: &[CanvasPoint]) -> Result<CanvasPathObject> {
    let mut segments = Vec::new();
    for (index, point) in points.iter().copied().enumerate() {
        let segment = if index == 0 {
            PathSegment::MoveTo { x: point.x, y: point.y }
        } else {
            PathSegment::LineTo { x: point.x, y: point.y }
        };
        segments.push(segment);
    }
    let chunk = segments_to_chunk(&segments);
    CanvasPathObject::from_chunk(&chunk)
}

fn build_pen_path(segments: &[PenSegment], current: Option<&PenSegment>) -> Result<CanvasPathObject> {
    let mut path = Vec::new();
    let all: Vec<&PenSegment> = segments.iter().chain(current.into_iter()).collect();
    if let Some(first) = all.first() {
        path.push(PathSegment::MoveTo {
            x: first.anchor.x,
            y: first.anchor.y,
        });
    }
    for pair in all.windows(2) {
        let prev = pair[0];
        let next = pair[1];
        match (prev.handle_out, next.handle_in) {
            (Some(h1), Some(h2)) => path.push(PathSegment::CurveTo {
                x1: h1.x,
                y1: h1.y,
                x2: h2.x,
                y2: h2.y,
                x3: next.anchor.x,
                y3: next.anchor.y,
            }),
            _ => path.push(PathSegment::LineTo {
                x: next.anchor.x,
                y: next.anchor.y,
            }),
        }
    }
    let chunk = segments_to_chunk(&path);
    CanvasPathObject::from_chunk(&chunk)
}

fn mirror_point(point: CanvasPoint, center: CanvasPoint) -> CanvasPoint {
    CanvasPoint {
        x: center.x - (point.x - center.x),
        y: center.y - (point.y - center.y),
    }
}

fn distance(a: CanvasPoint, b: CanvasPoint) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx * dx + dy * dy).sqrt()
}

#[allow(dead_code)]
fn _chunk_from_preview(preview: &CanvasPathObject) -> Result<GlyphPathChunk> {
    let chunk = preview.to_chunk();
    let _ = chunk_to_segments(&chunk).map_err(|err| anyhow!(err.to_string()))?;
    Ok(chunk)
}
