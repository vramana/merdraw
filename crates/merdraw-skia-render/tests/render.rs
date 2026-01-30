use merdraw_layout::{layout_flowchart, LayoutStyle};
use merdraw_parser::parse_flowchart;
use merdraw_skia_render::{render_to_bytes, ImageFormat, SkiaRenderOptions, SkiaRenderError};

#[test]
fn encodes_png() {
    let graph = parse_flowchart("flowchart TB\nA-->B\n").expect("parse failed");
    let layout = layout_flowchart(&graph, &LayoutStyle::default());
    let bytes = render_to_bytes(&layout, ImageFormat::Png, &SkiaRenderOptions::default())
        .expect("png render failed");
    assert!(bytes.starts_with(b"\x89PNG"));
}

#[test]
fn encodes_jpeg_or_reports_unsupported() {
    let graph = parse_flowchart("flowchart TB\nA-->B\n").expect("parse failed");
    let layout = layout_flowchart(&graph, &LayoutStyle::default());
    match render_to_bytes(&layout, ImageFormat::Jpeg { quality: 80 }, &SkiaRenderOptions::default()) {
        Ok(bytes) => assert!(bytes.starts_with(&[0xFF, 0xD8])),
        Err(SkiaRenderError::EncodeUnsupported(_)) => {}
        Err(err) => panic!("unexpected error: {:?}", err),
    }
}

#[test]
fn renders_labels_without_error() {
    let graph = parse_flowchart("flowchart TB\nA[Alpha]-->|Edge label|B[Beta]\n")
        .expect("parse failed");
    let layout = layout_flowchart(&graph, &LayoutStyle::default());
    let bytes = render_to_bytes(&layout, ImageFormat::Png, &SkiaRenderOptions::default())
        .expect("render failed");
    assert!(bytes.starts_with(b"\x89PNG"));
}
