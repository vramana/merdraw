use std::collections::HashMap;

use merdraw_layout::{LayoutGraph, LayoutNode};

#[derive(Debug, Clone)]
pub struct AsciiRenderOptions {
    pub max_width: usize,
    pub max_height: usize,
    pub show_arrows: bool,
}

impl Default for AsciiRenderOptions {
    fn default() -> Self {
        Self {
            max_width: 80,
            max_height: 30,
            show_arrows: true,
        }
    }
}

pub fn render_ascii(layout: &LayoutGraph, options: &AsciiRenderOptions) -> String {
    if layout.nodes.is_empty() {
        return String::new();
    }

    let width = layout.width.max(1.0);
    let height = layout.height.max(1.0);
    let scale_x = (width / options.max_width as f32).max(1.0);
    let scale_y = (height / options.max_height as f32).max(1.0);
    let scale = scale_x.max(scale_y);

    let grid_width = ((width / scale).ceil() as usize).max(1) + 2;
    let grid_height = ((height / scale).ceil() as usize).max(1) + 2;

    let mut grid = vec![vec![' '; grid_width]; grid_height];

    let bounds = build_bounds(&layout.nodes, scale);
    let mut edge_paths: Vec<Vec<(i32, i32)>> = Vec::new();

    // Edges first so nodes appear on top.
    for edge in &layout.edges {
        let mut points: Vec<(i32, i32)> = edge
            .points
            .iter()
            .map(|&point| map_point(point, scale))
            .collect();

        if points.len() >= 2 {
            if let Some(bound) = bounds.get(&edge.from) {
                let next = points[1];
                points[0] = clip_point(points[0], next, bound);
            }
            if let Some(bound) = bounds.get(&edge.to) {
                let last = points.len() - 1;
                let prev = points[last - 1];
                points[last] = clip_point(points[last], prev, bound);
            }
        }

        for segment in points.windows(2) {
            draw_line(&mut grid, segment[0].0, segment[0].1, segment[1].0, segment[1].1);
        }
        edge_paths.push(points);
    }

    for node in &layout.nodes {
        if node.is_dummy {
            continue;
        }
        draw_node(&mut grid, node, scale);
    }

    if options.show_arrows {
        for points in &edge_paths {
            draw_arrow(&mut grid, points);
        }
    }

    grid.into_iter()
        .map(|row| row.into_iter().collect::<String>().trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn map_point(point: (f32, f32), scale: f32) -> (i32, i32) {
    let x = (point.0 / scale).round() as i32;
    let y = (point.1 / scale).round() as i32;
    (x, y)
}

fn draw_node(grid: &mut [Vec<char>], node: &LayoutNode, scale: f32) {
    let (cx, cy) = map_point((node.x, node.y), scale);
    let label = node.id.as_str();
    let min_width = 3usize;
    let box_width = (label.chars().count() + 2).max(min_width) as i32;
    let box_height = 3i32;

    let left = cx - box_width / 2;
    let right = left + box_width - 1;
    let top = cy - box_height / 2;
    let bottom = top + box_height - 1;

    for x in left..=right {
        set_cell(grid, x, top, '-');
        set_cell(grid, x, bottom, '-');
    }
    for y in top..=bottom {
        set_cell(grid, left, y, '|');
        set_cell(grid, right, y, '|');
    }

    set_cell(grid, left, top, '-');
    set_cell(grid, right, top, '-');
    set_cell(grid, left, bottom, '-');
    set_cell(grid, right, bottom, '-');

    let available = (right - left - 1).max(0) as usize;
    if available > 0 {
        let mut text = label.to_string();
        if text.len() > available {
            text.truncate(available);
        }
        let start_x = left + 1 + ((available.saturating_sub(text.len())) / 2) as i32;
        let label_y = top + (bottom - top) / 2;
        for (idx, ch) in text.chars().enumerate() {
            set_cell(grid, start_x + idx as i32, label_y, ch);
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Bounds {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
}

fn build_bounds(nodes: &[LayoutNode], scale: f32) -> HashMap<String, Bounds> {
    let mut bounds = HashMap::new();
    for node in nodes {
        if node.is_dummy {
            continue;
        }
        let (cx, cy) = map_point((node.x, node.y), scale);
        let label = node.id.as_str();
        let min_width = 3usize;
        let box_width = (label.chars().count() + 2).max(min_width) as i32;
        let box_height = 3i32;

        let left = cx - box_width / 2;
        let right = left + box_width - 1;
        let top = cy - box_height / 2;
        let bottom = top + box_height - 1;

        bounds.insert(
            node.id.clone(),
            Bounds {
                left,
                right,
                top,
                bottom,
            },
        );
    }
    bounds
}

fn clip_point(point: (i32, i32), other: (i32, i32), bounds: &Bounds) -> (i32, i32) {
    let (mut x, mut y) = point;
    if x < bounds.left || x > bounds.right || y < bounds.top || y > bounds.bottom {
        return point;
    }

    let dx = (other.0 - x).signum();
    let dy = (other.1 - y).signum();
    if dx != 0 && dy == 0 {
        x = if dx > 0 { bounds.right } else { bounds.left };
    } else if dy != 0 && dx == 0 {
        y = if dy > 0 { bounds.bottom } else { bounds.top };
    }
    (x, y)
}

fn draw_line(grid: &mut [Vec<char>], x1: i32, y1: i32, x2: i32, y2: i32) {
    if x1 == x2 {
        let (start, end) = if y1 <= y2 { (y1, y2) } else { (y2, y1) };
        for y in start..=end {
            set_cell(grid, x1, y, '|');
        }
        return;
    }

    if y1 == y2 {
        let (start, end) = if x1 <= x2 { (x1, x2) } else { (x2, x1) };
        for x in start..=end {
            set_cell(grid, x, y1, '-');
        }
        return;
    }

    // Manhattan corner: draw two segments.
    draw_line(grid, x1, y1, x1, y2);
    draw_line(grid, x1, y2, x2, y2);
    set_cell(grid, x1, y2, '-');
}

fn set_cell(grid: &mut [Vec<char>], x: i32, y: i32, ch: char) {
    if y < 0 || x < 0 {
        return;
    }
    let y = y as usize;
    let x = x as usize;
    if y >= grid.len() || x >= grid[y].len() {
        return;
    }

    let existing = grid[y][x];
    if existing == ' ' || existing == ch || existing == '-' || existing == '|' {
        grid[y][x] = merge_char(existing, ch);
    }
}

fn merge_char(existing: char, incoming: char) -> char {
    if existing == ' ' {
        return incoming;
    }
    if existing == incoming {
        return existing;
    }
    match (existing, incoming) {
        ('-', '|') | ('|', '-') => '-',
        _ => incoming,
    }
}

fn draw_arrow(grid: &mut [Vec<char>], points: &[(i32, i32)]) {
    if points.len() < 2 {
        return;
    }
    let (x2, y2) = points[points.len() - 1];
    let (x1, y1) = points[points.len() - 2];
    let dx = (x2 - x1).signum();
    let dy = (y2 - y1).signum();
    if dx == 0 && dy == 0 {
        return;
    }
    let mut arrow_x = x2;
    let mut arrow_y = y2;
    if matches!(get_cell(grid, arrow_x, arrow_y), Some('-' | '|')) {
        arrow_x -= dx;
        arrow_y -= dy;
    }
    let ch = if dx > 0 {
        '>'
    } else if dx < 0 {
        '<'
    } else if dy > 0 {
        'v'
    } else {
        '^'
    };
    set_arrow_cell(grid, arrow_x, arrow_y, ch);
}

fn set_arrow_cell(grid: &mut [Vec<char>], x: i32, y: i32, ch: char) {
    if y < 0 || x < 0 {
        return;
    }
    let y = y as usize;
    let x = x as usize;
    if y >= grid.len() || x >= grid[y].len() {
        return;
    }
    grid[y][x] = ch;
}

fn get_cell(grid: &[Vec<char>], x: i32, y: i32) -> Option<char> {
    if y < 0 || x < 0 {
        return None;
    }
    let y = y as usize;
    let x = x as usize;
    if y >= grid.len() || x >= grid[y].len() {
        return None;
    }
    Some(grid[y][x])
}
