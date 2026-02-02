use std::collections::{HashMap, VecDeque};

use merdraw_layout::{LayoutEdge, LayoutGraph, LayoutNode, LayoutSubgraph};
use merdraw_parser::{Direction, EdgeArrow, EdgeStyle, Graph, NodeShape, Subgraph};
use skia_safe::{Font, Paint};

use crate::{build_text_paint, configure_font, load_font, SkiaRenderError, SkiaRenderOptions};

#[derive(Debug, Clone)]
pub struct SkiaLayoutOptions {
    pub node_padding_x: f32,
    pub node_padding_y: f32,
    pub node_gap: f32,
    pub layer_gap: f32,
    pub min_node_width: f32,
    pub min_node_height: f32,
}

impl Default for SkiaLayoutOptions {
    fn default() -> Self {
        Self {
            node_padding_x: 18.0,
            node_padding_y: 12.0,
            node_gap: 36.0,
            layer_gap: 64.0,
            min_node_width: 40.0,
            min_node_height: 24.0,
        }
    }
}

#[derive(Debug, Clone)]
struct WorkNode {
    id: String,
    label: Option<String>,
    width: f32,
    height: f32,
    layer: usize,
    order: usize,
    x: f32,
    y: f32,
    is_dummy: bool,
    shape: NodeShape,
}

#[derive(Debug, Clone)]
struct EdgeMeta {
    from: usize,
    to: usize,
    label: Option<String>,
    style: EdgeStyle,
    arrow: EdgeArrow,
    reversed: bool,
}

#[derive(Debug, Clone)]
struct UnitEdge {
    from: usize,
    to: usize,
}

pub fn layout_flowchart_skia(
    graph: &Graph,
    render_options: &SkiaRenderOptions,
    layout_options: &SkiaLayoutOptions,
) -> Result<LayoutGraph, SkiaRenderError> {
    if graph.nodes.is_empty() {
        return Ok(LayoutGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
            subgraphs: graph
                .subgraphs
                .iter()
                .map(build_layout_subgraph)
                .collect(),
            width: 0.0,
            height: 0.0,
        });
    }

    let mut font = load_font(render_options)?;
    configure_font(&mut font);
    let text_paint = build_text_paint();

    let min_width = layout_options
        .min_node_width
        .max(render_options.font_size * 2.5);
    let min_height = layout_options
        .min_node_height
        .max(render_options.font_size * 1.6);

    let mut nodes = Vec::new();
    let mut node_index = HashMap::new();
    for node in &graph.nodes {
        let label = node.label.as_deref().unwrap_or(node.id.as_str());
        let (width, height) = measure_node(
            label,
            &font,
            &text_paint,
            layout_options,
            min_width,
            min_height,
        );
        let idx = nodes.len();
        nodes.push(WorkNode {
            id: node.id.clone(),
            label: node.label.clone(),
            width,
            height,
            layer: 0,
            order: 0,
            x: 0.0,
            y: 0.0,
            is_dummy: false,
            shape: node.shape.clone(),
        });
        node_index.insert(node.id.clone(), idx);
    }

    let mut edges = Vec::new();
    for edge in &graph.edges {
        let from = *node_index
            .get(&edge.from)
            .expect("edge 'from' node missing");
        let to = *node_index
            .get(&edge.to)
            .expect("edge 'to' node missing");
        edges.push(EdgeMeta {
            from,
            to,
            label: edge.label.clone(),
            style: edge.style.clone(),
            arrow: edge.arrow.clone(),
            reversed: false,
        });
    }

    make_acyclic(&mut edges, nodes.len());
    assign_layers(&mut nodes, &edges);
    let unit_edges = insert_dummy_nodes(&mut nodes, &edges);
    let mut layers = build_layers(&mut nodes);
    reduce_crossings(&mut nodes, &mut layers, &unit_edges, 6);
    assign_coordinates(&mut nodes, &layers, layout_options, graph.direction.clone());

    let (width, height) = compute_graph_extent(&nodes);
    mirror_coordinates(&mut nodes, graph.direction.clone(), width, height);

    let mut layout_nodes = Vec::with_capacity(nodes.len());
    for node in nodes {
        layout_nodes.push(LayoutNode {
            id: node.id,
            label: node.label,
            width: node.width,
            height: node.height,
            layer: node.layer,
            order: node.order,
            x: node.x,
            y: node.y,
            is_dummy: node.is_dummy,
            shape: node.shape,
        });
    }

    let mut layout_edges = Vec::with_capacity(edges.len());
    for edge in &edges {
        let from = &layout_nodes[edge.from];
        let to = &layout_nodes[edge.to];
        let points = if edge.from == edge.to {
            route_self_loop(from, layout_options, graph.direction.clone())
        } else {
            route_edge_with_avoidance(
                from,
                to,
                &layout_nodes,
                layout_options,
            )
        };
        layout_edges.push(LayoutEdge {
            from: from.id.clone(),
            to: to.id.clone(),
            is_cross: false,
            label: edge.label.clone(),
            style: edge.style.clone(),
            arrow: edge.arrow.clone(),
            reversed: edge.reversed,
            points,
        });
    }

    let layout_subgraphs = graph
        .subgraphs
        .iter()
        .map(build_layout_subgraph)
        .collect();

    Ok(LayoutGraph {
        nodes: layout_nodes,
        edges: layout_edges,
        subgraphs: layout_subgraphs,
        width,
        height,
    })
}

