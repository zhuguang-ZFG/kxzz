use anyhow::{anyhow, Result};
use app_core::{
    create_font, open_font, FontDraft, FontGlyphSession, PointerButton, ToolKind,
};
use font_core::FontKind;
use serde::Serialize;
use serde_json::to_string_pretty;
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();

    let mut font_path = None;
    let mut glyph = None;
    let mut script_path = None;
    let mut dump_json_path = None;
    let mut dump_full_json_path = None;
    let mut save_font_path = None;
    let mut tool = ToolKind::Select;

    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--font" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --font"))?;
                font_path = Some(PathBuf::from(value));
            }
            "--glyph" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --glyph"))?;
                glyph = Some(value.clone());
            }
            "--script" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --script"))?;
                script_path = Some(PathBuf::from(value));
            }
            "--dump-json" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --dump-json"))?;
                dump_json_path = Some(PathBuf::from(value));
            }
            "--dump-full-json" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --dump-full-json"))?;
                dump_full_json_path = Some(PathBuf::from(value));
            }
            "--save-font" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --save-font"))?;
                save_font_path = Some(PathBuf::from(value));
            }
            "--tool" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --tool"))?;
                tool = parse_tool(value)?;
            }
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            other => {
                return Err(anyhow!("unknown argument: {other}"));
            }
        }
        index += 1;
    }

    let font = match font_path {
        Some(path) => {
            println!("Opening font: {}", path.display());
            open_font(&path)?
        }
        None => {
            println!("No font provided, creating an in-memory demo font.");
            let mut font = create_font(FontDraft {
                name: "kxzz-demo".to_string(),
                kind: FontKind::Chinese2500,
                author: "rust_rewrite".to_string(),
                description: "Minimal shell demo".to_string(),
                password: None,
            });
            font.add_missing_tokens_from_text("demoA");
            font
        }
    };

    let mut session = FontGlyphSession::new(app_core::FontEditorState::new(font))?;
    if let Some(glyph) = glyph {
        session.select_glyph(&glyph)?;
    }
    session.set_tool(tool);

    println!();
    println!("Initial session state:");
    dump_session(&session);

    if let Some(script_path) = script_path {
        println!();
        println!("Running script: {}", script_path.display());
        run_script(&mut session, &script_path)?;
    } else {
        run_demo_interaction(&mut session)?;
    }

    println!();
    println!("After demo interaction:");
    dump_session(&session);

    if let Some(path) = dump_json_path {
        write_snapshot_json(&session, &path)?;
        println!("Wrote JSON snapshot: {}", path.display());
    }
    if let Some(path) = dump_full_json_path {
        write_full_display_json(&session, &path)?;
        println!("Wrote full display JSON: {}", path.display());
    }
    if let Some(path) = save_font_path {
        session.save_font_to(&path)?;
        println!("Saved font: {}", path.display());
    }

    Ok(())
}

fn run_demo_interaction(session: &mut FontGlyphSession) -> Result<()> {
    match session.canvas_state.active_tool.tool {
        ToolKind::Select => {
            let _ = session.pointer_moved(0.0, 0.0, false)?;
        }
        ToolKind::Rectangle => {
            let _ = session.pointer_pressed(-40.0, -40.0, PointerButton::Primary)?;
            let _ = session.pointer_moved(60.0, 80.0, true)?;
            let _ = session.pointer_released(60.0, 80.0)?;
        }
        ToolKind::Circle => {
            let _ = session.pointer_pressed(0.0, 0.0, PointerButton::Primary)?;
            let _ = session.pointer_moved(80.0, 40.0, true)?;
            let _ = session.pointer_released(80.0, 40.0)?;
        }
        ToolKind::Polygon => {
            session.set_polygon_sides(5);
            let _ = session.pointer_pressed(0.0, 0.0, PointerButton::Primary)?;
            let _ = session.pointer_moved(70.0, 20.0, true)?;
            let _ = session.pointer_released(70.0, 20.0)?;
        }
        ToolKind::Brush => {
            let _ = session.pointer_pressed(-60.0, -30.0, PointerButton::Primary)?;
            let _ = session.pointer_moved(-20.0, -10.0, true)?;
            let _ = session.pointer_moved(30.0, 20.0, true)?;
            let _ = session.pointer_released(30.0, 20.0)?;
        }
        ToolKind::Line => {
            let _ = session.pointer_pressed(-80.0, -40.0, PointerButton::Primary)?;
            let _ = session.pointer_moved(-10.0, 30.0, false)?;
            let _ = session.pointer_pressed(-10.0, 30.0, PointerButton::Primary)?;
            let _ = session.pointer_moved(50.0, 60.0, false)?;
            let _ = session.pointer_pressed(50.0, 60.0, PointerButton::Secondary)?;
        }
        ToolKind::Pen => {
            let _ = session.pointer_pressed(-30.0, -20.0, PointerButton::Primary)?;
            let _ = session.pointer_moved(20.0, 10.0, false)?;
            let _ = session.pointer_moved(35.0, 5.0, true)?;
            let _ = session.pointer_released(35.0, 5.0)?;
            let _ = session.pointer_moved(80.0, 20.0, false)?;
            let _ = session.pointer_pressed(80.0, 20.0, PointerButton::Primary)?;
            let _ = session.pointer_moved(100.0, 40.0, true)?;
            let _ = session.pointer_released(100.0, 40.0)?;
            let _ = session.pointer_pressed(100.0, 40.0, PointerButton::Secondary)?;
        }
    }

    Ok(())
}

