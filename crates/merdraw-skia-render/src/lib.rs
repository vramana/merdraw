use std::fs;
use std::path::{Path, PathBuf};

use merdraw_layout::{LayoutEdge, LayoutGraph, LayoutNode, LayoutSubgraph};
use skia_safe::{
    surfaces, Canvas, Color, EncodedImageFormat, Font, FontMgr, FontStyle, Paint, PaintStyle,
    PathBuilder, Point, FontHinting, font::Edging,
};

mod layout;

pub use layout::{layout_flowchart_skia, SkiaLayoutOptions};

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
    pub device_pixel_ratio: f32,
    pub debug: bool,
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
            device_pixel_ratio: 1.0,
            debug: false,
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
    let dpr = options.device_pixel_ratio.max(1.0);
    let surface_width = (options.width as f32 * dpr).ceil().max(1.0) as i32;
    let surface_height = (options.height as f32 * dpr).ceil().max(1.0) as i32;
    let mut surface = surfaces::raster_n32_premul((surface_width, surface_height))
        .ok_or_else(|| SkiaRenderError::EncodeFailed("failed to create surface".to_string()))?;

    let canvas = surface.canvas();
    clear_canvas(canvas, options.background);
    if dpr != 1.0 {
        canvas.scale((dpr, dpr));
    }

    let transform = compute_transform(layout, options);

    let mut font = load_font(options)?;
    configure_font(&mut font);
    if options.debug {
        let family = font.typeface().family_name();
        eprintln!(
            "skia font: {} (size {}, dpr {:.2})",
            family, options.font_size, dpr
        );
    }
    let text_paint = build_text_paint();

    let subgraph_rects = draw_subgraphs(canvas, layout, &transform, options, &font, &text_paint);
    draw_edges(
        canvas,
        layout,
        &transform,
        options,
        &font,
        &text_paint,
        &subgraph_rects,
    );
    draw_nodes(canvas, layout, &transform, options, &font, &text_paint)?;

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

pub(crate) fn load_font(options: &SkiaRenderOptions) -> Result<Font, SkiaRenderError> {
    if let Some(path) = options.font_path.as_ref() {
        let data = fs::read(path).map_err(|err| {
            SkiaRenderError::FontLoadFailed(format!("failed to read font {path:?}: {err}"))
        })?;
        let font_mgr = FontMgr::new();
        let typeface = font_mgr
            .new_from_data(&data, 0)
            .ok_or_else(|| SkiaRenderError::FontLoadFailed(format!("failed to load font {path:?}")))?;
        Ok(Font::from_typeface(typeface, options.font_size))
    } else {
        let mut font = Font::default();
        font.set_size(options.font_size);
        let font_mgr = FontMgr::new();
        let style = FontStyle::default();
        let candidates = ["SF Mono", "Menlo", "Monaco", "Courier New", "Courier"];
        for family in candidates {
            if let Some(typeface) = font_mgr.match_family_style(family, style) {
                font.set_typeface(typeface);
                break;
            }
        }
        Ok(font)
    }
}

pub(crate) fn configure_font(font: &mut Font) {
    font.set_edging(Edging::SubpixelAntiAlias);
    font.set_hinting(FontHinting::Full);
    font.set_subpixel(true);
    font.set_baseline_snap(true);
    font.set_force_auto_hinting(true);
}

pub(crate) fn build_text_paint() -> Paint {
    let mut paint = Paint::default();
    paint.set_color(Color::BLACK);
    paint.set_anti_alias(true);
    paint
}

fn snap_point(value: f32) -> f32 {
    value.round()
}

fn collect_node_rects(layout: &LayoutGraph, transform: &Transform) -> Vec<skia_safe::Rect> {
    let mut rects = Vec::new();
    for node in &layout.nodes {
        if node.is_dummy {
            continue;
        }
        let center = transform_point((node.x, node.y), transform);
        let half_w = node.width * transform.scale / 2.0;
        let half_h = node.height * transform.scale / 2.0;
        rects.push(skia_safe::Rect::from_xywh(
            center.x - half_w,
            center.y - half_h,
            half_w * 2.0,
            half_h * 2.0,
        ));
    }
    rects
}

