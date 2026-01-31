use std::collections::{HashMap, HashSet, VecDeque};

use merdraw_parser::{
    Direction, Edge, EdgeArrow, EdgeStyle, Graph, Node as ParsedNode, NodeShape, Subgraph,
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
    pub is_cross: bool,
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
pub struct LayoutSubgraphBounds {
    pub path: String,
    pub label: String,
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

pub fn suggest_canvas_size(layout: &LayoutGraph, padding: f32, scale: f32) -> (u32, u32) {
    let layout_width = layout.width.max(1.0) * scale;
    let layout_height = layout.height.max(1.0) * scale;
    let width = (layout_width + padding * 2.0).ceil().max(1.0) as u32;
    let height = (layout_height + padding * 2.0).ceil().max(1.0) as u32;
    (width, height)
}

pub fn subgraph_bounds(layout: &LayoutGraph, padding: f32) -> Vec<LayoutSubgraphBounds> {
    let mut node_map = HashMap::new();
    for node in &layout.nodes {
        node_map.insert(node.id.as_str(), node);
    }
    let mut bounds = Vec::new();
    let mut path = Vec::new();
    for subgraph in &layout.subgraphs {
        collect_subgraph_bounds(subgraph, &node_map, padding, &mut path, &mut bounds);
    }
    bounds
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
    group_key: Vec<usize>,
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
    if graph.subgraphs.is_empty() {
        return layout_flowchart_flat(graph, style, None);
    }
    layout_flowchart_grouped(graph, style)
}

fn layout_flowchart_flat(
    graph: &Graph,
    style: &LayoutStyle,
    size_overrides: Option<&HashMap<String, (f32, f32)>>,
) -> LayoutGraph {
    let mut nodes = Vec::new();
    let mut node_index = HashMap::new();
    let mut group_paths = HashMap::new();
    collect_group_paths(&graph.subgraphs, &mut Vec::new(), &mut group_paths);

    for node in &graph.nodes {
        let (width, height) = size_overrides
            .and_then(|map| map.get(&node.id).copied())
            .unwrap_or_else(|| estimate_node_size(node, style));
        let idx = nodes.len();
        let group_key = group_paths.get(&node.id).cloned().unwrap_or_default();
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
            group_key,
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
    if size_overrides.is_none() {
        adjust_node_sizes_for_ports(&mut nodes, &edges, style, graph.direction.clone());
    }
    assign_layers(&mut nodes, &edges);

    let (mut chains, unit_edges) = insert_dummy_nodes(&mut nodes, &edges);

    let mut layers = build_layers(&mut nodes);
    reduce_crossings(&mut nodes, &mut layers, &unit_edges, 6);
    let direction = graph.direction.clone();
    let mut effective_style = style.clone();
    effective_style.layer_gap = compute_layer_gap(&nodes, &edges, style, direction.clone());
    assign_coordinates(&mut nodes, &layers, &effective_style, direction.clone());
    expand_layer_gaps(&mut nodes, &edges, &effective_style, direction.clone());
    separate_subgraphs(&mut nodes, graph, style, direction.clone());

    let (width, height) = compute_graph_extent(&nodes, direction.clone());
    let layout_edges = route_edges(
        &nodes,
        &edges,
        &mut chains,
        direction,
        &effective_style,
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

#[derive(Debug, Clone)]
struct GroupLayout {
    id: String,
    title: Option<String>,
    node_ids: Vec<String>,
    layout: LayoutGraph,
    width: f32,
    height: f32,
    padding_x: f32,
    padding_y: f32,
    title_height: f32,
    is_virtual: bool,
}

#[derive(Debug, Clone)]
struct CrossEdge {
    edge: Edge,
    from: LayoutNode,
    to: LayoutNode,
    forward: bool,
}

fn layout_flowchart_grouped(graph: &Graph, style: &LayoutStyle) -> LayoutGraph {
    let mut group_nodes: Vec<GroupLayout> = Vec::new();
    let mut node_to_group: HashMap<String, usize> = HashMap::new();
    let mut assigned: HashSet<String> = HashSet::new();

    for subgraph in &graph.subgraphs {
        let mut node_ids = Vec::new();
        collect_subgraph_nodes(subgraph, &mut node_ids);
        node_ids.sort();
        node_ids.dedup();
        for node_id in &node_ids {
            assigned.insert(node_id.clone());
        }

        let group_graph = build_subgraph_graph(graph, &node_ids);
        let layout = layout_flowchart_flat(&group_graph, style, None);
        let padding_x = style.node_padding_x * 2.0;
        let padding_y = style.node_padding_y * 2.0;
        let title_height = style.char_height + style.node_padding_y * 2.0;
        let width = layout.width + padding_x * 2.0;
        let height = layout.height + padding_y * 2.0 + title_height;

        let group_index = group_nodes.len();
        for node_id in &node_ids {
            node_to_group.insert(node_id.clone(), group_index);
        }
        group_nodes.push(GroupLayout {
            id: subgraph.id.clone(),
            title: subgraph.title.clone(),
            node_ids,
            layout,
            width,
            height,
            padding_x,
            padding_y,
            title_height,
            is_virtual: false,
        });
    }

    for node in &graph.nodes {
        if assigned.contains(&node.id) {
            continue;
        }
        let node_ids = vec![node.id.clone()];
        let group_graph = build_subgraph_graph(graph, &node_ids);
        let layout = layout_flowchart_flat(&group_graph, style, None);
        let padding_x = 0.0;
        let padding_y = 0.0;
        let title_height = 0.0;
        let width = layout.width;
        let height = layout.height;
        let group_index = group_nodes.len();
        node_to_group.insert(node.id.clone(), group_index);
        group_nodes.push(GroupLayout {
            id: format!("__group_{}", node.id),
            title: None,
            node_ids,
            layout,
            width,
            height,
            padding_x,
            padding_y,
            title_height,
            is_virtual: true,
        });
    }

    let super_layout = build_super_layout_row(&group_nodes, style);

    let mut global_nodes: Vec<LayoutNode> = Vec::new();
    let mut node_lookup: HashMap<String, LayoutNode> = HashMap::new();
    let mut global_edges: Vec<LayoutEdge> = Vec::new();
    let mut subgraphs: Vec<LayoutSubgraph> = Vec::new();

    for group in group_nodes.iter() {
        let super_node = find_layout_node(&super_layout, &group.id);
        if let Some(node) = super_node {
            let left = node.x - group.width / 2.0;
            let top = node.y - group.height / 2.0;
            let offset_x = left + group.padding_x;
            let offset_y = top + group.padding_y + group.title_height;

            for mut node in group.layout.nodes.clone() {
                node.x += offset_x;
                node.y += offset_y;
                node_lookup.insert(node.id.clone(), node.clone());
                global_nodes.push(node);
            }

            for mut edge in group.layout.edges.clone() {
                edge.points = edge
                    .points
                    .into_iter()
                    .map(|(x, y)| (x + offset_x, y + offset_y))
                    .collect();
                global_edges.push(edge);
            }

            if !group.is_virtual {
                subgraphs.push(LayoutSubgraph {
                    id: group.id.clone(),
                    title: group.title.clone(),
                    nodes: group.node_ids.clone(),
                    subgraphs: Vec::new(),
                });
            }
        }
    }

    // Cross-group edges routed above the top row
    let mut cross_edges = Vec::new();
    let mut cross_edge_keys: HashSet<(String, String, Option<String>)> = HashSet::new();
    for edge in &graph.edges {
        let from_group = node_to_group.get(&edge.from).copied();
        let to_group = node_to_group.get(&edge.to).copied();
        if from_group.is_none() || to_group.is_none() {
            continue;
        }
        if from_group == to_group && edge.from != edge.to {
            continue;
        }
        let from_node = node_lookup.get(&edge.from);
        let to_node = node_lookup.get(&edge.to);
        if let Some(from_node) = from_node {
            if edge.from == edge.to {
                continue;
            } else if let Some(to_node) = to_node {
                let key = (edge.from.clone(), edge.to.clone(), edge.label.clone());
                if !cross_edge_keys.insert(key) {
                    continue;
                }
                let forward = match graph.direction {
                    Direction::TB | Direction::BT => from_node.x <= to_node.x,
                    Direction::LR | Direction::RL => from_node.y <= to_node.y,
                };
                cross_edges.push(CrossEdge {
                    edge: edge.clone(),
                    from: from_node.clone(),
                    to: to_node.clone(),
                    forward,
                });
            }
        }
    }

    let cross_edge_count = cross_edges.len();

    let (start_offsets, end_offsets) =
        compute_cross_edge_ports(&cross_edges, graph.direction.clone(), style);

    let mut forward_indices: Vec<usize> = cross_edges
        .iter()
        .enumerate()
        .filter(|(_, edge)| edge.forward)
        .map(|(idx, _)| idx)
        .collect();
    let mut backward_indices: Vec<usize> = cross_edges
        .iter()
        .enumerate()
        .filter(|(_, edge)| !edge.forward)
        .map(|(idx, _)| idx)
        .collect();

    forward_indices.sort_by(|a, b| {
        let left = &cross_edges[*a];
        let right = &cross_edges[*b];
        left.from
            .x
            .partial_cmp(&right.from.x)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                left.to
                    .x
                    .partial_cmp(&right.to.x)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    backward_indices.sort_by(|a, b| {
        let left = &cross_edges[*a];
        let right = &cross_edges[*b];
        right
            .from
            .x
            .partial_cmp(&left.from.x)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                right
                    .to
                    .x
                    .partial_cmp(&left.to.x)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    let (band_top_start, band_bottom_start, band_gap, shift_y) = compute_cross_edge_bands(
        &super_layout,
        style,
        forward_indices.len(),
        backward_indices.len(),
    );
    if shift_y != 0.0 {
        for node in &mut global_nodes {
            node.y += shift_y;
        }
        for edge in &mut global_edges {
            edge.points = edge.points.iter().map(|(x, y)| (*x, *y + shift_y)).collect();
        }
        for edge in &mut cross_edges {
            edge.from.y += shift_y;
            edge.to.y += shift_y;
        }
    }

    let mut band_top = band_top_start + shift_y;
    let band_bottom = band_bottom_start + shift_y;
    if forward_indices.is_empty() && !backward_indices.is_empty() {
        band_top = band_bottom;
    }

    let forward_count = forward_indices.len();
    let backward_count = backward_indices.len();

    for (lane_index, edge_index) in forward_indices.iter().enumerate() {
        let edge_index = *edge_index;
        let edge = &cross_edges[edge_index];
        let band_y = band_top + band_gap * lane_index as f32;
        let start_offset = start_offsets.get(&edge_index).copied().unwrap_or(0.0);
        let end_offset = end_offsets.get(&edge_index).copied().unwrap_or(0.0);
        let points = route_cross_edge_band(
            &edge.from,
            &edge.to,
            graph.direction.clone(),
            band_y,
            start_offset,
            end_offset,
        );
        global_edges.push(LayoutEdge {
            from: edge.edge.from.clone(),
            to: edge.edge.to.clone(),
            is_cross: true,
            label: edge.edge.label.clone(),
            style: edge.edge.style.clone(),
            arrow: edge.edge.arrow.clone(),
            reversed: false,
            points,
        });
    }

    for (lane_index, edge_index) in backward_indices.iter().enumerate() {
        let edge_index = *edge_index;
        let edge = &cross_edges[edge_index];
        let band_y = band_bottom + band_gap * lane_index as f32;
        let start_offset = start_offsets.get(&edge_index).copied().unwrap_or(0.0);
        let end_offset = end_offsets.get(&edge_index).copied().unwrap_or(0.0);
        let points = route_cross_edge_band(
            &edge.from,
            &edge.to,
            graph.direction.clone(),
            band_y,
            start_offset,
            end_offset,
        );
        global_edges.push(LayoutEdge {
            from: edge.edge.from.clone(),
            to: edge.edge.to.clone(),
            is_cross: true,
            label: edge.edge.label.clone(),
            style: edge.edge.style.clone(),
            arrow: edge.edge.arrow.clone(),
            reversed: false,
            points,
        });
    }

    let band_bottom_extent = if backward_count > 0 {
        band_bottom + band_gap * (backward_count as f32 - 1.0)
    } else if forward_count > 0 {
        band_top + band_gap * (forward_count as f32 - 1.0)
    } else {
        band_top
    };

    let (width, height) = compute_group_extent(
        &group_nodes,
        &super_layout,
        band_top,
        band_bottom_extent,
        shift_y,
        cross_edge_count,
        band_gap,
    );

    LayoutGraph {
        nodes: global_nodes,
        edges: global_edges,
        subgraphs,
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

fn collect_group_paths(
    subgraphs: &[Subgraph],
    prefix: &mut Vec<usize>,
    map: &mut HashMap<String, Vec<usize>>,
) {
    for (idx, subgraph) in subgraphs.iter().enumerate() {
        prefix.push(idx);
        for node_id in &subgraph.nodes {
            map.entry(node_id.clone()).or_insert_with(|| prefix.clone());
        }
        collect_group_paths(&subgraph.subgraphs, prefix, map);
        prefix.pop();
    }
}

fn collect_subgraph_nodes(subgraph: &Subgraph, out: &mut Vec<String>) {
    for node_id in &subgraph.nodes {
        out.push(node_id.clone());
    }
    for child in &subgraph.subgraphs {
        collect_subgraph_nodes(child, out);
    }
}

fn build_subgraph_graph(graph: &Graph, node_ids: &[String]) -> Graph {
    let mut set = HashSet::new();
    for id in node_ids {
        set.insert(id.clone());
    }

    let mut subgraph = Graph::new(graph.direction.clone());
    for node in &graph.nodes {
        if set.contains(&node.id) {
            subgraph.nodes.push(node.clone());
        }
    }
    for edge in &graph.edges {
        if set.contains(&edge.from) && set.contains(&edge.to) {
            subgraph.edges.push(edge.clone());
        }
    }
    subgraph
}

fn find_layout_node<'a>(layout: &'a LayoutGraph, id: &str) -> Option<&'a LayoutNode> {
    layout.nodes.iter().find(|node| node.id == id)
}

fn build_super_layout_row(groups: &[GroupLayout], style: &LayoutStyle) -> LayoutGraph {
    let mut nodes = Vec::new();
    let mut x = 0.0f32;
    let gap = style.node_gap * 2.0;
    let mut max_height = 0.0f32;

    for group in groups {
        let center_x = x + group.width / 2.0;
        let center_y = group.height / 2.0;
        nodes.push(LayoutNode {
            id: group.id.clone(),
            label: group.title.clone(),
            width: group.width,
            height: group.height,
            layer: 0,
            order: 0,
            x: center_x,
            y: center_y,
            is_dummy: false,
            shape: NodeShape::Plain,
        });
        x += group.width + gap;
        max_height = max_height.max(group.height);
    }

    let width = if nodes.is_empty() { 0.0 } else { x - gap };
    let height = max_height;

    LayoutGraph {
        nodes,
        edges: Vec::new(),
        subgraphs: Vec::new(),
        width,
        height,
    }
}

fn route_cross_edge_band(
    from: &LayoutNode,
    to: &LayoutNode,
    direction: Direction,
    band_y: f32,
    start_offset: f32,
    end_offset: f32,
) -> Vec<(f32, f32)> {
    let mut points = Vec::new();
    match direction {
        Direction::TB | Direction::BT => {
            let start = (from.x + start_offset, from.y - from.height / 2.0);
            let end = (to.x + end_offset, to.y - to.height / 2.0);
            push_point(&mut points, start);
            push_point(&mut points, (start.0, band_y));
            push_point(&mut points, (end.0, band_y));
            push_point(&mut points, end);
        }
        Direction::LR | Direction::RL => {
            let start = (from.x - from.width / 2.0, from.y + start_offset);
            let end = (to.x - to.width / 2.0, to.y + end_offset);
            push_point(&mut points, start);
            push_point(&mut points, (band_y, start.1));
            push_point(&mut points, (band_y, end.1));
            push_point(&mut points, end);
        }
    }
    points
}

fn compute_cross_edge_bands(
    super_layout: &LayoutGraph,
    style: &LayoutStyle,
    forward_count: usize,
    backward_count: usize,
) -> (f32, f32, f32, f32) {
    let mut min_top = 0.0f32;
    for node in &super_layout.nodes {
        let top = node.y - node.height / 2.0;
        min_top = min_top.min(top);
    }
    let band_gap = (style.char_height + style.node_padding_y * 2.0).max(24.0);
    let total = forward_count + backward_count;
    if total == 0 {
        return (min_top, min_top, band_gap, 0.0);
    }

    let band_top_start = min_top - band_gap * total as f32 - style.node_padding_y;
    let band_bottom_start = band_top_start + band_gap * forward_count as f32;
    let shift_y = if band_top_start < 0.0 { -band_top_start } else { 0.0 };
    (band_top_start, band_bottom_start, band_gap, shift_y)
}

fn compute_group_extent(
    groups: &[GroupLayout],
    super_layout: &LayoutGraph,
    band_top: f32,
    band_bottom: f32,
    shift_y: f32,
    cross_edge_count: usize,
    band_gap: f32,
) -> (f32, f32) {
    let mut max_x = 0.0f32;
    let mut max_y = 0.0f32;
    for group in groups {
        if let Some(node) = find_layout_node(super_layout, &group.id) {
            let right = node.x + group.width / 2.0;
            let bottom = node.y + group.height / 2.0 + shift_y;
            max_x = max_x.max(right);
            max_y = max_y.max(bottom);
        }
    }
    let base_top = if cross_edge_count > 0 {
        band_top - band_gap * 1.5
    } else {
        band_top
    };
    let min_y = base_top.min(0.0);
    let max_y = max_y.max(band_bottom);
    let height = max_y - min_y;
    (max_x, height)
}

fn compute_cross_edge_ports(
    cross_edges: &[CrossEdge],
    direction: Direction,
    style: &LayoutStyle,
) -> (HashMap<usize, f32>, HashMap<usize, f32>) {
    let mut outgoing: HashMap<String, Vec<usize>> = HashMap::new();
    let mut incoming: HashMap<String, Vec<usize>> = HashMap::new();
    let mut nodes: HashMap<String, LayoutNode> = HashMap::new();

    for (idx, edge) in cross_edges.iter().enumerate() {
        outgoing.entry(edge.from.id.clone()).or_default().push(idx);
        incoming.entry(edge.to.id.clone()).or_default().push(idx);
        nodes.entry(edge.from.id.clone()).or_insert_with(|| edge.from.clone());
        nodes.entry(edge.to.id.clone()).or_insert_with(|| edge.to.clone());
    }

    let mut start_offsets = HashMap::new();
    let mut end_offsets = HashMap::new();

    for (node_id, edge_indices) in outgoing {
        if let Some(node) = nodes.get(&node_id) {
            assign_cross_edge_offsets(
                node,
                &edge_indices,
                cross_edges,
                direction.clone(),
                style,
                true,
                &mut start_offsets,
            );
        }
    }

    for (node_id, edge_indices) in incoming {
        if let Some(node) = nodes.get(&node_id) {
            assign_cross_edge_offsets(
                node,
                &edge_indices,
                cross_edges,
                direction.clone(),
                style,
                false,
                &mut end_offsets,
            );
        }
    }

    (start_offsets, end_offsets)
}

fn assign_cross_edge_offsets(
    node: &LayoutNode,
    edge_indices: &[usize],
    cross_edges: &[CrossEdge],
    direction: Direction,
    style: &LayoutStyle,
    outgoing: bool,
    out: &mut HashMap<usize, f32>,
) {
    if edge_indices.is_empty() {
        return;
    }

    let mut sorted = edge_indices.to_vec();
    sorted.sort_by(|a, b| {
        let a_node = if outgoing {
            &cross_edges[*a].to
        } else {
            &cross_edges[*a].from
        };
        let b_node = if outgoing {
            &cross_edges[*b].to
        } else {
            &cross_edges[*b].from
        };
        match direction {
            Direction::TB | Direction::BT => a_node
                .x
                .partial_cmp(&b_node.x)
                .unwrap_or(std::cmp::Ordering::Equal),
            Direction::LR | Direction::RL => a_node
                .y
                .partial_cmp(&b_node.y)
                .unwrap_or(std::cmp::Ordering::Equal),
        }
    });

    let max_offset = match direction {
        Direction::TB | Direction::BT => {
            (node.width / 2.0 - style.node_padding_x).max(style.char_width)
        }
        Direction::LR | Direction::RL => {
            (node.height / 2.0 - style.node_padding_y).max(style.char_height)
        }
    };

    let n = sorted.len();
    if n == 1 {
        out.insert(sorted[0], 0.0);
        return;
    }

    let span = max_offset * 2.0;
    let step = span / (n as f32 - 1.0);
    for (i, edge_idx) in sorted.into_iter().enumerate() {
        let offset = -max_offset + step * i as f32;
        out.insert(edge_idx, offset);
    }
}

fn adjust_node_sizes_for_ports(
    nodes: &mut [WorkNode],
    edges: &[EdgeMeta],
    style: &LayoutStyle,
    direction: Direction,
) {
    let mut outgoing = vec![0usize; nodes.len()];
    let mut incoming = vec![0usize; nodes.len()];
    for edge in edges {
        outgoing[edge.from] += 1;
        incoming[edge.to] += 1;
    }

    let port_gap = style.char_width.max(6.0) * 2.0;
    let port_gap_v = style.char_height.max(10.0) * 1.2;

    for (idx, node) in nodes.iter_mut().enumerate() {
        if node.is_dummy {
            continue;
        }
        let ports = outgoing[idx].max(incoming[idx]).max(1) as f32;
        match direction {
            Direction::TB | Direction::BT => {
                let min_width = (ports - 1.0) * port_gap + style.node_padding_x * 2.0 + style.char_width;
                if node.width < min_width {
                    node.width = min_width;
                }
            }
            Direction::LR | Direction::RL => {
                let min_height = (ports - 1.0) * port_gap_v + style.node_padding_y * 2.0 + style.char_height;
                if node.height < min_height {
                    node.height = min_height;
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Bounds {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

fn collect_subgraph_bounds(
    subgraph: &LayoutSubgraph,
    node_map: &HashMap<&str, &LayoutNode>,
    padding: f32,
    path: &mut Vec<String>,
    out: &mut Vec<LayoutSubgraphBounds>,
) -> Option<Bounds> {
    path.push(subgraph.id.clone());
    let mut bounds: Option<Bounds> = None;
    let mut has_content = false;

    for node_id in &subgraph.nodes {
        if let Some(node) = node_map.get(node_id.as_str()) {
            has_content = true;
            let node_bounds = Bounds {
                left: node.x - node.width / 2.0,
                right: node.x + node.width / 2.0,
                top: node.y - node.height / 2.0,
                bottom: node.y + node.height / 2.0,
            };
            bounds = Some(match bounds {
                Some(existing) => union_bounds(existing, node_bounds),
                None => node_bounds,
            });
        }
    }

    for child in &subgraph.subgraphs {
        if let Some(child_bounds) =
            collect_subgraph_bounds(child, node_map, padding, path, out)
        {
            has_content = true;
            bounds = Some(match bounds {
                Some(existing) => union_bounds(existing, child_bounds),
                None => child_bounds,
            });
        }
    }

    if !has_content {
        path.pop();
        return None;
    }

    let mut bounds = bounds.unwrap();
    bounds.left -= padding;
    bounds.right += padding;
    bounds.top -= padding;
    bounds.bottom += padding;

    let label = subgraph.title.as_deref().unwrap_or(subgraph.id.as_str());
    out.push(LayoutSubgraphBounds {
        path: path.join("/"),
        label: label.to_string(),
        left: bounds.left,
        top: bounds.top,
        right: bounds.right,
        bottom: bounds.bottom,
    });

    path.pop();
    Some(bounds)
}

fn union_bounds(a: Bounds, b: Bounds) -> Bounds {
    Bounds {
        left: a.left.min(b.left),
        top: a.top.min(b.top),
        right: a.right.max(b.right),
        bottom: a.bottom.max(b.bottom),
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
                group_key: nodes[edge.from].group_key.clone(),
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
        layer.sort_by(|&a, &b| {
            cmp_group_key(&nodes[a], &nodes[b]).then_with(|| a.cmp(&b))
        });
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
        cmp_group_key(&nodes[a.0], &nodes[b.0])
            .then_with(|| {
                a.1
                    .partial_cmp(&b.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| position[a.0].cmp(&position[b.0]))
    });

    layers[layer_index] = scored.iter().map(|(idx, _)| *idx).collect();
    for (order, &node_idx) in layers[layer_index].iter().enumerate() {
        nodes[node_idx].order = order;
    }
}

fn cmp_group_key(a: &WorkNode, b: &WorkNode) -> std::cmp::Ordering {
    match (a.group_key.is_empty(), b.group_key.is_empty()) {
        (true, true) => std::cmp::Ordering::Equal,
        (true, false) => std::cmp::Ordering::Greater,
        (false, true) => std::cmp::Ordering::Less,
        (false, false) => a.group_key.cmp(&b.group_key),
    }
}

fn separate_subgraphs(nodes: &mut [WorkNode], graph: &Graph, style: &LayoutStyle, direction: Direction) {
    if graph.subgraphs.is_empty() {
        return;
    }

    let gap = style.node_gap.max(16.0);
    let padding = (style.node_gap + style.layer_gap * 0.5).max(12.0);
    let mut groups: Vec<(usize, Bounds)> = Vec::new();
    for (idx, _subgraph) in graph.subgraphs.iter().enumerate() {
        if let Some(bounds) = group_bounds(nodes, idx, padding) {
            groups.push((idx, bounds));
        }
    }

    if groups.len() <= 1 {
        return;
    }

    match direction {
        Direction::TB | Direction::BT => {
            groups.sort_by(|a, b| a.1.left.partial_cmp(&b.1.left).unwrap_or(std::cmp::Ordering::Equal));
            let mut current_right = groups[0].1.right;
            for (group_idx, bounds) in groups.into_iter().skip(1) {
                if bounds.left < current_right + gap {
                    let delta = current_right + gap - bounds.left;
                    shift_group(nodes, group_idx, delta, 0.0);
                    current_right = bounds.right + delta;
                } else {
                    current_right = bounds.right;
                }
            }
        }
        Direction::LR | Direction::RL => {
            groups.sort_by(|a, b| a.1.top.partial_cmp(&b.1.top).unwrap_or(std::cmp::Ordering::Equal));
            let mut current_bottom = groups[0].1.bottom;
            for (group_idx, bounds) in groups.into_iter().skip(1) {
                if bounds.top < current_bottom + gap {
                    let delta = current_bottom + gap - bounds.top;
                    shift_group(nodes, group_idx, 0.0, delta);
                    current_bottom = bounds.bottom + delta;
                } else {
                    current_bottom = bounds.bottom;
                }
            }
        }
    }
}

fn group_bounds(nodes: &[WorkNode], group_idx: usize, padding: f32) -> Option<Bounds> {
    let mut bounds: Option<Bounds> = None;
    for node in nodes {
        if node.is_dummy {
            continue;
        }
        if node.group_key.first().copied() != Some(group_idx) {
            continue;
        }
        let node_bounds = Bounds {
            left: node.x - node.width / 2.0,
            right: node.x + node.width / 2.0,
            top: node.y - node.height / 2.0,
            bottom: node.y + node.height / 2.0,
        };
        bounds = Some(match bounds {
            Some(existing) => union_bounds(existing, node_bounds),
            None => node_bounds,
        });
    }
    bounds.map(|mut bounds| {
        bounds.left -= padding;
        bounds.right += padding;
        bounds.top -= padding;
        bounds.bottom += padding;
        bounds
    })
}

fn shift_group(nodes: &mut [WorkNode], group_idx: usize, dx: f32, dy: f32) {
    if dx == 0.0 && dy == 0.0 {
        return;
    }
    for node in nodes {
        if node.group_key.first().copied() != Some(group_idx) {
            continue;
        }
        node.x += dx;
        node.y += dy;
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
    style: &LayoutStyle,
) -> Vec<LayoutEdge> {
    let mut layout_edges = Vec::new();
    let lane_offsets = build_edge_lane_offsets(nodes, edges, direction.clone(), style);
    let (start_offsets, end_offsets) = build_edge_port_offsets(nodes, edges, direction.clone(), style);
    for chain in chains {
        let edge = &edges[chain.edge_index];
        if edge.orig_from == edge.orig_to {
            let points = match direction {
                Direction::TB | Direction::BT => route_self_loop_tb(&nodes[edge.orig_from], style),
                Direction::LR | Direction::RL => route_self_loop_lr(&nodes[edge.orig_from], style),
            };
            layout_edges.push(LayoutEdge {
                from: nodes[edge.orig_from].id.clone(),
                to: nodes[edge.orig_to].id.clone(),
                is_cross: false,
                label: edge.label.clone(),
                style: edge.style.clone(),
                arrow: edge.arrow.clone(),
                reversed: edge.reversed,
                points,
            });
            continue;
        }
        if edge.from == edge.to {
            let points = match direction {
                Direction::TB | Direction::BT => route_self_loop_tb(&nodes[edge.from], style),
                Direction::LR | Direction::RL => route_self_loop_lr(&nodes[edge.from], style),
            };
            layout_edges.push(LayoutEdge {
                from: nodes[edge.orig_from].id.clone(),
                to: nodes[edge.orig_to].id.clone(),
                is_cross: false,
                label: edge.label.clone(),
                style: edge.style.clone(),
                arrow: edge.arrow.clone(),
                reversed: edge.reversed,
                points,
            });
            continue;
        }
        let lane_offset = lane_offsets
            .get(&chain.edge_index)
            .copied()
            .unwrap_or(0.0);
        let start_offset = start_offsets
            .get(&chain.edge_index)
            .copied()
            .unwrap_or(0.0);
        let end_offset = end_offsets
            .get(&chain.edge_index)
            .copied()
            .unwrap_or(0.0);
        let points = match direction {
            Direction::TB | Direction::BT => route_chain_tb(
                nodes,
                &chain.nodes,
                lane_offset,
                start_offset,
                end_offset,
                edge.orig_from,
                edge.orig_to,
            ),
            Direction::LR | Direction::RL => route_chain_lr(
                nodes,
                &chain.nodes,
                lane_offset,
                start_offset,
                end_offset,
                edge.orig_from,
                edge.orig_to,
            ),
        };
        layout_edges.push(LayoutEdge {
            from: nodes[edge.orig_from].id.clone(),
            to: nodes[edge.orig_to].id.clone(),
            is_cross: false,
            label: edge.label.clone(),
            style: edge.style.clone(),
            arrow: edge.arrow.clone(),
            reversed: edge.reversed,
            points,
        });
    }
    layout_edges
}

fn route_self_loop_tb(node: &WorkNode, style: &LayoutStyle) -> Vec<(f32, f32)> {
    let mut points = Vec::new();
    let right = node.x + node.width / 2.0;
    let loop_w = style.node_gap.max(style.char_width * 3.0);
    let loop_h = (style.char_height * 1.5).max(style.node_padding_y * 2.0);
    let start = (right, node.y);
    push_point(&mut points, start);
    push_point(&mut points, (right + loop_w, node.y));
    push_point(&mut points, (right + loop_w, node.y - loop_h));
    push_point(&mut points, (right, node.y - loop_h));
    push_point(&mut points, (right, node.y));
    points
}

fn route_self_loop_lr(node: &WorkNode, style: &LayoutStyle) -> Vec<(f32, f32)> {
    let mut points = Vec::new();
    let bottom = node.y + node.height / 2.0;
    let loop_h = style.node_gap.max(style.char_height * 2.0);
    let loop_w = (style.char_width * 1.5).max(style.node_padding_x * 2.0);
    let start = (node.x, bottom);
    push_point(&mut points, start);
    push_point(&mut points, (node.x, bottom + loop_h));
    push_point(&mut points, (node.x + loop_w, bottom + loop_h));
    push_point(&mut points, (node.x + loop_w, bottom));
    push_point(&mut points, (node.x, bottom));
    points
}

fn route_chain_tb(
    nodes: &[WorkNode],
    chain: &[usize],
    lane_offset: f32,
    start_offset: f32,
    end_offset: f32,
    orig_from: usize,
    orig_to: usize,
) -> Vec<(f32, f32)> {
    let mut points = Vec::new();
    for pair in chain.windows(2) {
        let from = &nodes[pair[0]];
        let to = &nodes[pair[1]];
        let start_x = if pair[0] == orig_from {
            from.x + start_offset
        } else {
            from.x
        };
        let end_x = if pair[1] == orig_to {
            to.x + end_offset
        } else {
            to.x
        };
        let start = (start_x, from.y + from.height / 2.0);
        let end = (end_x, to.y - to.height / 2.0);
        let mid_y = (start.1 + end.1) / 2.0 + lane_offset;
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

fn route_chain_lr(
    nodes: &[WorkNode],
    chain: &[usize],
    lane_offset: f32,
    start_offset: f32,
    end_offset: f32,
    orig_from: usize,
    orig_to: usize,
) -> Vec<(f32, f32)> {
    let mut points = Vec::new();
    for pair in chain.windows(2) {
        let from = &nodes[pair[0]];
        let to = &nodes[pair[1]];
        let start_y = if pair[0] == orig_from {
            from.y + start_offset
        } else {
            from.y
        };
        let end_y = if pair[1] == orig_to {
            to.y + end_offset
        } else {
            to.y
        };
        let start = (from.x + from.width / 2.0, start_y);
        let end = (to.x - to.width / 2.0, end_y);
        let mid_x = (start.0 + end.0) / 2.0 + lane_offset;
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

fn build_edge_lane_offsets(
    nodes: &[WorkNode],
    edges: &[EdgeMeta],
    direction: Direction,
    style: &LayoutStyle,
) -> HashMap<usize, f32> {
    let layer_bounds = compute_layer_bounds(nodes, direction.clone());
    let mut by_from: HashMap<usize, Vec<usize>> = HashMap::new();
    for (idx, edge) in edges.iter().enumerate() {
        by_from.entry(edge.from).or_default().push(idx);
    }

    let mut offsets = HashMap::new();
    for (from_idx, mut edge_indices) in by_from {
        edge_indices.sort_by(|a, b| match direction {
            Direction::TB | Direction::BT => nodes[edges[*a].to]
                .x
                .partial_cmp(&nodes[edges[*b].to].x)
                .unwrap_or(std::cmp::Ordering::Equal),
            Direction::LR | Direction::RL => nodes[edges[*a].to]
                .y
                .partial_cmp(&nodes[edges[*b].to].y)
                .unwrap_or(std::cmp::Ordering::Equal),
        });

        let n = edge_indices.len();
        let from_layer = nodes[from_idx].layer;
        let bounds = layer_bounds.get(from_layer);
        if n == 1 || bounds.is_none() {
            for edge_idx in edge_indices {
                offsets.insert(edge_idx, 0.0);
            }
            continue;
        }

        let (_layer_min, layer_max) = bounds.unwrap();
        let next_top = layer_bounds
            .get(from_layer + 1)
            .map(|(min, _)| *min)
            .unwrap_or(layer_max + style.layer_gap.max(24.0));
        let gap = (next_top - layer_max).max(style.layer_gap.max(24.0));

        for (i, edge_idx) in edge_indices.into_iter().enumerate() {
            let offset = match direction {
                Direction::TB | Direction::BT => {
                    let start = nodes[edges[edge_idx].from].y
                        + nodes[edges[edge_idx].from].height / 2.0;
                    let end = nodes[edges[edge_idx].to].y
                        - nodes[edges[edge_idx].to].height / 2.0;
                    let mid_y = (start + end) / 2.0;
                    let lane_y = layer_max + gap * ((i + 1) as f32) / ((n + 1) as f32);
                    lane_y - mid_y
                }
                Direction::LR | Direction::RL => {
                    let start = nodes[edges[edge_idx].from].x
                        + nodes[edges[edge_idx].from].width / 2.0;
                    let end = nodes[edges[edge_idx].to].x
                        - nodes[edges[edge_idx].to].width / 2.0;
                    let mid_x = (start + end) / 2.0;
                    let lane_x = layer_max + gap * ((i + 1) as f32) / ((n + 1) as f32);
                    lane_x - mid_x
                }
            };
            offsets.insert(edge_idx, offset);
        }
    }

    offsets
}

fn compute_layer_bounds(nodes: &[WorkNode], direction: Direction) -> Vec<(f32, f32)> {
    let max_layer = nodes.iter().map(|node| node.layer).max().unwrap_or(0);
    let mut bounds = vec![(f32::MAX, f32::MIN); max_layer + 1];
    for node in nodes {
        if node.is_dummy {
            continue;
        }
        let (min_axis, max_axis) = match direction {
            Direction::TB | Direction::BT => (
                node.y - node.height / 2.0,
                node.y + node.height / 2.0,
            ),
            Direction::LR | Direction::RL => (
                node.x - node.width / 2.0,
                node.x + node.width / 2.0,
            ),
        };
        let entry = &mut bounds[node.layer];
        entry.0 = entry.0.min(min_axis);
        entry.1 = entry.1.max(max_axis);
    }
    for entry in &mut bounds {
        if entry.0 == f32::MAX {
            entry.0 = 0.0;
            entry.1 = 0.0;
        }
    }
    bounds
}

fn expand_layer_gaps(
    nodes: &mut [WorkNode],
    edges: &[EdgeMeta],
    style: &LayoutStyle,
    direction: Direction,
) {
    if nodes.is_empty() {
        return;
    }

    let layer_bounds = compute_layer_bounds(nodes, direction.clone());
    if layer_bounds.len() <= 1 {
        return;
    }

    let layer_count = layer_bounds.len();
    let mut offsets = vec![0.0f32; layer_count];
    let lane_size = match direction {
        Direction::TB | Direction::BT => (style.char_height + style.node_padding_y * 2.0).max(18.0),
        Direction::LR | Direction::RL => (style.char_width + style.node_padding_x).max(10.0) * 1.5,
    };

    let mut outgoing_per_layer = vec![0usize; layer_count];
    for edge in edges {
        if edge.reversed {
            continue;
        }
        let from_layer = nodes[edge.from].layer;
        if from_layer < outgoing_per_layer.len() {
            outgoing_per_layer[from_layer] += 1;
        }
    }

    for layer in 0..layer_count.saturating_sub(1) {
        let required = style.layer_gap
            + (outgoing_per_layer[layer].saturating_sub(1) as f32) * lane_size * 0.7;

        match direction {
            Direction::TB | Direction::BT => {
                let current_bottom = layer_bounds[layer].1 + offsets[layer];
                let next_top = layer_bounds[layer + 1].0 + offsets[layer + 1];
                let gap = next_top - current_bottom;
                if gap < required {
                    let delta = required - gap;
                    for next in (layer + 1)..layer_count {
                        offsets[next] += delta;
                    }
                }
            }
            Direction::LR | Direction::RL => {
                let current_right = layer_bounds[layer].1 + offsets[layer];
                let next_left = layer_bounds[layer + 1].0 + offsets[layer + 1];
                let gap = next_left - current_right;
                if gap < required {
                    let delta = required - gap;
                    for next in (layer + 1)..layer_count {
                        offsets[next] += delta;
                    }
                }
            }
        }
    }

    for node in nodes {
        let layer = node.layer;
        if layer >= offsets.len() {
            continue;
        }
        match direction {
            Direction::TB | Direction::BT => node.y += offsets[layer],
            Direction::LR | Direction::RL => node.x += offsets[layer],
        }
    }
}

fn build_edge_port_offsets(
    nodes: &[WorkNode],
    edges: &[EdgeMeta],
    direction: Direction,
    style: &LayoutStyle,
) -> (HashMap<usize, f32>, HashMap<usize, f32>) {
    let mut outgoing: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut incoming: HashMap<usize, Vec<usize>> = HashMap::new();

    for (idx, edge) in edges.iter().enumerate() {
        outgoing.entry(edge.from).or_default().push(idx);
        incoming.entry(edge.to).or_default().push(idx);
    }

    let mut start_offsets = HashMap::new();
    let mut end_offsets = HashMap::new();

    for (node_idx, edge_indices) in outgoing {
        assign_port_offsets(
            nodes,
            edges,
            node_idx,
            &edge_indices,
            direction.clone(),
            style,
            true,
            &mut start_offsets,
        );
    }

    for (node_idx, edge_indices) in incoming {
        assign_port_offsets(
            nodes,
            edges,
            node_idx,
            &edge_indices,
            direction.clone(),
            style,
            false,
            &mut end_offsets,
        );
    }

    (start_offsets, end_offsets)
}

fn assign_port_offsets(
    nodes: &[WorkNode],
    edges: &[EdgeMeta],
    node_idx: usize,
    edge_indices: &[usize],
    direction: Direction,
    style: &LayoutStyle,
    outgoing: bool,
    out: &mut HashMap<usize, f32>,
) {
    if edge_indices.is_empty() {
        return;
    }

    let mut sorted = edge_indices.to_vec();
    sorted.sort_by(|a, b| {
        let a_node = if outgoing { edges[*a].to } else { edges[*a].from };
        let b_node = if outgoing { edges[*b].to } else { edges[*b].from };
        match direction {
            Direction::TB | Direction::BT => nodes[a_node]
                .x
                .partial_cmp(&nodes[b_node].x)
                .unwrap_or(std::cmp::Ordering::Equal),
            Direction::LR | Direction::RL => nodes[a_node]
                .y
                .partial_cmp(&nodes[b_node].y)
                .unwrap_or(std::cmp::Ordering::Equal),
        }
    });

    let max_offset = match direction {
        Direction::TB | Direction::BT => {
            (nodes[node_idx].width / 2.0 - style.node_padding_x).max(style.char_width)
        }
        Direction::LR | Direction::RL => {
            (nodes[node_idx].height / 2.0 - style.node_padding_y).max(style.char_height)
        }
    };

    let n = sorted.len();
    if n == 1 {
        out.insert(sorted[0], 0.0);
        return;
    }

    let span = max_offset * 2.0;
    let step = span / (n as f32 - 1.0);
    for (i, edge_idx) in sorted.into_iter().enumerate() {
        let offset = -max_offset + step * i as f32;
        out.insert(edge_idx, offset);
    }
}

fn compute_layer_gap(
    nodes: &[WorkNode],
    edges: &[EdgeMeta],
    style: &LayoutStyle,
    direction: Direction,
) -> f32 {
    let mut out_counts: HashMap<usize, usize> = HashMap::new();
    for edge in edges {
        let from_layer = nodes[edge.from].layer;
        *out_counts.entry(from_layer).or_insert(0) += 1;
    }
    let max_lanes = out_counts.values().copied().max().unwrap_or(1).max(1);
    let lane_size = match direction {
        Direction::TB | Direction::BT => (style.char_height + style.node_padding_y * 2.0).max(18.0),
        Direction::LR | Direction::RL => (style.char_width + style.node_padding_x).max(10.0) * 1.5,
    };
    let gap = style.layer_gap + (max_lanes.saturating_sub(1) as f32) * lane_size * 0.6;
    gap.min(style.layer_gap * 4.0).max(style.layer_gap)
}

fn push_point(points: &mut Vec<(f32, f32)>, point: (f32, f32)) {
    if points.last().map_or(true, |last| {
        (last.0 - point.0).abs() > 0.01 || (last.1 - point.1).abs() > 0.01
    }) {
        points.push(point);
    }
}
