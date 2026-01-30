# Mermaid Native Renderer Plan (MVP)

## Notes (Paper)
- Paper: "An Efficient Implementation of Sugiyama’s Algorithm for Layered Graph Drawing" (JGAA 2005).
- Key idea to adopt later: *sparse normalization* for long edges (use p/q vertices + segment instead of a dummy per layer) and a sparse compaction graph to keep memory and runtime near-linear.
- MVP should stick to classic Sugiyama with full dummy nodes; upgrade to sparse normalization in a later iteration.

## MVP Layout Engine (No Dependencies)

### Goals
- Deterministic layout for flowcharts (DAGs), direction-aware (TB/LR/etc.).
- Reasonable crossing reduction and spacing without external libs.
- Simple polyline routing that is easy to render.

### Phases
1) **Graph normalization**
   - Detect and reverse back-edges to break cycles (DFS-based); record `reversed` for rendering.
   - Convert to a DAG.

2) **Layer assignment**
   - Use longest-path or BFS rank assignment.
   - Rank is y-layer for TB/BT, x-layer for LR/RL.

3) **Dummy node insertion**
   - For edges that span multiple layers, insert dummy nodes (one per intermediate layer).

4) **Crossing reduction**
   - Barycenter heuristic sweeps: top-down then bottom-up.
   - Iterate a small fixed number of passes (e.g., 4–8).

5) **Coordinate assignment**
   - For each layer, order nodes by sweep result.
   - Assign positions with fixed spacing (node size + padding).
   - Optional compaction pass to reduce gaps.

6) **Edge routing**
   - For edges crossing multiple layers, route through dummy nodes.
   - Use simple orthogonal segments aligned to layer direction.
   - Choose ports by direction (top/bottom for TB, left/right for LR).

### Data Structures (Rust)
- `LayoutNode { id, width, height, layer, order, x, y, is_dummy, ... }`
- `LayoutEdge { from, to, label, style, arrow, reversed, ... }`
- `Layer { nodes: Vec<NodeId> }`

### Deliverables
- `crates/merdraw-layout` with:
  - `layout_flowchart(graph: &Graph, style: &LayoutStyle) -> LayoutGraph` (MVP entrypoint)
  - `LayoutGraph { nodes, edges, width, height }`
  - Unit tests for: layering, dummy nodes, edge routing, direction handling.