fn normalize_point(point: Point) -> Point {
    let len = (point.x * point.x + point.y * point.y).sqrt();
    if len <= f32::EPSILON {
        return Point::new(0.0, -1.0);
    }
    Point::new(point.x / len, point.y / len)
}

fn centered_rect(center: Point, width: f32, height: f32) -> skia_safe::Rect {
    skia_safe::Rect::from_xywh(
        center.x - width / 2.0,
        center.y - height / 2.0,
        width,
        height,
    )
}

fn rects_intersect_any(rect: skia_safe::Rect, rects: &[skia_safe::Rect]) -> bool {
    rects.iter().any(|other| rects_overlap(rect, *other))
}

fn draw_subgraphs(
    canvas: &Canvas,
    layout: &LayoutGraph,
    transform: &Transform,
    options: &SkiaRenderOptions,
    font: &Font,
    text_paint: &Paint,
) -> Vec<SubgraphRect> {
    if layout.subgraphs.is_empty() {
        return Vec::new();
    }

    let mut node_map = std::collections::HashMap::new();
    for node in &layout.nodes {
        node_map.insert(node.id.as_str(), node);
    }

    let mut stroke = Paint::default();
    stroke.set_style(PaintStyle::Stroke);
    stroke.set_color(Color::from_argb(255, 90, 90, 90));
    stroke.set_stroke_width(1.5);

    let mut rects = Vec::new();
    let mut path = Vec::new();
    for subgraph in &layout.subgraphs {
        draw_subgraph(
            canvas,
            subgraph,
            &node_map,
            transform,
            options,
            font,
            text_paint,
            &stroke,
            &mut path,
            &mut rects,
        );
    }

    if options.debug {
        for entry in &rects {
            let rect = entry.rect;
            eprintln!(
                "subgraph: path={} label={} rect=({:.1},{:.1})-({:.1},{:.1}) size=({:.1},{:.1})",
                entry.path,
                entry.label,
                rect.left(),
                rect.top(),
                rect.right(),
                rect.bottom(),
                rect.width(),
                rect.height()
            );
        }
        log_subgraph_overlaps(&rects);
    }

    rects
}

fn draw_subgraph(
    canvas: &Canvas,
    subgraph: &LayoutSubgraph,
    node_map: &std::collections::HashMap<&str, &LayoutNode>,
    transform: &Transform,
    options: &SkiaRenderOptions,
    font: &Font,
    text_paint: &Paint,
    stroke: &Paint,
    path: &mut Vec<String>,
    rects: &mut Vec<SubgraphRect>,
) -> Option<skia_safe::Rect> {
    path.push(subgraph.id.clone());
    let mut rect: Option<skia_safe::Rect> = None;
    let mut has_content = false;

    for node_id in &subgraph.nodes {
        if let Some(node) = node_map.get(node_id.as_str()) {
            has_content = true;
            let center = transform_point((node.x, node.y), transform);
            let half_w = node.width * transform.scale / 2.0;
            let half_h = node.height * transform.scale / 2.0;
            let node_rect = skia_safe::Rect::from_xywh(
                center.x - half_w,
                center.y - half_h,
                half_w * 2.0,
                half_h * 2.0,
            );
            rect = Some(match rect {
                Some(existing) => union_rect(existing, node_rect),
                None => node_rect,
            });
        }
    }

    for child in &subgraph.subgraphs {
        if let Some(child_rect) = draw_subgraph(
            canvas,
            child,
            node_map,
            transform,
            options,
            font,
            text_paint,
            stroke,
            path,
            rects,
        ) {
            has_content = true;
            rect = Some(match rect {
                Some(existing) => union_rect(existing, child_rect),
                None => child_rect,
            });
        }
    }

    if !has_content {
        path.pop();
        return None;
    }

    let padding = (options.stroke_width * 4.0 + options.font_size).max(12.0);
    let mut rect = rect.unwrap();
    rect = skia_safe::Rect::from_xywh(
        rect.left() - padding,
        rect.top() - padding,
        rect.width() + padding * 2.0,
        rect.height() + padding * 2.0,
    );

    canvas.draw_rect(rect, stroke);

    let label = subgraph.title.as_deref().unwrap_or(subgraph.id.as_str());
    if !label.is_empty() {
        let (_text_width, text_bounds) = font.measure_str(label, Some(text_paint));
        let text_x = snap_point(rect.left() + padding);
        let text_y = snap_point(rect.top() + padding + text_bounds.height());
        canvas.draw_str(label, (text_x, text_y), font, text_paint);
    }

    rects.push(SubgraphRect {
        path: path.join("/"),
        label: label.to_string(),
        rect,
    });
    path.pop();
    Some(rect)
}

