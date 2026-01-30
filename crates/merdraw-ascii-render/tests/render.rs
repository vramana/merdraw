use merdraw_ascii_render::{render_ascii, AsciiRenderOptions};
use merdraw_layout::{layout_flowchart, LayoutStyle};
use merdraw_parser::parse_flowchart;

#[test]
fn renders_basic_ascii() {
    let graph = parse_flowchart("flowchart TB\nA-->B-->C\n").expect("parse failed");
    let layout = layout_flowchart(&graph, &LayoutStyle::default());
    let output = render_ascii(&layout, &AsciiRenderOptions::default());
    assert!(output.contains("A"));
    assert!(output.contains("B"));
    assert!(output.contains("C"));
    assert!(output.contains("+"));
}
