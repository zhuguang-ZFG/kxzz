use anyhow::{anyhow, Result};
use app_core::{
    create_font, open_font, FontDraft, FontGlyphSession, PointerButton, ToolKind,
};
use font_core::FontKind;
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();

    let mut font_path = None;
    let mut glyph = None;
    let mut script_path = None;
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
            font.add_missing_tokens_from_text("测试A");
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
    println!("hovered handle: {:?}", display.hovered_handle.as_ref().map(|handle| handle.point_index));
    println!("hovered guides: {}", display.hovered_guides.len());
    println!("active drag: {:?}", display.active_drag);
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
            let value = parts.get(1).ok_or_else(|| anyhow!("line {line_no}: missing tool name"))?;
            session.set_tool(parse_tool(value)?);
        }
        "glyph" => {
            let value = parts.get(1).ok_or_else(|| anyhow!("line {line_no}: missing glyph value"))?;
            session.select_glyph(value)?;
        }
        "polygon_sides" => {
            let value = parts.get(1).ok_or_else(|| anyhow!("line {line_no}: missing polygon side count"))?;
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
    println!("  cargo run -p app_core --example session_shell -- [--font PATH] [--glyph TEXT] [--tool NAME] [--script PATH]");
    println!("tools: select | brush | circle | line | polygon | rectangle | pen");
    println!("script commands: tool | glyph | polygon_sides | press | move | release | dump");
}