fn measure_node(
    label: &str,
    font: &Font,
    text_paint: &Paint,
    options: &SkiaLayoutOptions,
    min_width: f32,
    min_height: f32,
) -> (f32, f32) {
    let (text_width, text_bounds) = font.measure_str(label, Some(text_paint));
    let width = (text_width + options.node_padding_x * 2.0).max(min_width);
    let height = (text_bounds.height() + options.node_padding_y * 2.0).max(min_height);
    (width, height)
}

fn make_acyclic(edges: &mut [EdgeMeta], node_count: usize) {
    let mut adjacency = vec![Vec::new(); node_count];
    for (idx, edge) in edges.iter().enumerate() {
        adjacency[edge.from].push(idx);
    }

    let mut state = vec![0u8; node_count];
    for node in 0..node_count {
        if state[node] == 0 {
            dfs_cycle_break(node, &adjacency, edges, &mut state);
        }
    }
}

fn dfs_cycle_break(
    node: usize,
    adjacency: &[Vec<usize>],
    edges: &mut [EdgeMeta],
    state: &mut [u8],
) {
    state[node] = 1;
    for &edge_idx in &adjacency[node] {
        let (from, to) = {
            let edge = &edges[edge_idx];
            (edge.from, edge.to)
        };
        if from != node {
            continue;
        }
        match state[to] {
            0 => dfs_cycle_break(to, adjacency, edges, state),
            1 => edges[edge_idx].reversed = true,
            _ => {}
        }
    }
    state[node] = 2;
}

fn assign_layers(nodes: &mut [WorkNode], edges: &[EdgeMeta]) {
    let node_count = nodes.len();
    let mut indegree = vec![0usize; node_count];
    let mut outgoing = vec![Vec::new(); node_count];

    for edge in edges {
        if edge.reversed {
            continue;
        }
        outgoing[edge.from].push(edge.to);
        indegree[edge.to] += 1;
    }

    let mut queue = VecDeque::new();
    for i in 0..node_count {
        if indegree[i] == 0 {
            queue.push_back(i);
        }
    }

    let mut order = Vec::with_capacity(node_count);
    while let Some(node) = queue.pop_front() {
        order.push(node);
        for &next in &outgoing[node] {
            indegree[next] -= 1;
            if indegree[next] == 0 {
                queue.push_back(next);
            }
        }
    }

    for &node in &order {
        let current = nodes[node].layer;
        for &next in &outgoing[node] {
            nodes[next].layer = nodes[next].layer.max(current + 1);
        }
    }
}

fn insert_dummy_nodes(nodes: &mut Vec<WorkNode>, edges: &[EdgeMeta]) -> Vec<UnitEdge> {
    let mut unit_edges = Vec::new();

    for edge in edges {
        let from_layer = nodes[edge.from].layer;
        let to_layer = nodes[edge.to].layer;
        if to_layer <= from_layer + 1 {
            unit_edges.push(UnitEdge {
                from: edge.from,
                to: edge.to,
            });
            continue;
        }

        let mut prev = edge.from;
        for layer in (from_layer + 1)..to_layer {
            let dummy_id = format!("__dummy{}", nodes.len());
            let dummy_idx = nodes.len();
            nodes.push(WorkNode {
                id: dummy_id,
                label: None,
                width: 1.0,
                height: 1.0,
                layer,
                order: 0,
                x: 0.0,
                y: 0.0,
                is_dummy: true,
                shape: NodeShape::Plain,
            });
            unit_edges.push(UnitEdge {
                from: prev,
                to: dummy_idx,
            });
            prev = dummy_idx;
        }
        unit_edges.push(UnitEdge {
            from: prev,
            to: edge.to,
        });
    }

    unit_edges
}

