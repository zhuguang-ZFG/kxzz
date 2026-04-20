# rust_rewrite

Rust workspace skeleton for the reversed `奎享字体` main program.

Current crates:

- `font_core`: `.gfont` model, parser skeleton, path conversion helpers
- `app_core`: editor state and app-facing wrappers over `font_core`

Example shell:

- `cargo run -p app_core --example session_shell -- --tool rectangle`
- `cargo run -p app_core --example session_shell -- --font path\\to\\font.gfont --glyph 我 --tool pen`
- `cargo run -p app_core --example session_shell -- --script path\\to\\ops.txt`

Script lines:

- `tool pen`
- `glyph 我`
- `press -30 -20 primary`
- `move 20 10 up`
- `move 35 5 down`
- `release 35 5`
- `dump`

Notes:

- This is a scaffold based on reversed Java sources.
- Header crypto for `version >= 5` is intentionally stubbed for now.
- Rust tooling was not available in the current environment when this was created.
