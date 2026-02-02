# AGENTS

Project: merdraw is a Rust workspace for parsing Mermaid-style flowcharts and rendering to ASCII or images.

## Workspace layout
- `crates/merdraw-parser`: lexer, AST, and `parse_flowchart` for a Mermaid flowchart subset.
- `crates/merdraw-layout`: layered layout + subgraph grouping; exposes `LayoutGraph`, `LayoutStyle`, `suggest_canvas_size`, `subgraph_bounds`.
- `crates/merdraw-ascii-render`: ASCII renderer for `LayoutGraph`.
- `crates/merdraw-skia-render`: Skia-based PNG/JPEG renderer.
- `crates/merdraw`: CLI that wires parser + layout + renderers.
- `crates/merdraw-preview`: tiny HTTP server that renders random flowcharts via the CLI.
- `examples/`: sample `.mmd` files.
- `tmp/`: preview images written here.

## Supported Mermaid flowchart subset
- Header: `flowchart` or `graph`, directions TB/TD/BT/LR/RL.
- Nodes: plain ids, labels with `[bracket]`, `(round)`, `((circle))`, `{diamond}`, `{{hex}}`.
- Edge operators: `-->`, `---`, `-.->`, `-.-`, `==>`, `===`.
- Edge labels: `A -->|label| B`.
- Quoted node ids: `"Node A"`.
- Subgraphs: `subgraph id "Title"` ... `end` (nested supported).
- Comments: `%%` to end of line.

## CLI usage (crates/merdraw)
- ASCII (default): `cargo run -p merdraw -- <file>.mmd` or stdin with `-`.
- Image output: `cargo run -p merdraw -- <file>.mmd --out out.png` (format inferred from extension).
- Options: `--format png|jpg|jpeg`, `--width`, `--height`, `--quality`, `--font <path>`, `--dpr <float>`, `--debug`.

Note: ASCII mode uses a tighter `LayoutStyle` in `crates/merdraw/src/main.rs`; image output uses `LayoutStyle::default()`.

## Preview server
- `cargo run -p merdraw-preview`
- Serves at `http://127.0.0.1:7878`, renders random flowcharts to `tmp/preview_*.png` using the CLI.

## Tests
- All tests: `cargo test`
- Per crate: `cargo test -p merdraw-parser` (etc.).