fn build_layers(nodes: &mut [WorkNode]) -> Vec<Vec<usize>> {
    let max_layer = nodes.iter().map(|node| node.layer).max().unwrap_or(0);
    let mut layers = vec![Vec::new(); max_layer + 1];
    for (idx, node) in nodes.iter().enumerate() {
        layers[node.layer].push(idx);
    }
    for layer in &mut layers {
        layer.sort();
        for (order, &node_idx) in layer.iter().enumerate() {
            nodes[node_idx].order = order;
        }
    }
    layers
}

fn reduce_crossings(
    nodes: &mut [WorkNode],
    layers: &mut [Vec<usize>],
    unit_edges: &[UnitEdge],
    passes: usize,
) {
    let mut down_neighbors = vec![Vec::new(); nodes.len()];
    let mut up_neighbors = vec![Vec::new(); nodes.len()];
    for edge in unit_edges {
        if nodes[edge.to].layer == nodes[edge.from].layer + 1 {
            down_neighbors[edge.from].push(edge.to);
            up_neighbors[edge.to].push(edge.from);
        }
    }

    let mut positions = vec![0usize; nodes.len()];
    for layer in layers.iter() {
        update_positions_for_layer(&mut positions, layer);
    }

    let edges_per_layer = build_edges_per_layer(unit_edges, nodes, layers.len());

    for pass in 0..passes {
        let downward = pass % 2 == 0;
        if downward {
            for layer in 1..layers.len() {
                reorder_layer(nodes, layers, layer, &up_neighbors);
                update_positions_for_layer(&mut positions, &layers[layer]);
                optimize_layer(
                    nodes,
                    layers,
                    layer,
                    &edges_per_layer,
                    &mut positions,
                );
            }
        } else {
            for layer in (0..layers.len().saturating_sub(1)).rev() {
                reorder_layer(nodes, layers, layer, &down_neighbors);
                update_positions_for_layer(&mut positions, &layers[layer]);
                optimize_layer(
                    nodes,
                    layers,
                    layer,
                    &edges_per_layer,
                    &mut positions,
                );
            }
        }
    }
}

fn reorder_layer(
    nodes: &mut [WorkNode],
    layers: &mut [Vec<usize>],
    layer_index: usize,
    neighbor_lists: &[Vec<usize>],
) {
    let layer = &layers[layer_index];
    let mut position = vec![0usize; nodes.len()];
    for (pos, &node_idx) in layer.iter().enumerate() {
        position[node_idx] = pos;
    }

    let mut scored: Vec<(usize, f32)> = layer
        .iter()
        .map(|&node_idx| {
            let neighbors = &neighbor_lists[node_idx];
            if neighbors.is_empty() {
                return (node_idx, position[node_idx] as f32);
            }
            let sum: usize = neighbors.iter().map(|&n| position[n]).sum();
            (node_idx, sum as f32 / neighbors.len() as f32)
        })
        .collect();

    scored.sort_by(|a, b| {
        a.1.partial_cmp(&b.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| position[a.0].cmp(&position[b.0]))
    });

    layers[layer_index] = scored.iter().map(|(idx, _)| *idx).collect();
    for (order, &node_idx) in layers[layer_index].iter().enumerate() {
        nodes[node_idx].order = order;
    }
}

fn update_positions_for_layer(positions: &mut [usize], layer: &[usize]) {
    for (pos, &node_idx) in layer.iter().enumerate() {
        positions[node_idx] = pos;
    }
}

