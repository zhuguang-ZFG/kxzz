use crate::error::FontCoreError;
use crate::model::{GlyphData, GlyphPathChunk};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathVerb {
    MoveTo,
    LineTo,
    CurveTo,
    Close,
    Unknown(u8),
}

#[derive(Debug, Clone, PartialEq)]
pub enum PathSegment {
    MoveTo { x: f32, y: f32 },
    LineTo { x: f32, y: f32 },
    CurveTo { x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32 },
    Close,
}

fn decode_verb(raw: u8) -> PathVerb {
    match raw {
        0 => PathVerb::MoveTo,
        1 => PathVerb::LineTo,
        2 => PathVerb::CurveTo,
        3 => PathVerb::Close,
        other => PathVerb::Unknown(other),
    }
}

pub fn chunk_to_segments(chunk: &GlyphPathChunk) -> Result<Vec<PathSegment>, FontCoreError> {
    let mut segments = Vec::new();
    let mut point_index = 0usize;

    for &raw in &chunk.verbs {
        match decode_verb(raw) {
            PathVerb::MoveTo | PathVerb::LineTo => {
                if point_index + 1 >= chunk.points.len() {
                    return Err(FontCoreError::InvalidPath("point array too short".into()));
                }
                let x = chunk.points[point_index];
                let y = chunk.points[point_index + 1];
                point_index += 2;
                if raw == 0 {
                    segments.push(PathSegment::MoveTo { x, y });
                } else {
                    segments.push(PathSegment::LineTo { x, y });
                }
            }
            PathVerb::CurveTo => {
                if point_index + 5 >= chunk.points.len() {
                    return Err(FontCoreError::InvalidPath("curve point array too short".into()));
                }
                segments.push(PathSegment::CurveTo {
                    x1: chunk.points[point_index],
                    y1: chunk.points[point_index + 1],
                    x2: chunk.points[point_index + 2],
                    y2: chunk.points[point_index + 3],
                    x3: chunk.points[point_index + 4],
                    y3: chunk.points[point_index + 5],
                });
                point_index += 6;
            }
            PathVerb::Close => segments.push(PathSegment::Close),
            PathVerb::Unknown(v) => {
                return Err(FontCoreError::InvalidPath(format!("unknown verb: {v}")));
            }
        }
    }

    Ok(segments)
}

pub fn segments_to_chunk(segments: &[PathSegment]) -> GlyphPathChunk {
    let mut points = Vec::new();
    let mut verbs = Vec::new();
    for segment in segments {
        match *segment {
            PathSegment::MoveTo { x, y } => {
                verbs.push(0);
                points.extend([x, y]);
            }
            PathSegment::LineTo { x, y } => {
                verbs.push(1);
                points.extend([x, y]);
            }
            PathSegment::CurveTo { x1, y1, x2, y2, x3, y3 } => {
                verbs.push(2);
                points.extend([x1, y1, x2, y2, x3, y3]);
            }
            PathSegment::Close => verbs.push(3),
        }
    }
    GlyphPathChunk { points, verbs }
}

pub fn glyph_to_segments(glyph: &GlyphData) -> Result<Vec<Vec<PathSegment>>, FontCoreError> {
    glyph.chunks.iter().map(chunk_to_segments).collect()
}

