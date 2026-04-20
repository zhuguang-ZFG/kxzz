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
        nodes: Vec<PenNode>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, PartialEq)]
struct PenNode {
    start_anchor: CanvasPoint,
    end_anchor: Option<CanvasPoint>,
    control1: CanvasPoint,
    control2: Option<CanvasPoint>,
}

impl ToolSession {
    pub fn new(tool: ToolKind) -> Self {
        let state = match tool {
            ToolKind::Line => ToolState::Line { points: Vec::new() },
            ToolKind::Pen => ToolState::Pen { nodes: Vec::new() },
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
            (ToolKind::Pen, ToolState::Pen { nodes }, Some(preview)) => {
                if let Some(current) = nodes.last_mut() {
                    if button_down {
                        if let Some(end_anchor) = current.end_anchor {
                            current.control2 = Some(mirror_point(point, end_anchor));
                        }
                    } else {
                        current.end_anchor = Some(point);
                        current.control2 = Some(point);
                    }
                    *preview = build_pen_path(nodes)?;
                    return Ok(Some(preview));
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    pub fn pointer_released(&mut self, _x: f32, _y: f32) -> Result<Option<CanvasPathObject>> {
        match (&self.tool, &mut self.state, &mut self.preview) {
            (ToolKind::Brush, ToolState::Brush { .. }, preview) => {
                self.state = ToolState::Idle;
                if let Some(preview) = preview.as_ref() {
                    return Ok(Some(finalize_brush_preview(preview)?));
                }
                Ok(None)
            }
            (ToolKind::Circle | ToolKind::Rectangle | ToolKind::Polygon, ToolState::Shape { .. }, preview) => {
                self.state = match self.tool {
                    ToolKind::Polygon => ToolState::Idle,
                    ToolKind::Circle | ToolKind::Rectangle => ToolState::Idle,
                    _ => ToolState::Idle,
                };
                Ok(preview.clone())
            }
            (ToolKind::Pen, ToolState::Pen { nodes }, Some(preview)) => {
                let next_placeholder = nodes.last().and_then(|current| {
                    current.end_anchor.map(|end_anchor| {
                        let control2 = current.control2.unwrap_or(end_anchor);
                        PenNode {
                            start_anchor: end_anchor,
                            end_anchor: None,
                            control1: mirror_point(control2, end_anchor),
                            control2: None,
                        }
                    })
                });

                if let Some(next_placeholder) = next_placeholder {
                    nodes.push(next_placeholder);
                    *preview = build_pen_path(nodes)?;
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
                let ToolState::Pen { nodes } = &mut self.state else {
                    return Err(anyhow!("pen tool state mismatch"));
                };

                let created_preview = if nodes.is_empty() {
                    nodes.push(PenNode {
                        start_anchor: point,
                        end_anchor: None,
                        control1: point,
                        control2: None,
                    });
                    true
                } else if let Some(current) = nodes.last_mut() {
                    current.end_anchor = Some(point);
                    current.control2 = Some(point);
                    false
                } else {
                    false
                };

                let preview = build_pen_path(nodes)?;
                self.preview = Some(preview.clone());
                if created_preview {
                    Ok(Some(preview))
                } else {
                    Ok(None)
                }
            }
            ToolPointerButton::Secondary => {
                let ToolState::Pen { nodes } = &mut self.state else {
                    return Err(anyhow!("pen tool state mismatch"));
                };

                if matches!(nodes.last(), Some(node) if node.end_anchor.is_none()) {
                    nodes.pop();
                }

                let committed = if nodes.is_empty() {
                    None
                } else {
                    Some(build_pen_path(nodes)?)
                };

                self.preview = None;
                self.state = ToolState::Pen { nodes: Vec::new() };
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

fn finalize_brush_preview(preview: &CanvasPathObject) -> Result<CanvasPathObject> {
    let points = flatten_path_to_points(preview.segments());
    if points.len() <= 2 {
        return build_polyline(&points);
    }

    let simplified = simplify_polyline(&points, brush_simplify_tolerance());
    build_polyline(&simplified)
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

fn build_pen_path(nodes: &[PenNode]) -> Result<CanvasPathObject> {
    let mut path = Vec::new();
    if let Some(first) = nodes.first() {
        path.push(PathSegment::MoveTo {
            x: first.start_anchor.x,
            y: first.start_anchor.y,
        });
    }

    for node in nodes {
        if let (Some(end_anchor), Some(control2)) = (node.end_anchor, node.control2) {
            path.push(PathSegment::CurveTo {
                x1: node.control1.x,
                y1: node.control1.y,
                x2: control2.x,
                y2: control2.y,
                x3: end_anchor.x,
                y3: end_anchor.y,
            });
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

fn flatten_path_to_points(segments: &[PathSegment]) -> Vec<CanvasPoint> {
    let mut points = Vec::new();
    let mut current = None;

    for segment in segments {
        match *segment {
            PathSegment::MoveTo { x, y } => {
                let point = CanvasPoint { x, y };
                points.push(point);
                current = Some(point);
            }
            PathSegment::LineTo { x, y } => {
                let point = CanvasPoint { x, y };
                points.push(point);
                current = Some(point);
            }
            PathSegment::CurveTo { x1, y1, x2, y2, x3, y3 } => {
                if let Some(start) = current {
                    let control1 = CanvasPoint { x: x1, y: y1 };
                    let control2 = CanvasPoint { x: x2, y: y2 };
                    let end = CanvasPoint { x: x3, y: y3 };
                    let steps = cubic_sample_steps(start, control1, control2, end);
                    for step in 1..=steps {
                        let t = step as f32 / steps as f32;
                        points.push(sample_cubic_bezier(start, control1, control2, end, t));
                    }
                    current = Some(end);
                }
            }
            PathSegment::Close => {}
        }
    }

    dedupe_adjacent_points(points)
}

fn brush_simplify_tolerance() -> f32 {
    ((0.1f32 * 0.039_370_08f32 * 96.0f32) * 100.0f32).round() / 100.0f32
}

fn cubic_sample_steps(
    start: CanvasPoint,
    control1: CanvasPoint,
    control2: CanvasPoint,
    end: CanvasPoint,
) -> usize {
    let approx_length = distance(start, control1) + distance(control1, control2) + distance(control2, end);
    (approx_length * 3.0).ceil().max(1.0) as usize
}

fn sample_cubic_bezier(
    start: CanvasPoint,
    control1: CanvasPoint,
    control2: CanvasPoint,
    end: CanvasPoint,
    t: f32,
) -> CanvasPoint {
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    let t2 = t * t;
    CanvasPoint {
        x: mt2 * mt * start.x
            + 3.0 * mt2 * t * control1.x
            + 3.0 * mt * t2 * control2.x
            + t2 * t * end.x,
        y: mt2 * mt * start.y
            + 3.0 * mt2 * t * control1.y
            + 3.0 * mt * t2 * control2.y
            + t2 * t * end.y,
    }
}

fn simplify_polyline(points: &[CanvasPoint], epsilon: f32) -> Vec<CanvasPoint> {
    if points.len() <= 2 {
        return dedupe_adjacent_points(points.to_vec());
    }

    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    simplify_polyline_range(points, epsilon, 0, points.len() - 1, &mut keep);

    let simplified: Vec<CanvasPoint> = points
        .iter()
        .copied()
        .zip(keep)
        .filter_map(|(point, keep)| keep.then_some(point))
        .collect();
    dedupe_adjacent_points(simplified)
}

fn simplify_polyline_range(
    points: &[CanvasPoint],
    epsilon: f32,
    start: usize,
    end: usize,
    keep: &mut [bool],
) {
    if end <= start + 1 {
        return;
    }

    let start_point = points[start];
    let end_point = points[end];
    let mut max_distance = -1.0f32;
    let mut split_index = None;

    for (index, point) in points.iter().enumerate().take(end).skip(start + 1) {
        let distance = perpendicular_distance(*point, start_point, end_point);
        if distance > max_distance {
            max_distance = distance;
            split_index = Some(index);
        }
    }

    if max_distance > epsilon {
        if let Some(split_index) = split_index {
            keep[split_index] = true;
            simplify_polyline_range(points, epsilon, start, split_index, keep);
            simplify_polyline_range(points, epsilon, split_index, end, keep);
        }
    }
}

fn perpendicular_distance(point: CanvasPoint, line_start: CanvasPoint, line_end: CanvasPoint) -> f32 {
    let dx = line_end.x - line_start.x;
    let dy = line_end.y - line_start.y;
    if dx.abs() < f32::EPSILON && dy.abs() < f32::EPSILON {
        return distance(point, line_start);
    }

    let numerator =
        ((dy * point.x) - (dx * point.y) + (line_end.x * line_start.y) - (line_end.y * line_start.x)).abs();
    let denominator = (dx * dx + dy * dy).sqrt();
    numerator / denominator
}

fn dedupe_adjacent_points(points: Vec<CanvasPoint>) -> Vec<CanvasPoint> {
    let mut deduped = Vec::with_capacity(points.len());
    for point in points {
        let should_push = deduped
            .last()
            .map(|last| distance(*last, point) > f32::EPSILON)
            .unwrap_or(true);
        if should_push {
            deduped.push(point);
        }
    }
    deduped
}

#[allow(dead_code)]
fn _chunk_from_preview(preview: &CanvasPathObject) -> Result<GlyphPathChunk> {
    let chunk = preview.to_chunk();
    let _ = chunk_to_segments(&chunk).map_err(|err| anyhow!(err.to_string()))?;
    Ok(chunk)
}