fn build_edges_per_layer(
    unit_edges: &[UnitEdge],
    nodes: &[WorkNode],
    layer_count: usize,
) -> Vec<Vec<(usize, usize)>> {
    let mut edges = vec![Vec::new(); layer_count.saturating_sub(1)];
    for edge in unit_edges {
        let from_layer = nodes[edge.from].layer;
        let to_layer = nodes[edge.to].layer;
        if to_layer == from_layer + 1 {
            if let Some(bucket) = edges.get_mut(from_layer) {
                bucket.push((edge.from, edge.to));
            }
        }
    }
    edges
}

fn optimize_layer(
    nodes: &mut [WorkNode],
    layers: &mut [Vec<usize>],
    layer_index: usize,
    edges_per_layer: &[Vec<(usize, usize)>],
    positions: &mut [usize],
) {
    if layer_index >= layers.len() {
        return;
    }
    let layer_len = layers[layer_index].len();
    if layer_len < 2 {
        return;
    }

    let mut improved = true;
    let mut passes = 0;
    while improved && passes < 8 {
        improved = false;
        for i in 0..(layer_len - 1) {
            let before = crossings_for_layer(layer_index, edges_per_layer, positions);
            let a = layers[layer_index][i];
            let b = layers[layer_index][i + 1];
            layers[layer_index].swap(i, i + 1);
            positions[a] = i + 1;
            positions[b] = i;
            let after = crossings_for_layer(layer_index, edges_per_layer, positions);
            if after < before {
                improved = true;
            } else {
                layers[layer_index].swap(i, i + 1);
                positions[a] = i;
                positions[b] = i + 1;
            }
        }
        passes += 1;
    }

    for (order, &node_idx) in layers[layer_index].iter().enumerate() {
        nodes[node_idx].order = order;
    }
}

fn crossings_for_layer(
    layer_index: usize,
    edges_per_layer: &[Vec<(usize, usize)>],
    positions: &[usize],
) -> usize {
    let mut total = 0usize;
    if layer_index > 0 {
        if let Some(edges) = edges_per_layer.get(layer_index - 1) {
            total += count_crossings(edges, positions);
        }
    }
    if let Some(edges) = edges_per_layer.get(layer_index) {
        total += count_crossings(edges, positions);
    }
    total
}

fn count_crossings(edges: &[(usize, usize)], positions: &[usize]) -> usize {
    let mut count = 0usize;
    for i in 0..edges.len() {
        let (a_from, a_to) = edges[i];
        let a_from_pos = positions[a_from];
        let a_to_pos = positions[a_to];
        for j in (i + 1)..edges.len() {
            let (b_from, b_to) = edges[j];
            let b_from_pos = positions[b_from];
            let b_to_pos = positions[b_to];
            if (a_from_pos < b_from_pos && a_to_pos > b_to_pos)
                || (a_from_pos > b_from_pos && a_to_pos < b_to_pos)
            {
                count += 1;
            }
        }
    }
    count
}

fn assign_coordinates(
    nodes: &mut [WorkNode],
    layers: &[Vec<usize>],
    options: &SkiaLayoutOptions,
    direction: Direction,
) {
    match direction {
        Direction::TB | Direction::BT => assign_coordinates_tb(nodes, layers, options),
        Direction::LR | Direction::RL => assign_coordinates_lr(nodes, layers, options),
    }
}

fn assign_coordinates_tb(nodes: &mut [WorkNode], layers: &[Vec<usize>], options: &SkiaLayoutOptions) {
    let mut y = 0.0f32;
    for layer in layers {
        let mut layer_height = 0.0f32;
        for &node_idx in layer {
            layer_height = layer_height.max(nodes[node_idx].height);
        }
        let mut x = 0.0f32;
        for &node_idx in layer {
            let node = &mut nodes[node_idx];
            node.x = x + node.width / 2.0;
            node.y = y + layer_height / 2.0;
            x += node.width + options.node_gap;
        }
        y += layer_height + options.layer_gap;
    }
}

fn assign_coordinates_lr(nodes: &mut [WorkNode], layers: &[Vec<usize>], options: &SkiaLayoutOptions) {
    let mut x = 0.0f32;
    for layer in layers {
        let mut layer_width = 0.0f32;
        for &node_idx in layer {
            layer_width = layer_width.max(nodes[node_idx].width);
        }
        let mut y = 0.0f32;
        for &node_idx in layer {
            let node = &mut nodes[node_idx];
            node.x = x + layer_width / 2.0;
            node.y = y + node.height / 2.0;
            y += node.height + options.node_gap;
        }
        x += layer_width + options.layer_gap;
    }
}