fn dump_session(session: &FontGlyphSession) {
    let display = session.display_state();
    println!("selected glyph: {:?}", session.selected_glyph_key());
    println!("selected path index: {:?}", session.selected_path_index());
    println!("tool: {:?}", session.canvas_state.active_tool.tool);
    println!("document objects: {}", display.document.objects.len());
    println!("preview present: {}", display.preview.is_some());
    println!("selected object: {:?}", display.selected_object);
    println!("hovered object: {:?}", display.hovered_object);
    println!("hovered target: {:?}", display.hovered_target);
    println!("selected handles: {}", display.selected_handles.len());
    println!("selected guides: {}", display.selected_guides.len());
    println!(
        "hovered handle: {:?}",
        display.hovered_handle.as_ref().map(|handle| handle.point_index)
    );
    println!("hovered guides: {}", display.hovered_guides.len());
    println!("active drag: {:?}", display.active_drag);
}

fn write_snapshot_json(session: &FontGlyphSession, path: &PathBuf) -> Result<()> {
    let snapshot = SessionSnapshot::from_session(session);
    fs::write(path, to_string_pretty(&snapshot)?)?;
    Ok(())
}

fn write_full_display_json(session: &FontGlyphSession, path: &PathBuf) -> Result<()> {
    fs::write(path, to_string_pretty(&session.display_state())?)?;
    Ok(())
}

fn run_script(session: &mut FontGlyphSession, path: &PathBuf) -> Result<()> {
    let source = fs::read_to_string(path)?;
    for (line_no, raw_line) in source.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        run_script_line(session, line, line_no + 1)?;
    }
    Ok(())
}

