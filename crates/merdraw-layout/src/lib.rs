use std::collections::{HashMap, VecDeque};

use merdraw_parser::{
    Direction, EdgeArrow, EdgeStyle, Graph, Node as ParsedNode, NodeShape, Subgraph,
};

#[derive(Debug, Clone)]
pub struct LayoutStyle {
    pub min_width: f32,
    pub min_height: f32,
    pub char_width: f32,
    pub char_height: f32,
    pub node_padding_x: f32,
    pub node_padding_y: f32,
    pub node_gap: f32,
    pub layer_gap: f32,
}

impl Default for LayoutStyle {
    fn default() -> Self {
        Self {
            min_width: 60.0,
            min_height: 40.0,
            char_width: 7.0,
            char_height: 14.0,
            node_padding_x: 12.0,
            node_padding_y: 8.0,
            node_gap: 24.0,
            layer_gap: 40.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LayoutNode {
    pub id: String,
    pub label: Option<String>,
    pub width: f32,
    pub height: f32,
    pub layer: usize,
    pub order: usize,
    pub x: f32,
    pub y: f32,
    pub is_dummy: bool,
    pub shape: NodeShape,
}

#[derive(Debug, Clone)]
pub struct LayoutEdge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub style: EdgeStyle,
    pub arrow: EdgeArrow,
    pub reversed: bool,
    pub points: Vec<(f32, f32)>,
}

#[derive(Debug, Clone)]
pub struct LayoutGraph {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub subgraphs: Vec<LayoutSubgraph>,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone)]
pub struct LayoutSubgraph {
    pub id: String,
    pub title: Option<String>,
    pub nodes: Vec<String>,
    pub subgraphs: Vec<LayoutSubgraph>,
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
    orig_from: usize,
    orig_to: usize,
    from: usize,
    to: usize,
    label: Option<String>,
    style: EdgeStyle,
    arrow: EdgeArrow,
    reversed: bool,
}

#[derive(Debug, Clone)]
struct EdgeChain {
    edge_index: usize,
    nodes: Vec<usize>,
}

#[derive(Debug, Clone)]
struct UnitEdge {
    from: usize,
    to: usize,
}

pub fn layout_flowchart(graph: &Graph, style: &LayoutStyle) -> LayoutGraph {
    let mut nodes = Vec::new();
    let mut node_index = HashMap::new();

    for node in &graph.nodes {
        let (width, height) = estimate_node_size(node, style);
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
        let from = *node_index.get(&edge.from).unwrap();
        let to = *node_index.get(&edge.to).unwrap();
        edges.push(EdgeMeta {
            orig_from: from,
            orig_to: to,
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

    let (mut chains, unit_edges) = insert_dummy_nodes(&mut nodes, &edges);

    let mut layers = build_layers(&mut nodes);
    reduce_crossings(&mut nodes, &mut layers, &unit_edges, 6);
    let direction = graph.direction.clone();
    assign_coordinates(&mut nodes, &layers, style, direction.clone());

    let (width, height) = compute_graph_extent(&nodes, direction.clone());
    let layout_edges = route_edges(
        &nodes,
        &edges,
        &mut chains,
        direction,
    );
    let layout_subgraphs = graph
        .subgraphs
        .iter()
        .map(build_layout_subgraph)
        .collect();

    LayoutGraph {
        nodes: nodes
            .into_iter()
            .map(|node| LayoutNode {
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
            })
            .collect(),
        edges: layout_edges,
        subgraphs: layout_subgraphs,
        width,
        height,
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

fn estimate_node_size(node: &ParsedNode, style: &LayoutStyle) -> (f32, f32) {
    let label = node.label.as_deref().unwrap_or(&node.id);
    let width = (label.chars().count() as f32 * style.char_width + style.node_padding_x * 2.0)
        .max(style.min_width);
    let height = (style.char_height + style.node_padding_y * 2.0).max(style.min_height);
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

    for edge in edges.iter_mut() {
        if edge.reversed {
            std::mem::swap(&mut edge.from, &mut edge.to);
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

fn insert_dummy_nodes(
    nodes: &mut Vec<WorkNode>,
    edges: &[EdgeMeta],
) -> (Vec<EdgeChain>, Vec<UnitEdge>) {
    let mut chains = Vec::new();
    let mut unit_edges = Vec::new();

    for (edge_index, edge) in edges.iter().enumerate() {
        let from_layer = nodes[edge.from].layer;
        let to_layer = nodes[edge.to].layer;
        if to_layer <= from_layer + 1 {
            chains.push(EdgeChain {
                edge_index,
                nodes: vec![edge.from, edge.to],
            });
            unit_edges.push(UnitEdge {
                from: edge.from,
                to: edge.to,
            });
            continue;
        }

        let mut chain_nodes = Vec::new();
        chain_nodes.push(edge.from);
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
            chain_nodes.push(dummy_idx);
            unit_edges.push(UnitEdge {
                from: prev,
                to: dummy_idx,
            });
            prev = dummy_idx;
        }
        chain_nodes.push(edge.to);
        unit_edges.push(UnitEdge {
            from: prev,
            to: edge.to,
        });

        chains.push(EdgeChain {
            edge_index,
            nodes: chain_nodes,
        });
    }

    (chains, unit_edges)
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

    for pass in 0..passes {
        let downward = pass % 2 == 0;
        if downward {
            for layer in 1..layers.len() {
                reorder_layer(nodes, layers, layer, &up_neighbors);
            }
        } else {
            for layer in (0..layers.len().saturating_sub(1)).rev() {
                reorder_layer(nodes, layers, layer, &down_neighbors);
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
        a.1
            .partial_cmp(&b.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| position[a.0].cmp(&position[b.0]))
    });

    layers[layer_index] = scored.iter().map(|(idx, _)| *idx).collect();
    for (order, &node_idx) in layers[layer_index].iter().enumerate() {
        nodes[node_idx].order = order;
    }
}

fn assign_coordinates(nodes: &mut [WorkNode], layers: &[Vec<usize>], style: &LayoutStyle, direction: Direction) {
    match direction {
        Direction::TB | Direction::BT => assign_coordinates_tb(nodes, layers, style),
        Direction::LR | Direction::RL => assign_coordinates_lr(nodes, layers, style),
    }
}

fn assign_coordinates_tb(nodes: &mut [WorkNode], layers: &[Vec<usize>], style: &LayoutStyle) {
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
            x += node.width + style.node_gap;
        }
        y += layer_height + style.layer_gap;
    }
}

fn assign_coordinates_lr(nodes: &mut [WorkNode], layers: &[Vec<usize>], style: &LayoutStyle) {
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
            y += node.height + style.node_gap;
        }
        x += layer_width + style.layer_gap;
    }
}

fn compute_graph_extent(nodes: &[WorkNode], direction: Direction) -> (f32, f32) {
    let mut max_x = 0.0f32;
    let mut max_y = 0.0f32;
    for node in nodes {
        let right = node.x + node.width / 2.0;
        let bottom = node.y + node.height / 2.0;
        max_x = max_x.max(right);
        max_y = max_y.max(bottom);
    }

    match direction {
        Direction::TB | Direction::BT => (max_x, max_y),
        Direction::LR | Direction::RL => (max_x, max_y),
    }
}

fn route_edges(
    nodes: &[WorkNode],
    edges: &[EdgeMeta],
    chains: &mut [EdgeChain],
    direction: Direction,
) -> Vec<LayoutEdge> {
    let mut layout_edges = Vec::new();
    for chain in chains {
        let edge = &edges[chain.edge_index];
        let points = match direction {
            Direction::TB | Direction::BT => route_chain_tb(nodes, &chain.nodes),
            Direction::LR | Direction::RL => route_chain_lr(nodes, &chain.nodes),
        };
        layout_edges.push(LayoutEdge {
            from: nodes[edge.orig_from].id.clone(),
            to: nodes[edge.orig_to].id.clone(),
            label: edge.label.clone(),
            style: edge.style.clone(),
            arrow: edge.arrow.clone(),
            reversed: edge.reversed,
            points,
        });
    }
    layout_edges
}

fn route_chain_tb(nodes: &[WorkNode], chain: &[usize]) -> Vec<(f32, f32)> {
    let mut points = Vec::new();
    for pair in chain.windows(2) {
        let from = &nodes[pair[0]];
        let to = &nodes[pair[1]];
        let start = (from.x, from.y + from.height / 2.0);
        let end = (to.x, to.y - to.height / 2.0);
        let mid_y = (start.1 + end.1) / 2.0;
        push_point(&mut points, start);
        if (start.0 - end.0).abs() < 0.01 {
            push_point(&mut points, (start.0, mid_y));
        } else {
            push_point(&mut points, (start.0, mid_y));
            push_point(&mut points, (end.0, mid_y));
        }
        push_point(&mut points, end);
    }
    points
}

fn route_chain_lr(nodes: &[WorkNode], chain: &[usize]) -> Vec<(f32, f32)> {
    let mut points = Vec::new();
    for pair in chain.windows(2) {
        let from = &nodes[pair[0]];
        let to = &nodes[pair[1]];
        let start = (from.x + from.width / 2.0, from.y);
        let end = (to.x - to.width / 2.0, to.y);
        let mid_x = (start.0 + end.0) / 2.0;
        push_point(&mut points, start);
        if (start.1 - end.1).abs() < 0.01 {
            push_point(&mut points, (mid_x, start.1));
        } else {
            push_point(&mut points, (mid_x, start.1));
            push_point(&mut points, (mid_x, end.1));
        }
        push_point(&mut points, end);
    }
    points
}

fn push_point(points: &mut Vec<(f32, f32)>, point: (f32, f32)) {
    if points.last().map_or(true, |last| {
        (last.0 - point.0).abs() > 0.01 || (last.1 - point.1).abs() > 0.01
    }) {
        points.push(point);
    }
}