fn compute_graph_extent(nodes: &[WorkNode]) -> (f32, f32) {
    let mut max_x = 0.0f32;
    let mut max_y = 0.0f32;
    for node in nodes {
        let right = node.x + node.width / 2.0;
        let bottom = node.y + node.height / 2.0;
        max_x = max_x.max(right);
        max_y = max_y.max(bottom);
    }
    (max_x, max_y)
}

fn mirror_coordinates(nodes: &mut [WorkNode], direction: Direction, width: f32, height: f32) {
    match direction {
        Direction::TB | Direction::LR => {}
        Direction::BT => {
            for node in nodes {
                node.y = height - node.y;
            }
        }
        Direction::RL => {
            for node in nodes {
                node.x = width - node.x;
            }
        }
    }
}

fn edge_boundary_point(from: &LayoutNode, to: &LayoutNode) -> (f32, f32) {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    if dx.abs() < 1e-3 && dy.abs() < 1e-3 {
        return (from.x, from.y);
    }
    let half_w = from.width / 2.0;
    let half_h = from.height / 2.0;
    let scale_x = if dx.abs() < 1e-6 {
        f32::INFINITY
    } else {
        half_w / dx.abs()
    };
    let scale_y = if dy.abs() < 1e-6 {
        f32::INFINITY
    } else {
        half_h / dy.abs()
    };
    let scale = scale_x.min(scale_y);
    (from.x + dx * scale, from.y + dy * scale)
}

#[derive(Debug, Clone, Copy)]
struct NodeRect {
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}

fn route_edge_with_avoidance(
    from: &LayoutNode,
    to: &LayoutNode,
    nodes: &[LayoutNode],
    options: &SkiaLayoutOptions,
) -> Vec<(f32, f32)> {
    let start = edge_boundary_point(from, to);
    let end = edge_boundary_point(to, from);

    let mut obstacles = Vec::new();
    for node in nodes {
        if node.is_dummy || node.id == from.id || node.id == to.id {
            continue;
        }
        obstacles.push(NodeRect {
            left: node.x - node.width / 2.0,
            right: node.x + node.width / 2.0,
            top: node.y - node.height / 2.0,
            bottom: node.y + node.height / 2.0,
        });
    }

    if obstacles.is_empty() || !path_hits_obstacles(&[start, end], &obstacles) {
        return vec![start, end];
    }

    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let ux = dx / len;
    let uy = dy / len;
    let mut px = -uy;
    let mut py = ux;
    if px.abs() < 1e-3 && py.abs() < 1e-3 {
        px = 1.0;
        py = 0.0;
    }

    let mid = ((start.0 + end.0) * 0.5, (start.1 + end.1) * 0.5);
    let offsets = [
        options.node_gap * 0.9,
        options.node_gap * 1.5,
        options.node_gap * 2.2,
    ];

    let mut best = vec![start, end];
    let mut best_score = path_score(&best, &obstacles);
    for offset in offsets {
        for sign in [-1.0, 1.0] {
            let candidate = (
                mid.0 + px * offset * sign,
                mid.1 + py * offset * sign,
            );
            let path = vec![start, candidate, end];
            let score = path_score(&path, &obstacles);
            if score < best_score {
                best_score = score;
                best = path;
            }
            if score == 0.0 {
                return best;
            }
        }
    }

    let offset = options.node_gap * 1.6;
    for sign in [-1.0, 1.0] {
        let p1 = (
            start.0 + dx * 0.33 + px * offset * sign,
            start.1 + dy * 0.33 + py * offset * sign,
        );
        let p2 = (
            start.0 + dx * 0.66 + px * offset * sign,
            start.1 + dy * 0.66 + py * offset * sign,
        );
        let path = vec![start, p1, p2, end];
        let score = path_score(&path, &obstacles);
        if score < best_score {
            best_score = score;
            best = path;
        }
        if score == 0.0 {
            return best;
        }
    }

    best
}

fn path_score(points: &[(f32, f32)], obstacles: &[NodeRect]) -> f32 {
    let intersections = path_intersections(points, obstacles) as f32;
    let length = path_length(points);
    intersections * 1000.0 + length
}