fn run_script_line(session: &mut FontGlyphSession, line: &str, line_no: usize) -> Result<()> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    let command = parts
        .first()
        .copied()
        .ok_or_else(|| anyhow!("line {line_no}: empty command"))?;

    match command {
        "tool" => {
            let value = parts
                .get(1)
                .ok_or_else(|| anyhow!("line {line_no}: missing tool name"))?;
            session.set_tool(parse_tool(value)?);
        }
        "glyph" => {
            let value = parts
                .get(1)
                .ok_or_else(|| anyhow!("line {line_no}: missing glyph value"))?;
            session.select_glyph(value)?;
        }
        "next_glyph" => {
            let next = session.select_next_visible_glyph()?;
            println!("Script next_glyph @ line {line_no}: {:?}", next);
        }
        "prev_glyph" => {
            let previous = session.select_previous_visible_glyph()?;
            println!("Script prev_glyph @ line {line_no}: {:?}", previous);
        }
        "add_missing_chars" => {
            let text = line
                .strip_prefix(command)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| anyhow!("line {line_no}: missing text payload"))?;
            let added = session.add_missing_chars_from_text(text);
            println!(
                "Script add_missing_chars @ line {line_no}: added {}, selected {:?}",
                added,
                session.selected_glyph_key()
            );
        }
        "select_path" => {
            let value = parts
                .get(1)
                .ok_or_else(|| anyhow!("line {line_no}: missing path index"))?;
            let index: usize = value.parse()?;
            session.select_path(index)?;
            println!("Script select_path @ line {line_no}: {}", index);
        }
        "search" => {
            let value = parts
                .get(1)
                .ok_or_else(|| anyhow!("line {line_no}: missing search text"))?;
            session.search(value)?;
            println!("Script search @ line {line_no}: {:?}", session.selected_glyph_key());
        }
        "clear_search" => {
            session.clear_search()?;
            println!(
                "Script clear_search @ line {line_no}: {:?}",
                session.selected_glyph_key()
            );
        }
        "finish_and_next" => {
            let next = session.finish_selected_glyph_and_select_next_unfinished()?;
            println!("Script finish_and_next @ line {line_no}: {:?}", next);
        }
        "undo_canvas" => {
            let changed = session.undo_canvas()?;
            println!("Script undo_canvas @ line {line_no}: {}", changed);
        }
        "redo_canvas" => {
            let changed = session.redo_canvas()?;
            println!("Script redo_canvas @ line {line_no}: {}", changed);
        }
        "undo_path" => {
            let changed = session.undo_path_edit()?;
            println!("Script undo_path @ line {line_no}: {}", changed);
        }
        "redo_path" => {
            let changed = session.redo_path_edit()?;
            println!("Script redo_path @ line {line_no}: {}", changed);
        }
        "append_style_from_selected" => {
            let next_index = session.append_style_from_selected_path()?;
            println!(
                "Script append_style_from_selected @ line {line_no}: {}",
                next_index
            );
        }
        "clear_selected_path" => {
            session.clear_selected_path()?;
            println!("Script clear_selected_path @ line {line_no}");
        }
        "clear_all_paths" => {
            session.clear_all_paths()?;
            println!("Script clear_all_paths @ line {line_no}");
        }
        "delete_selected_object" => {
            let changed = session.delete_selected_canvas_object()?;
            println!("Script delete_selected_object @ line {line_no}: {}", changed);
        }
        "polygon_sides" => {
            let value = parts
                .get(1)
                .ok_or_else(|| anyhow!("line {line_no}: missing polygon side count"))?;
            session.set_polygon_sides(value.parse()?);
        }
        "press" => {
            let (x, y, button) = parse_xy_button(&parts, line_no)?;
            let _ = session.pointer_pressed(x, y, button)?;
        }
        "move" => {
            let x = parse_f32(parts.get(1), line_no, "x")?;
            let y = parse_f32(parts.get(2), line_no, "y")?;
            let button_down = parse_button_state(parts.get(3), line_no)?;
            let _ = session.pointer_moved(x, y, button_down)?;
        }
        "release" => {
            let x = parse_f32(parts.get(1), line_no, "x")?;
            let y = parse_f32(parts.get(2), line_no, "y")?;
            let _ = session.pointer_released(x, y)?;
        }
        "dump" => {
            println!();
            println!("Script dump @ line {line_no}:");
            dump_session(session);
        }
        "dump_json" => {
            let value = parts
                .get(1)
                .ok_or_else(|| anyhow!("line {line_no}: missing output path"))?;
            let path = PathBuf::from(value);
            write_snapshot_json(session, &path)?;
            println!("Script wrote JSON snapshot @ line {line_no}: {}", path.display());
        }
        "dump_full_json" => {
            let value = parts
                .get(1)
                .ok_or_else(|| anyhow!("line {line_no}: missing output path"))?;
            let path = PathBuf::from(value);
            write_full_display_json(session, &path)?;
            println!(
                "Script wrote full display JSON @ line {line_no}: {}",
                path.display()
            );
        }
        "save_font" => {
            let value = parts
                .get(1)
                .ok_or_else(|| anyhow!("line {line_no}: missing output path"))?;
            let path = PathBuf::from(value);
            session.save_font_to(&path)?;
            println!("Script saved font @ line {line_no}: {}", path.display());
        }
        other => {
            return Err(anyhow!("line {line_no}: unsupported command: {other}"));
        }
    }

    Ok(())
}