fn union_rect(a: skia_safe::Rect, b: skia_safe::Rect) -> skia_safe::Rect {
    skia_safe::Rect::from_ltrb(
        a.left().min(b.left()),
        a.top().min(b.top()),
        a.right().max(b.right()),
        a.bottom().max(b.bottom()),
    )
}

#[derive(Debug, Clone)]
struct SubgraphRect {
    path: String,
    label: String,
    rect: skia_safe::Rect,
}

fn log_subgraph_overlaps(rects: &[SubgraphRect]) {
    for i in 0..rects.len() {
        for j in (i + 1)..rects.len() {
            let a = &rects[i];
            let b = &rects[j];
            if is_ancestor(&a.path, &b.path) || is_ancestor(&b.path, &a.path) {
                continue;
            }
            if rects_overlap(a.rect, b.rect) {
                eprintln!(
                    "warning: subgraph overlap: {} <-> {}",
                    a.path, b.path
                );
            }
        }
    }
}

fn is_ancestor(parent: &str, child: &str) -> bool {
    if parent == child {
        return true;
    }
    if parent.is_empty() {
        return false;
    }
    let mut prefix = parent.to_string();
    prefix.push('/');
    child.starts_with(&prefix)
}

fn rects_overlap(a: skia_safe::Rect, b: skia_safe::Rect) -> bool {
    a.left() < b.right()
        && a.right() > b.left()
        && a.top() < b.bottom()
        && a.bottom() > b.top()
}

fn draw_nodes(
    canvas: &Canvas,
    layout: &LayoutGraph,
    transform: &Transform,
    options: &SkiaRenderOptions,
    font: &Font,
    text_paint: &Paint,
) -> Result<(), SkiaRenderError> {
    let mut stroke = Paint::default();
    stroke.set_style(PaintStyle::Stroke);
    stroke.set_color(Color::BLACK);
    stroke.set_stroke_width(options.stroke_width);

    let mut fill = Paint::default();
    fill.set_style(PaintStyle::Fill);
    fill.set_color(Color::WHITE);

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

        let text = node.label.as_deref().unwrap_or(node.id.as_str());
        let (text_width, text_bounds) = font.measure_str(text, Some(text_paint));
        let text_x = snap_point(center.x - text_width / 2.0);
        let text_y = snap_point(center.y + text_bounds.height() / 2.0);
        canvas.draw_str(text, (text_x, text_y), font, text_paint);
    }

    Ok(())
}