fn path_length(points: &[(f32, f32)]) -> f32 {
    let mut length = 0.0;
    for pair in points.windows(2) {
        let dx = pair[1].0 - pair[0].0;
        let dy = pair[1].1 - pair[0].1;
        length += (dx * dx + dy * dy).sqrt();
    }
    length
}

fn path_hits_obstacles(points: &[(f32, f32)], obstacles: &[NodeRect]) -> bool {
    for pair in points.windows(2) {
        for rect in obstacles {
            if segment_intersects_rect(pair[0], pair[1], *rect) {
                return true;
            }
        }
    }
    false
}

fn path_intersections(points: &[(f32, f32)], obstacles: &[NodeRect]) -> usize {
    let mut count = 0usize;
    for pair in points.windows(2) {
        for rect in obstacles {
            if segment_intersects_rect(pair[0], pair[1], *rect) {
                count += 1;
            }
        }
    }
    count
}

fn segment_intersects_rect(a: (f32, f32), b: (f32, f32), rect: NodeRect) -> bool {
    if point_in_rect(a, rect) || point_in_rect(b, rect) {
        return true;
    }
    let tl = (rect.left, rect.top);
    let tr = (rect.right, rect.top);
    let br = (rect.right, rect.bottom);
    let bl = (rect.left, rect.bottom);
    segments_intersect(a, b, tl, tr)
        || segments_intersect(a, b, tr, br)
        || segments_intersect(a, b, br, bl)
        || segments_intersect(a, b, bl, tl)
}

fn point_in_rect(p: (f32, f32), rect: NodeRect) -> bool {
    let eps = 0.01;
    p.0 >= rect.left - eps
        && p.0 <= rect.right + eps
        && p.1 >= rect.top - eps
        && p.1 <= rect.bottom + eps
}

fn segments_intersect(a: (f32, f32), b: (f32, f32), c: (f32, f32), d: (f32, f32)) -> bool {
    let o1 = orient(a, b, c);
    let o2 = orient(a, b, d);
    let o3 = orient(c, d, a);
    let o4 = orient(c, d, b);

    if o1 == 0.0 && on_segment(a, b, c) {
        return true;
    }
    if o2 == 0.0 && on_segment(a, b, d) {
        return true;
    }
    if o3 == 0.0 && on_segment(c, d, a) {
        return true;
    }
    if o4 == 0.0 && on_segment(c, d, b) {
        return true;
    }

    (o1 > 0.0) != (o2 > 0.0) && (o3 > 0.0) != (o4 > 0.0)
}

fn orient(a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> f32 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

fn on_segment(a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> bool {
    let min_x = a.0.min(b.0);
    let max_x = a.0.max(b.0);
    let min_y = a.1.min(b.1);
    let max_y = a.1.max(b.1);
    c.0 >= min_x - 0.01 && c.0 <= max_x + 0.01 && c.1 >= min_y - 0.01 && c.1 <= max_y + 0.01
}

fn route_self_loop(
    node: &LayoutNode,
    options: &SkiaLayoutOptions,
    direction: Direction,
) -> Vec<(f32, f32)> {
    let gap = options.node_gap.max(20.0);
    match direction {
        Direction::TB | Direction::BT => {
            let right = node.x + node.width / 2.0;
            let loop_w = gap * 0.8;
            let loop_h = gap * 0.6;
            vec![
                (right, node.y),
                (right + loop_w, node.y - loop_h),
                (right + loop_w, node.y - loop_h * 2.0),
                (right, node.y - loop_h * 2.0),
                (right, node.y),
            ]
        }
        Direction::LR | Direction::RL => {
            let bottom = node.y + node.height / 2.0;
            let loop_h = gap * 0.8;
            let loop_w = gap * 0.6;
            vec![
                (node.x, bottom),
                (node.x + loop_w, bottom + loop_h),
                (node.x + loop_w * 2.0, bottom + loop_h),
                (node.x + loop_w * 2.0, bottom),
                (node.x, bottom),
            ]
        }
    }
}

fn build_layout_subgraph(subgraph: &Subgraph) -> LayoutSubgraph {
    LayoutSubgraph {
        id: subgraph.id.clone(),
        title: subgraph.title.clone(),
        nodes: subgraph.nodes.clone(),
        subgraphs: subgraph
            .subgraphs
            .iter()
            .map(build_layout_subgraph)
            .collect(),
    }
}
