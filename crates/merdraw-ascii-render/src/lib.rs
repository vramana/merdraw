use merdraw_layout::{LayoutGraph, LayoutNode};

#[derive(Debug, Clone)]
pub struct AsciiRenderOptions {
    pub max_width: usize,
    pub max_height: usize,
}

impl Default for AsciiRenderOptions {
    fn default() -> Self {
        Self {
            max_width: 120,
            max_height: 40,
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

    // Edges first so nodes appear on top.
    for edge in &layout.edges {
        for segment in edge.points.windows(2) {
            let (x1, y1) = map_point(segment[0], scale);
            let (x2, y2) = map_point(segment[1], scale);
            draw_line(&mut grid, x1, y1, x2, y2);
        }
    }

    for node in &layout.nodes {
        if node.is_dummy {
            continue;
        }
        draw_node(&mut grid, node, scale);
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

    set_cell(grid, left, top, '+');
    set_cell(grid, right, top, '+');
    set_cell(grid, left, bottom, '+');
    set_cell(grid, right, bottom, '+');

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
    set_cell(grid, x1, y2, '+');
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
    if existing == ' ' || existing == ch || existing == '-' || existing == '|' || existing == '+' {
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
        ('-', '|') | ('|', '-') => '+',
        ('+', _) | (_, '+') => '+',
        _ => incoming,
    }
}
