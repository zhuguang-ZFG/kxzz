use crate::state::FontEditorState;
use anyhow::{anyhow, Result};
use font_core::GlyphPathChunk;
use native_algorithms::{
    polyline_length, translate_chunk, validate_raster_size, AutoTraceRequest, NativeAlgorithms,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceSelectionMode {
    ReplaceSelectedPath,
    AppendAsStyles,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedTrace {
    pub applied_count: usize,
    pub selected_path_index: usize,
}

pub fn trace_glyph_paths(
    native: &dyn NativeAlgorithms,
    argb_pixels: &[i32],
    request: &AutoTraceRequest,
) -> Result<Vec<GlyphPathChunk>> {
    validate_raster_size(request.width, request.height, argb_pixels.len())?;

    let threshold = match request.threshold {
        Some(value) => value,
        None => native.detect_threshold(argb_pixels)?,
    };

    let bitmap = native.binary_image(argb_pixels, threshold)?;
    validate_raster_size(request.width, request.height, bitmap.len())?;

    let skeleton = native.skeletonize(
        request.width,
        request.height,
        &bitmap,
        request.skeleton_limit,
    )?;
    validate_raster_size(request.width, request.height, skeleton.len())?;

    let mut paths = native.create_path(
        &skeleton,
        request.width,
        request.path_mode,
        request.path_merge,
        request.path_simplify,
    )?;

    paths.retain(|chunk| polyline_length(chunk) >= request.min_path_length);

    Ok(paths
        .into_iter()
        .map(|chunk| translate_chunk(&chunk, request.translate_x, request.translate_y))
        .collect())
}

pub fn trace_and_apply_to_selected_glyph(
    state: &mut FontEditorState,
    native: &dyn NativeAlgorithms,
    argb_pixels: &[i32],
    request: &AutoTraceRequest,
    mode: TraceSelectionMode,
) -> Result<AppliedTrace> {
    let traced = trace_glyph_paths(native, argb_pixels, request)?;
    if traced.is_empty() {
        return Err(anyhow!("native tracing returned no paths"));
    }

    match mode {
        TraceSelectionMode::ReplaceSelectedPath => apply_replace_mode(state, traced),
        TraceSelectionMode::AppendAsStyles => apply_append_mode(state, traced),
    }
}

fn apply_replace_mode(
    state: &mut FontEditorState,
    traced: Vec<GlyphPathChunk>,
) -> Result<AppliedTrace> {
    let mut iter = traced.into_iter();
    let first = iter
        .next()
        .ok_or_else(|| anyhow!("native tracing returned no paths"))?;
    state.replace_selected_path(first)?;

    let mut applied_count = 1usize;
    let selected_path_index = state
        .selected_path_index
        .ok_or_else(|| anyhow!("no path selected after trace"))?;

    for chunk in iter {
        if state.selected_path_count()? >= 20 {
            break;
        }
        state.append_path(chunk)?;
        applied_count += 1;
    }

    state.select_path(selected_path_index)?;
    Ok(AppliedTrace {
        applied_count,
        selected_path_index,
    })
}

fn apply_append_mode(
    state: &mut FontEditorState,
    traced: Vec<GlyphPathChunk>,
) -> Result<AppliedTrace> {
    let mut applied_count = 0usize;
    let mut first_index = None;

    for chunk in traced {
        if state.selected_path_count()? >= 20 {
            break;
        }
        let next_index = state.append_path(chunk)?;
        if first_index.is_none() {
            first_index = Some(next_index);
        }
        applied_count += 1;
    }

    let selected_path_index = first_index.ok_or_else(|| anyhow!("no trace path could be applied"))?;
    state.select_path(selected_path_index)?;
    Ok(AppliedTrace {
        applied_count,
        selected_path_index,
    })
}
