use std::fs;
use std::path::{Path, PathBuf};

use merdraw_layout::{LayoutEdge, LayoutGraph};
use skia_safe::{
    surfaces, Canvas, Color, EncodedImageFormat, Font, FontMgr, FontStyle, Paint, PaintStyle,
    PathBuilder, Point, FontHinting, font::Edging,
};

#[derive(Debug, Clone, Copy)]
pub struct SkiaColor(pub u8, pub u8, pub u8, pub u8);

#[derive(Debug, Clone)]
pub struct SkiaRenderOptions {
    pub width: u32,
    pub height: u32,
    pub background: SkiaColor,
    pub jpeg_quality: u8,
    pub padding: f32,
    pub stroke_width: f32,
    pub font_size: f32,
    pub font_path: Option<PathBuf>,
}

impl Default for SkiaRenderOptions {
    fn default() -> Self {
        Self {
            width: 1024,
            height: 768,
            background: SkiaColor(255, 255, 255, 255),
            jpeg_quality: 85,
            padding: 24.0,
            stroke_width: 2.0,
            font_size: 16.0,
            font_path: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ImageFormat {
    Png,
    Jpeg { quality: u8 },
}

#[derive(Debug)]
pub enum SkiaRenderError {
    EncodeUnsupported(&'static str),
    EncodeFailed(String),
    Io(std::io::Error),
    FontLoadFailed(String),
}

impl From<std::io::Error> for SkiaRenderError {
    fn from(err: std::io::Error) -> Self {
        SkiaRenderError::Io(err)
    }
}

pub fn render_to_bytes(
    layout: &LayoutGraph,
    format: ImageFormat,
    options: &SkiaRenderOptions,
) -> Result<Vec<u8>, SkiaRenderError> {
    let mut surface = surfaces::raster_n32_premul((options.width as i32, options.height as i32))
        .ok_or_else(|| SkiaRenderError::EncodeFailed("failed to create surface".to_string()))?;

    let canvas = surface.canvas();
    clear_canvas(canvas, options.background);

    let transform = compute_transform(layout, options);

    draw_edges(canvas, layout, &transform, options);
    draw_nodes(canvas, layout, &transform, options)?;

    let image = surface.image_snapshot();
    let (encoded, label) = match format {
        ImageFormat::Png => (image.encode(None, EncodedImageFormat::PNG, 100), "PNG"),
        ImageFormat::Jpeg { quality } => {
            let q = quality.clamp(0, 100) as u32;
            (image.encode(None, EncodedImageFormat::JPEG, q), "JPEG")
        }
    };

    let data = encoded.ok_or(SkiaRenderError::EncodeUnsupported(label))?;
    let bytes = data.as_bytes();
    Ok(bytes.to_vec())
}

pub fn render_to_file(
    layout: &LayoutGraph,
    format: ImageFormat,
    options: &SkiaRenderOptions,
    path: &Path,
) -> Result<(), SkiaRenderError> {
    let bytes = render_to_bytes(layout, format, options)?;
    fs::write(path, bytes)?;
    Ok(())
}

struct Transform {
    scale: f32,
    offset_x: f32,
    offset_y: f32,
}

fn compute_transform(layout: &LayoutGraph, options: &SkiaRenderOptions) -> Transform {
    let layout_width = layout.width.max(1.0);
    let layout_height = layout.height.max(1.0);
    let available_w = options.width as f32 - options.padding * 2.0;
    let available_h = options.height as f32 - options.padding * 2.0;

    let scale = (available_w / layout_width).min(available_h / layout_height).max(0.1);
    let offset_x = (options.width as f32 - layout_width * scale) / 2.0;
    let offset_y = (options.height as f32 - layout_height * scale) / 2.0;

    Transform {
        scale,
        offset_x,
        offset_y,
    }
}

fn transform_point(point: (f32, f32), transform: &Transform) -> Point {
    Point::new(
        point.0 * transform.scale + transform.offset_x,
        point.1 * transform.scale + transform.offset_y,
    )
}

fn clear_canvas(canvas: &Canvas, background: SkiaColor) {
    canvas.clear(Color::from_argb(background.3, background.0, background.1, background.2));
}

fn draw_nodes(
    canvas: &Canvas,
    layout: &LayoutGraph,
    transform: &Transform,
    options: &SkiaRenderOptions,
) -> Result<(), SkiaRenderError> {
    let mut stroke = Paint::default();
    stroke.set_style(PaintStyle::Stroke);
    stroke.set_color(Color::BLACK);
    stroke.set_stroke_width(options.stroke_width);

    let mut fill = Paint::default();
    fill.set_style(PaintStyle::Fill);
    fill.set_color(Color::WHITE);

    let mut text_paint = Paint::default();
    text_paint.set_color(Color::BLACK);
    text_paint.set_anti_alias(true);

    let mut font = if let Some(path) = options.font_path.as_ref() {
        let data = fs::read(path).map_err(|err| {
            SkiaRenderError::FontLoadFailed(format!("failed to read font {path:?}: {err}"))
        })?;
        let font_mgr = FontMgr::new();
        let typeface = font_mgr
            .new_from_data(&data, 0)
            .ok_or_else(|| SkiaRenderError::FontLoadFailed(format!("failed to load font {path:?}")))?;
        Font::from_typeface(typeface, options.font_size)
    } else {
        let mut font = Font::default();
        font.set_size(options.font_size);
        let font_mgr = FontMgr::new();
        let style = FontStyle::default();
        let candidates = ["SF Mono", "Menlo", "Monaco", "Courier New", "Courier"];
        let mut selected = None;
        for family in candidates {
            if let Some(typeface) = font_mgr.match_family_style(family, style) {
                selected = Some(typeface);
                break;
            }
        }
        if let Some(typeface) = selected {
            font.set_typeface(typeface);
        }
        font
    };

    font.set_edging(Edging::Alias);
    font.set_hinting(FontHinting::Full);
    font.set_subpixel(false);
    font.set_baseline_snap(true);
    font.set_force_auto_hinting(true);

    for node in &layout.nodes {
        if node.is_dummy {
            continue;
        }
        let center = transform_point((node.x, node.y), transform);
        let half_w = node.width * transform.scale / 2.0;
        let half_h = node.height * transform.scale / 2.0;
        let rect = skia_safe::Rect::from_xywh(
            center.x - half_w,
            center.y - half_h,
            half_w * 2.0,
            half_h * 2.0,
        );
        canvas.draw_rect(rect, &fill);
        canvas.draw_rect(rect, &stroke);

        let text = node.id.as_str();
        let (text_width, text_bounds) = font.measure_str(text, Some(&text_paint));
        let text_x = center.x - text_width / 2.0;
        let text_y = center.y + text_bounds.height() / 2.0;
        canvas.draw_str(text, (text_x, text_y), &font, &text_paint);
    }

    Ok(())
}

fn draw_edges(canvas: &Canvas, layout: &LayoutGraph, transform: &Transform, options: &SkiaRenderOptions) {
    let mut paint = Paint::default();
    paint.set_style(PaintStyle::Stroke);
    paint.set_color(Color::BLACK);
    paint.set_stroke_width(options.stroke_width);

    for edge in &layout.edges {
        draw_edge_path(canvas, edge, transform, &paint, options);
    }
}

fn draw_edge_path(
    canvas: &Canvas,
    edge: &LayoutEdge,
    transform: &Transform,
    paint: &Paint,
    _options: &SkiaRenderOptions,
) {
    if edge.points.is_empty() {
        return;
    }
    let mut builder = PathBuilder::new();
    let start = transform_point(edge.points[0], transform);
    builder.move_to(start);
    for point in &edge.points[1..] {
        let p = transform_point(*point, transform);
        builder.line_to(p);
    }
    let path = builder.detach();
    canvas.draw_path(&path, paint);

    draw_arrowhead(canvas, edge, transform, _options);
}

fn draw_arrowhead(canvas: &Canvas, edge: &LayoutEdge, transform: &Transform, options: &SkiaRenderOptions) {
    if edge.points.len() < 2 {
        return;
    }
    let end = transform_point(*edge.points.last().unwrap(), transform);
    let prev = transform_point(edge.points[edge.points.len() - 2], transform);
    let dir = Point::new(end.x - prev.x, end.y - prev.y);
    let len = (dir.x * dir.x + dir.y * dir.y).sqrt().max(1.0);
    let ux = dir.x / len;
    let uy = dir.y / len;
    let arrow_len = options.stroke_width * 6.0;
    let arrow_w = options.stroke_width * 3.0;

    let tip = end;
    let base = Point::new(end.x - ux * arrow_len, end.y - uy * arrow_len);
    let left = Point::new(base.x + -uy * arrow_w, base.y + ux * arrow_w);
    let right = Point::new(base.x + uy * arrow_w, base.y + -ux * arrow_w);

    let mut paint = Paint::default();
    paint.set_style(PaintStyle::Fill);
    paint.set_color(Color::BLACK);

    let mut builder = PathBuilder::new();
    builder.move_to(tip);
    builder.line_to(left);
    builder.line_to(right);
    builder.close();
    let path = builder.detach();
    canvas.draw_path(&path, &paint);
}
