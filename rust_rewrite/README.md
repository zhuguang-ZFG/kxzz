# rust_rewrite

Rust workspace skeleton for the reversed main program.

Current crates:

- `font_core`: `.gfont` model, parser skeleton, path conversion helpers
- `app_core`: editor state and app-facing wrappers over `font_core`

Example shell:

- `cargo run -p app_core --example session_shell -- --tool rectangle`
- `cargo run -p app_core --example session_shell -- --font path\\to\\font.gfont --glyph A --tool pen`
- `cargo run -p app_core --example session_shell -- --script path\\to\\ops.txt`
- `cargo run -p app_core --example session_shell -- --script path\\to\\ops.txt --dump-json snapshot.json`
- `cargo run -p app_core --example session_shell -- --script path\\to\\ops.txt --dump-full-json display.json`
- `cargo run -p app_core --example session_shell -- --script path\\to\\ops.txt --save-font out.gfont`

Script lines:

- `tool pen`
- `glyph A`
- `finish_and_next`
- `press -30 -20 primary`
- `move 20 10 up`
- `move 35 5 down`
- `release 35 5`
- `dump`
- `dump_json snapshot.json`
- `dump_full_json display.json`
- `save_font out.gfont`

Notes:

- This is a scaffold based on reversed Java sources.
- Header crypto for `version >= 5` is intentionally stubbed for now.
- Rust tooling was not available in the current environment when this was created.