fn draw_edges(
    canvas: &Canvas,
    layout: &LayoutGraph,
    transform: &Transform,
    options: &SkiaRenderOptions,
    font: &Font,
    text_paint: &Paint,
    subgraph_rects: &[SubgraphRect],
) {
    let mut paint = Paint::default();
    paint.set_style(PaintStyle::Stroke);
    paint.set_color(Color::BLACK);
    paint.set_stroke_width(options.stroke_width);
    paint.set_anti_alias(true);
    paint.set_stroke_cap(skia_safe::paint::Cap::Round);
    paint.set_stroke_join(skia_safe::paint::Join::Round);

    for edge in &layout.edges {
        draw_edge_path(canvas, edge, transform, &paint, options);
    }

    let base_avoid = collect_node_rects(layout, transform);
    let mut placed = Vec::new();
    for edge in &layout.edges {
        let mut avoid_rects = base_avoid.clone();
        if edge.is_cross {
            for rect in subgraph_rects {
                avoid_rects.push(rect.rect);
            }
        }
        draw_edge_label(
            canvas,
            edge,
            transform,
            options,
            font,
            text_paint,
            &avoid_rects,
            &mut placed,
        );
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

fn draw_edge_label(
    canvas: &Canvas,
    edge: &LayoutEdge,
    transform: &Transform,
    options: &SkiaRenderOptions,
    font: &Font,
    text_paint: &Paint,
    avoid_rects: &[skia_safe::Rect],
    placed: &mut Vec<skia_safe::Rect>,
) {
    let label = match edge.label.as_deref() {
        Some(label) if !label.trim().is_empty() => label,
        _ => return,
    };

    if edge.points.len() < 2 {
        return;
    }

    let points: Vec<Point> = edge
        .points
        .iter()
        .map(|&point| transform_point(point, transform))
        .collect();
    let (text_width, text_bounds) = font.measure_str(label, Some(text_paint));
    let mut best_segment = None;
    let mut best_len = 0.0f32;
    let mut best_is_horizontal = false;
    for segment in points.windows(2) {
        let start = segment[0];
        let end = segment[1];
        let dx = (end.x - start.x).abs();
        let dy = (end.y - start.y).abs();
        let segment_len = segment_length(start, end);
        if segment_len <= f32::EPSILON {
            continue;
        }
        let is_horizontal = dy <= dx * 0.3;
        if edge.is_cross && is_horizontal && segment_len >= text_width + 8.0 {
            best_segment = Some((start, end));
            break;
        }
        if !edge.is_cross {
            let use_segment = if is_horizontal && !best_is_horizontal {
                true
            } else if is_horizontal == best_is_horizontal {
                segment_len > best_len
            } else {
                false
            };
            if use_segment {
                best_len = segment_len;
                best_is_horizontal = is_horizontal;
                best_segment = Some((start, end));
            }
        }
    }

    let (segment_start, segment_end, direction) = if let Some((start, end)) = best_segment {
        let dir = Point::new(end.x - start.x, end.y - start.y);
        (start, end, dir)
    } else {
        return;
    };

    let mut normal = Point::new(0.0, -1.0);
    if direction.x.abs() < direction.y.abs() {
        normal = Point::new(1.0, 0.0);
    }
    let offset = options.stroke_width * 4.0 + 6.0;
    let text_height = text_bounds.height().max(options.font_size);
    let normal = normalize_point(normal);
    let step = text_height + options.stroke_width * 2.0 + 4.0;
    let max_steps = 6;

    if edge.from == edge.to {
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        for point in &points {
            min_x = min_x.min(point.x);
            max_x = max_x.max(point.x);
            min_y = min_y.min(point.y);
        }
        let base_center = Point::new((min_x + max_x) / 2.0, min_y - offset);
        let mut chosen = None;
        for i in 0..=max_steps {
            let center = Point::new(base_center.x, base_center.y - i as f32 * step);
            let rect = centered_rect(center, text_width, text_height);
            if !rects_intersect_any(rect, avoid_rects) && !rects_intersect_any(rect, placed) {
                chosen = Some((center, rect));
                break;
            }
        }
        let (center, rect) =
            chosen.unwrap_or((base_center, centered_rect(base_center, text_width, text_height)));
        let mut label_bg = Paint::default();
        label_bg.set_style(PaintStyle::Fill);
        label_bg.set_color(Color::from_argb(
            options.background.3,
            options.background.0,
            options.background.1,
            options.background.2,
        ));
        let pad = 4.0;
        let bg_rect = skia_safe::Rect::from_xywh(
            rect.left() - pad,
            rect.top() - pad,
            rect.width() + pad * 2.0,
            rect.height() + pad * 2.0,
        );
        canvas.draw_rect(bg_rect, &label_bg);

        let text_x = snap_point(center.x - text_width / 2.0);
        let text_y = snap_point(center.y + text_height / 2.0);
        canvas.draw_str(label, (text_x, text_y), font, text_paint);
        placed.push(rect);
        return;
    }

    let mut candidate_centers = Vec::new();
    if edge.is_cross {
        let ts = [0.5f32, 0.35, 0.65, 0.2, 0.8];
        for t in ts {
            let base = Point::new(
                segment_start.x + (segment_end.x - segment_start.x) * t,
                segment_start.y + (segment_end.y - segment_start.y) * t,
            );
            candidate_centers.push(base);
        }
    } else {
        let base = Point::new(
            (segment_start.x + segment_end.x) / 2.0,
            (segment_start.y + segment_end.y) / 2.0,
        );
        candidate_centers.push(base);
    }

    let mut chosen = None;
    for base_center in &candidate_centers {
        let base_center = *base_center;
        let mut found = None;
        if edge.is_cross {
            for offset_idx in 0..=max_steps {
                let offset = offset + offset_idx as f32 * step;
                let center = Point::new(
                    base_center.x + normal.x * offset,
                    base_center.y + normal.y * offset,
                );
                let rect = centered_rect(center, text_width, text_height);
                if !rects_intersect_any(rect, avoid_rects) && !rects_intersect_any(rect, placed) {
                    found = Some((center, rect));
                    break;
                }
            }
        } else {
            for offset_idx in 0..=max_steps * 2 {
                let k = (offset_idx + 1) / 2;
                let sign = if offset_idx == 0 {
                    0.0
                } else if offset_idx % 2 == 1 {
                    1.0
                } else {
                    -1.0
                };
                let offset = sign * k as f32 * step + offset;
                let center = Point::new(
                    base_center.x + normal.x * offset,
                    base_center.y + normal.y * offset,
                );
                let rect = centered_rect(center, text_width, text_height);
                if !rects_intersect_any(rect, avoid_rects) && !rects_intersect_any(rect, placed) {
                    found = Some((center, rect));
                    break;
                }
            }
        }
        if found.is_some() {
            chosen = found;
            break;
        }
    }

    let fallback_center = candidate_centers
        .first()
        .copied()
        .unwrap_or(Point::new(
            (segment_start.x + segment_end.x) / 2.0,
            (segment_start.y + segment_end.y) / 2.0,
        ));
    let (center, rect) =
        chosen.unwrap_or((fallback_center, centered_rect(fallback_center, text_width, text_height)));
    let mut label_bg = Paint::default();
    label_bg.set_style(PaintStyle::Fill);
    label_bg.set_color(Color::from_argb(
        options.background.3,
        options.background.0,
        options.background.1,
        options.background.2,
    ));
    let pad = 4.0;
    let bg_rect = skia_safe::Rect::from_xywh(
        rect.left() - pad,
        rect.top() - pad,
        rect.width() + pad * 2.0,
        rect.height() + pad * 2.0,
    );
    canvas.draw_rect(bg_rect, &label_bg);

    let text_x = snap_point(center.x - text_width / 2.0);
    let text_y = snap_point(center.y + text_height / 2.0);
    canvas.draw_str(label, (text_x, text_y), font, text_paint);
    placed.push(rect);
}

fn segment_length(start: Point, end: Point) -> f32 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    (dx * dx + dy * dy).sqrt()
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
    paint.set_anti_alias(true);

    let mut builder = PathBuilder::new();
    builder.move_to(tip);
    builder.line_to(left);
    builder.line_to(right);
    builder.close();
    let path = builder.detach();
    canvas.draw_path(&path, &paint);
}