fn parse_tool(value: &str) -> Result<ToolKind> {
    match value.to_ascii_lowercase().as_str() {
        "select" => Ok(ToolKind::Select),
        "brush" => Ok(ToolKind::Brush),
        "circle" => Ok(ToolKind::Circle),
        "line" => Ok(ToolKind::Line),
        "polygon" => Ok(ToolKind::Polygon),
        "rectangle" => Ok(ToolKind::Rectangle),
        "pen" => Ok(ToolKind::Pen),
        _ => Err(anyhow!("unsupported tool: {value}")),
    }
}

fn parse_pointer_button(value: &str) -> Result<PointerButton> {
    match value.to_ascii_lowercase().as_str() {
        "primary" | "left" => Ok(PointerButton::Primary),
        "middle" => Ok(PointerButton::Middle),
        "secondary" | "right" => Ok(PointerButton::Secondary),
        _ => Err(anyhow!("unsupported pointer button: {value}")),
    }
}

fn parse_button_state(value: Option<&&str>, line_no: usize) -> Result<bool> {
    let value = value.ok_or_else(|| anyhow!("line {line_no}: missing move state"))?;
    match value.to_ascii_lowercase().as_str() {
        "down" | "true" => Ok(true),
        "up" | "false" => Ok(false),
        _ => Err(anyhow!("line {line_no}: unsupported move state: {value}")),
    }
}

fn parse_xy_button(parts: &[&str], line_no: usize) -> Result<(f32, f32, PointerButton)> {
    let x = parse_f32(parts.get(1), line_no, "x")?;
    let y = parse_f32(parts.get(2), line_no, "y")?;
    let button = parts
        .get(3)
        .ok_or_else(|| anyhow!("line {line_no}: missing pointer button"))?;
    Ok((x, y, parse_pointer_button(button)?))
}

fn parse_f32(value: Option<&&str>, line_no: usize, field: &str) -> Result<f32> {
    let value = value.ok_or_else(|| anyhow!("line {line_no}: missing {field}"))?;
    Ok(value.parse()?)
}

fn print_help() {
    println!("session_shell usage:");
    println!(
        "  cargo run -p app_core --example session_shell -- [--font PATH] [--glyph TEXT] [--tool NAME] [--script PATH] [--dump-json PATH] [--dump-full-json PATH] [--save-font PATH]"
    );
    println!("tools: select | brush | circle | line | polygon | rectangle | pen");
    println!(
        "script commands: tool | glyph | next_glyph | prev_glyph | add_missing_chars | select_path | search | clear_search | finish_and_next | undo_canvas | redo_canvas | undo_path | redo_path | append_style_from_selected | clear_selected_path | clear_all_paths | delete_selected_object | polygon_sides | press | move | release | dump | dump_json | dump_full_json | save_font"
    );
}

#[derive(Debug, Serialize)]
struct SessionSnapshot {
    selected_glyph: Option<String>,
    selected_path_index: Option<usize>,
    tool: String,
    document_object_count: usize,
    preview_present: bool,
    selected_object: Option<usize>,
    hovered_object: Option<usize>,
    hovered_target: Option<String>,
    selected_handle_count: usize,
    selected_guide_count: usize,
    hovered_handle_point_index: Option<usize>,
    hovered_guide_count: usize,
    active_drag: Option<String>,
}

impl SessionSnapshot {
    fn from_session(session: &FontGlyphSession) -> Self {
        let display = session.display_state();
        Self {
            selected_glyph: session.selected_glyph_key().map(ToOwned::to_owned),
            selected_path_index: session.selected_path_index(),
            tool: format!("{:?}", session.canvas_state.active_tool.tool),
            document_object_count: display.document.objects.len(),
            preview_present: display.preview.is_some(),
            selected_object: display.selected_object,
            hovered_object: display.hovered_object,
            hovered_target: display.hovered_target.map(|value| format!("{value:?}")),
            selected_handle_count: display.selected_handles.len(),
            selected_guide_count: display.selected_guides.len(),
            hovered_handle_point_index: display.hovered_handle.as_ref().map(|handle| handle.point_index),
            hovered_guide_count: display.hovered_guides.len(),
            active_drag: display.active_drag.map(|value| format!("{value:?}")),
        }
    }
}
