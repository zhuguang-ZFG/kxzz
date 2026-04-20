use anyhow::{anyhow, Result};
use font_core::GlyphPathChunk;
use thiserror::Error;

const PATH_SPLIT_MARKER: f32 = -1.0;

#[derive(Debug, Clone)]
pub struct AutoTraceRequest {
    pub width: usize,
    pub height: usize,
    pub threshold: Option<i32>,
    pub skeleton_limit: i32,
    pub path_mode: i32,
    pub path_merge: i32,
    pub path_simplify: f32,
    pub min_path_length: f32,
    pub translate_x: f32,
    pub translate_y: f32,
}

impl Default for AutoTraceRequest {
    fn default() -> Self {
        Self {
            width: 300,
            height: 300,
            threshold: None,
            skeleton_limit: i32::MAX,
            path_mode: 1,
            path_merge: 5,
            path_simplify: 0.01,
            min_path_length: 0.0,
            translate_x: -150.0,
            translate_y: -150.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SquiggleRequest {
    pub width: usize,
    pub height: usize,
    pub density: f32,
    pub angle: i32,
    pub spacing: i32,
    pub line_width: i32,
    pub line_cap: i32,
    pub jitter: f32,
}

#[derive(Debug, Error)]
pub enum NativeAlgorithmsError {
    #[error("native algorithms backend is unavailable")]
    Unavailable,
    #[error("invalid raster buffer length: expected {expected}, got {actual}")]
    InvalidRasterSize { expected: usize, actual: usize },
    #[error("invalid native path stream: {0}")]
    InvalidPathStream(String),
}

pub trait NativeAlgorithms {
    fn simplify_polyline(&self, points: &[f32], epsilon: f32) -> Result<Vec<f32>>;

    fn detect_threshold(&self, argb_pixels: &[i32]) -> Result<i32>;

    fn binary_image(&self, argb_pixels: &[i32], threshold: i32) -> Result<Vec<u8>>;

    fn skeletonize(
        &self,
        width: usize,
        height: usize,
        bitmap: &[u8],
        max_iterations: i32,
    ) -> Result<Vec<u8>>;

    fn create_path(
        &self,
        bitmap: &[u8],
        width: usize,
        path_mode: i32,
        path_merge: i32,
        path_simplify: f32,
    ) -> Result<Vec<GlyphPathChunk>>;

    fn generate_squiggle(
        &self,
        argb_pixels: &[i32],
        request: &SquiggleRequest,
    ) -> Result<Vec<GlyphPathChunk>>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopNativeAlgorithms;

impl NativeAlgorithms for NoopNativeAlgorithms {
    fn simplify_polyline(&self, _points: &[f32], _epsilon: f32) -> Result<Vec<f32>> {
        Err(NativeAlgorithmsError::Unavailable.into())
    }

    fn detect_threshold(&self, _argb_pixels: &[i32]) -> Result<i32> {
        Err(NativeAlgorithmsError::Unavailable.into())
    }

    fn binary_image(&self, _argb_pixels: &[i32], _threshold: i32) -> Result<Vec<u8>> {
        Err(NativeAlgorithmsError::Unavailable.into())
    }

    fn skeletonize(
        &self,
        _width: usize,
        _height: usize,
        _bitmap: &[u8],
        _max_iterations: i32,
    ) -> Result<Vec<u8>> {
        Err(NativeAlgorithmsError::Unavailable.into())
    }

    fn create_path(
        &self,
        _bitmap: &[u8],
        _width: usize,
        _path_mode: i32,
        _path_merge: i32,
        _path_simplify: f32,
    ) -> Result<Vec<GlyphPathChunk>> {
        Err(NativeAlgorithmsError::Unavailable.into())
    }

    fn generate_squiggle(
        &self,
        _argb_pixels: &[i32],
        _request: &SquiggleRequest,
    ) -> Result<Vec<GlyphPathChunk>> {
        Err(NativeAlgorithmsError::Unavailable.into())
    }
}

pub fn native_path_to_chunks(stream: &[f32]) -> Result<Vec<GlyphPathChunk>> {
    let mut chunks = Vec::new();
    let mut start = 0usize;

    for (idx, value) in stream.iter().copied().enumerate() {
        if value != PATH_SPLIT_MARKER {
            continue;
        }
        let points = &stream[start..idx];
        if !points.is_empty() {
            chunks.push(polyline_to_chunk(points)?);
        }
        start = idx + 1;
    }

    if start < stream.len() {
        chunks.push(polyline_to_chunk(&stream[start..])?);
    }

    Ok(chunks)
}

pub fn translate_chunk(chunk: &GlyphPathChunk, dx: f32, dy: f32) -> GlyphPathChunk {
    let mut points = chunk.points.clone();
    for pair in points.chunks_exact_mut(2) {
        pair[0] += dx;
        pair[1] += dy;
    }
    GlyphPathChunk {
        points,
        verbs: chunk.verbs.clone(),
    }
}

pub fn polyline_length(chunk: &GlyphPathChunk) -> f32 {
    let mut length = 0.0f32;
    let mut iter = chunk.points.chunks_exact(2);
    let Some(mut prev) = iter.next() else {
        return 0.0;
    };

    for point in iter {
        let dx = point[0] - prev[0];
        let dy = point[1] - prev[1];
        length += (dx * dx + dy * dy).sqrt();
        prev = point;
    }

    length
}

fn polyline_to_chunk(points: &[f32]) -> Result<GlyphPathChunk> {
    if points.len() < 2 || points.len() % 2 != 0 {
        return Err(NativeAlgorithmsError::InvalidPathStream(format!(
            "polyline float count must be a positive even number, got {}",
            points.len()
        ))
        .into());
    }

    let mut verbs = vec![1u8; points.len() / 2];
    if let Some(first) = verbs.first_mut() {
        *first = 0;
    }

    Ok(GlyphPathChunk {
        points: points.to_vec(),
        verbs,
    })
}

#[allow(dead_code)]
pub fn validate_raster_size(width: usize, height: usize, actual_len: usize) -> Result<()> {
    let expected = width
        .checked_mul(height)
        .ok_or_else(|| anyhow!("raster dimensions overflow"))?;
    if expected != actual_len {
        return Err(NativeAlgorithmsError::InvalidRasterSize { expected, actual: actual_len }.into());
    }
    Ok(())
}
