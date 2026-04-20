use anyhow::{anyhow, Result};
use font_core::{
    chunk_to_segments, segments_to_chunk, GlyphPathChunk, PathSegment,
};

const HANDLE_HIT_RADIUS: f32 = 5.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasTransformConfig {
    pub canvas_size: f32,
    pub source_font_size: f32,
    pub baseline_offset_y: f32,
    pub editable_handles: bool,
}

impl CanvasTransformConfig {
    pub fn for_font_size(source_font_size: i32) -> Self {
        Self {
            canvas_size: 300.0,
            source_font_size: source_font_size.max(1) as f32,
            baseline_offset_y: 75.0,
            editable_handles: false,
        }
    }

    pub fn load_scale(self) -> f32 {
        self.canvas_size / self.source_font_size
    }

    pub fn save_scale(self) -> f32 {
        self.source_font_size / self.canvas_size
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RectF {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl RectF {
    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurveHandleRole {
    SegmentStartAnchor,
    SegmentEndAnchor,
    Control1,
    Control2,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CurveHandleHit {
    pub point_index: usize,
    pub linked_anchor_index: usize,
    pub role: CurveHandleRole,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CurveHandlePoint {
    pub point_index: usize,
    pub linked_anchor_index: usize,
    pub role: CurveHandleRole,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasPathObject {
    segments: Vec<PathSegment>,
    pub bounds: Option<RectF>,
    pub editable_handles: bool,
    original_for_scale: Option<Vec<PathSegment>>,
    scale_factor: f32,
}

impl CanvasPathObject {
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
            bounds: None,
            editable_handles: true,
            original_for_scale: None,
            scale_factor: 1.0,
        }
    }

    pub fn from_chunk(chunk: &GlyphPathChunk) -> Result<Self> {
        let mut object = Self::new();
        object.segments = chunk_to_segments(chunk).map_err(|err| anyhow!(err.to_string()))?;
        object.bounds = compute_bounds(&object.segments);
        Ok(object)
    }

    pub fn to_chunk(&self) -> GlyphPathChunk {
        segments_to_chunk(&self.segments)
    }

    pub fn segments(&self) -> &[PathSegment] {
        &self.segments
    }

    pub fn set_segments(&mut self, segments: Vec<PathSegment>) {
        self.segments = segments;
        self.bounds = compute_bounds(&self.segments);
        self.original_for_scale = None;
        self.scale_factor = 1.0;
    }

    pub fn clear(&mut self) {
        self.segments.clear();
        self.bounds = None;
        self.original_for_scale = None;
        self.scale_factor = 1.0;
    }

    pub fn translate(&mut self, dx: f32, dy: f32) {
        for segment in &mut self.segments {
            translate_segment(segment, dx, dy);
        }
        if let Some(bounds) = &mut self.bounds {
            bounds.left += dx;
            bounds.right += dx;
            bounds.top += dy;
            bounds.bottom += dy;
        }
        if let Some(original) = &mut self.original_for_scale {
            for segment in original {
                translate_segment(segment, dx, dy);
            }
        }
    }

    pub fn set_scale_delta(&mut self, delta: f32) {
        if self.original_for_scale.is_none() {
            self.original_for_scale = Some(self.segments.clone());
        }
        self.scale_factor += delta;

        if let Some(original) = &self.original_for_scale {
            self.segments = original
                .iter()
                .cloned()
                .map(|segment| scale_segment(segment, self.scale_factor))
                .collect();
            self.bounds = compute_bounds(&self.segments);
        }
    }

    pub fn set_editable_handles(&mut self, editable: bool) {
        self.editable_handles = editable;
    }

    pub fn hit_bounds(&self, x: f32, y: f32) -> bool {
        self.bounds
            .as_ref()
            .map(|bounds| bounds.contains(x, y))
            .unwrap_or(false)
    }

    pub fn hit_curve_anchor(&self, x: f32, y: f32) -> Option<usize> {
        if !self.editable_handles {
            return None;
        }

        let mut point_index = 0usize;
        for segment in &self.segments {
            match segment {
                PathSegment::MoveTo { .. } | PathSegment::LineTo { .. } => {
                    point_index += 1;
                }
                PathSegment::CurveTo { x3, y3, .. } => {
                    if distance(self.point_at(point_index + 2)?, (x, y)) <= HANDLE_HIT_RADIUS {
                        return Some(point_index + 2);
                    }
                    if point_index == 1 && distance((self.first_point()?), (x, y)) <= HANDLE_HIT_RADIUS {
                        return Some(0);
                    }
                    let _ = (x3, y3);
                    point_index += 3;
                }
                PathSegment::Close => {}
            }
        }

        None
    }

    pub fn hit_curve_anchor_drag(&self, x: f32, y: f32) -> bool {
        self.hit_curve_anchor(x, y).is_some()
    }

    pub fn hit_curve_control(&self, x: f32, y: f32) -> Option<CurveHandleHit> {
        if !self.editable_handles {
            return None;
        }

        let mut point_index = 0usize;
        for segment in &self.segments {
            match segment {
                PathSegment::MoveTo { .. } | PathSegment::LineTo { .. } => {
                    point_index += 1;
                }
                PathSegment::CurveTo { .. } => {
                    let control1 = self.point_at(point_index)?;
                    if distance(control1, (x, y)) <= HANDLE_HIT_RADIUS {
                        return Some(CurveHandleHit {
                            point_index,
                            linked_anchor_index: point_index.saturating_sub(1),
                            role: CurveHandleRole::Control1,
                        });
                    }

                    let control2 = self.point_at(point_index + 1)?;
                    if distance(control2, (x, y)) < HANDLE_HIT_RADIUS {
                        return Some(CurveHandleHit {
                            point_index: point_index + 1,
                            linked_anchor_index: point_index + 2,
                            role: CurveHandleRole::Control2,
                        });
                    }
                    point_index += 3;
                }
                PathSegment::Close => {}
            }
        }

        None
    }

    pub fn curve_handle_points(&self) -> Vec<CurveHandlePoint> {
        if !self.editable_handles {
            return Vec::new();
        }

        let mut handles = Vec::new();
        let mut point_index = 0usize;
        for segment in &self.segments {
            match *segment {
                PathSegment::MoveTo { x, y } | PathSegment::LineTo { x, y } => {
                    let role = if point_index == 0 {
                        CurveHandleRole::SegmentStartAnchor
                    } else {
                        CurveHandleRole::SegmentEndAnchor
                    };
                    handles.push(CurveHandlePoint {
                        point_index,
                        linked_anchor_index: point_index,
                        role,
                        x,
                        y,
                    });
                    point_index += 1;
                }
                PathSegment::CurveTo { x1, y1, x2, y2, x3, y3 } => {
                    handles.push(CurveHandlePoint {
                        point_index,
                        linked_anchor_index: point_index.saturating_sub(1),
                        role: CurveHandleRole::Control1,
                        x: x1,
                        y: y1,
                    });
                    handles.push(CurveHandlePoint {
                        point_index: point_index + 1,
                        linked_anchor_index: point_index + 2,
                        role: CurveHandleRole::Control2,
                        x: x2,
                        y: y2,
                    });
                    handles.push(CurveHandlePoint {
                        point_index: point_index + 2,
                        linked_anchor_index: point_index + 2,
                        role: CurveHandleRole::SegmentEndAnchor,
                        x: x3,
                        y: y3,
                    });
                    point_index += 3;
                }
                PathSegment::Close => {}
            }
        }

        handles
    }

    pub fn translate_all_points(&mut self, dx: f32, dy: f32) {
        self.translate(dx, dy);
    }

    pub fn move_point(&mut self, point_index: usize, dx: f32, dy: f32) -> Result<()> {
        let mut changed = false;
        let mut current_index = 0usize;
        for segment in &mut self.segments {
            match segment {
                PathSegment::MoveTo { x, y } | PathSegment::LineTo { x, y } => {
                    if current_index == point_index {
                        *x += dx;
                        *y += dy;
                        changed = true;
                        break;
                    }
                    current_index += 1;
                }
                PathSegment::CurveTo { x1, y1, x2, y2, x3, y3 } => {
                    if current_index == point_index {
                        *x1 += dx;
                        *y1 += dy;
                        changed = true;
                        break;
                    }
                    if current_index + 1 == point_index {
                        *x2 += dx;
                        *y2 += dy;
                        changed = true;
                        break;
                    }
                    if current_index + 2 == point_index {
                        *x3 += dx;
                        *y3 += dy;
                        changed = true;
                        break;
                    }
                    current_index += 3;
                }
                PathSegment::Close => {}
            }
        }

        if changed {
            self.after_geometry_changed();
            Ok(())
        } else {
            Err(anyhow!("point index out of range: {point_index}"))
        }
    }

    pub fn move_curve_anchor_with_neighbors(&mut self, anchor_index: usize, dx: f32, dy: f32) -> Result<()> {
        self.move_point(anchor_index, dx, dy)?;
        if anchor_index > 0 {
            let _ = self.move_point(anchor_index - 1, dx, dy);
        }
        let _ = self.move_point(anchor_index + 1, dx, dy);
        Ok(())
    }

    fn point_at(&self, point_index: usize) -> Option<(f32, f32)> {
        let mut current_index = 0usize;
        for segment in &self.segments {
            match *segment {
                PathSegment::MoveTo { x, y } | PathSegment::LineTo { x, y } => {
                    if current_index == point_index {
                        return Some((x, y));
                    }
                    current_index += 1;
                }
                PathSegment::CurveTo { x1, y1, x2, y2, x3, y3 } => {
                    let points = [(x1, y1), (x2, y2), (x3, y3)];
                    for (offset, point) in points.into_iter().enumerate() {
                        if current_index + offset == point_index {
                            return Some(point);
                        }
                    }
                    current_index += 3;
                }
                PathSegment::Close => {}
            }
        }
        None
    }

    fn first_point(&self) -> Option<(f32, f32)> {
        self.point_at(0)
    }

    fn after_geometry_changed(&mut self) {
        self.bounds = compute_bounds(&self.segments);
        self.original_for_scale = None;
        self.scale_factor = 1.0;
    }
}

impl Default for CanvasPathObject {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CanvasDocument {
    pub objects: Vec<CanvasPathObject>,
}

impl CanvasDocument {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.objects.clear();
    }

    pub fn undo_last_object(&mut self) -> Option<CanvasPathObject> {
        self.objects.pop()
    }

    pub fn load_chunks(&mut self, chunks: &[GlyphPathChunk]) -> Result<()> {
        self.objects = chunks
            .iter()
            .map(CanvasPathObject::from_chunk)
            .collect::<Result<Vec<_>>>()?;
        Ok(())
    }

    pub fn load_chunks_with_transform(
        &mut self,
        chunks: &[GlyphPathChunk],
        config: CanvasTransformConfig,
    ) -> Result<()> {
        self.objects = chunks
            .iter()
            .cloned()
            .map(|chunk| transform_chunk_for_canvas_load(chunk, config))
            .map(|chunk| {
                let mut object = CanvasPathObject::from_chunk(&chunk)?;
                object.set_editable_handles(config.editable_handles);
                Ok(object)
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(())
    }

    pub fn add_chunk(&mut self, chunk: GlyphPathChunk) -> Result<usize> {
        let object = CanvasPathObject::from_chunk(&chunk)?;
        self.objects.push(object);
        Ok(self.objects.len() - 1)
    }

    pub fn add_object(&mut self, object: CanvasPathObject) -> usize {
        self.objects.push(object);
        self.objects.len() - 1
    }

    pub fn replace_object(&mut self, index: usize, object: CanvasPathObject) -> Result<()> {
        let slot = self
            .objects
            .get_mut(index)
            .ok_or_else(|| anyhow!("canvas object index out of range: {index}"))?;
        *slot = object;
        Ok(())
    }

    pub fn object(&self, index: usize) -> Option<&CanvasPathObject> {
        self.objects.get(index)
    }

    pub fn object_mut(&mut self, index: usize) -> Option<&mut CanvasPathObject> {
        self.objects.get_mut(index)
    }

    pub fn to_chunks(&self) -> Vec<GlyphPathChunk> {
        self.objects
            .iter()
            .map(CanvasPathObject::to_chunk)
            .filter(|chunk| !chunk.points.is_empty())
            .collect()
    }

    pub fn to_chunks_with_transform(&self, config: CanvasTransformConfig) -> Vec<GlyphPathChunk> {
        self.to_chunks()
            .into_iter()
            .map(|chunk| transform_chunk_for_canvas_save(chunk, config))
            .filter(|chunk| !chunk.points.is_empty())
            .collect()
    }
}

fn compute_bounds(segments: &[PathSegment]) -> Option<RectF> {
    let mut left = f32::NAN;
    let mut top = f32::NAN;
    let mut right = f32::NAN;
    let mut bottom = f32::NAN;

    for segment in segments {
        match *segment {
            PathSegment::MoveTo { x, y } | PathSegment::LineTo { x, y } => {
                expand_bounds(&mut left, &mut top, &mut right, &mut bottom, x, y);
            }
            PathSegment::CurveTo { x1, y1, x2, y2, x3, y3 } => {
                expand_bounds(&mut left, &mut top, &mut right, &mut bottom, x1, y1);
                expand_bounds(&mut left, &mut top, &mut right, &mut bottom, x2, y2);
                expand_bounds(&mut left, &mut top, &mut right, &mut bottom, x3, y3);
            }
            PathSegment::Close => {}
        }
    }

    if left.is_nan() {
        None
    } else {
        Some(RectF {
            left,
            top,
            right,
            bottom,
        })
    }
}

fn expand_bounds(
    left: &mut f32,
    top: &mut f32,
    right: &mut f32,
    bottom: &mut f32,
    x: f32,
    y: f32,
) {
    if left.is_nan() || x < *left {
        *left = x;
    }
    if top.is_nan() || y < *top {
        *top = y;
    }
    if right.is_nan() || x > *right {
        *right = x;
    }
    if bottom.is_nan() || y > *bottom {
        *bottom = y;
    }
}

fn translate_segment(segment: &mut PathSegment, dx: f32, dy: f32) {
    match segment {
        PathSegment::MoveTo { x, y } | PathSegment::LineTo { x, y } => {
            *x += dx;
            *y += dy;
        }
        PathSegment::CurveTo { x1, y1, x2, y2, x3, y3 } => {
            *x1 += dx;
            *y1 += dy;
            *x2 += dx;
            *y2 += dy;
            *x3 += dx;
            *y3 += dy;
        }
        PathSegment::Close => {}
    }
}

fn scale_segment(segment: PathSegment, factor: f32) -> PathSegment {
    match segment {
        PathSegment::MoveTo { x, y } => PathSegment::MoveTo {
            x: x * factor,
            y: y * factor,
        },
        PathSegment::LineTo { x, y } => PathSegment::LineTo {
            x: x * factor,
            y: y * factor,
        },
        PathSegment::CurveTo { x1, y1, x2, y2, x3, y3 } => PathSegment::CurveTo {
            x1: x1 * factor,
            y1: y1 * factor,
            x2: x2 * factor,
            y2: y2 * factor,
            x3: x3 * factor,
            y3: y3 * factor,
        },
        PathSegment::Close => PathSegment::Close,
    }
}

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    (dx * dx + dy * dy).sqrt()
}

fn transform_chunk_for_canvas_load(
    mut chunk: GlyphPathChunk,
    config: CanvasTransformConfig,
) -> GlyphPathChunk {
    let scale = config.load_scale();
    for pair in chunk.points.chunks_exact_mut(2) {
        pair[0] *= scale;
        pair[1] *= scale;
        pair[1] += config.baseline_offset_y;
    }
    chunk
}

fn transform_chunk_for_canvas_save(
    mut chunk: GlyphPathChunk,
    config: CanvasTransformConfig,
) -> GlyphPathChunk {
    let scale = config.save_scale();
    for pair in chunk.points.chunks_exact_mut(2) {
        pair[1] -= config.baseline_offset_y;
        pair[0] *= scale;
        pair[1] *= scale;
    }
    chunk
}
