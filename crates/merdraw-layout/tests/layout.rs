use merdraw_layout::{layout_flowchart, subgraph_bounds, LayoutStyle};
use merdraw_parser::parse_flowchart;

#[test]
fn layouts_simple_chain_tb() {
    let graph = parse_flowchart("flowchart TB\nA-->B-->C\n").expect("parse failed");
    let layout = layout_flowchart(&graph, &LayoutStyle::default());

    let a = layout.nodes.iter().find(|n| n.id == "A").unwrap();
    let b = layout.nodes.iter().find(|n| n.id == "B").unwrap();
    let c = layout.nodes.iter().find(|n| n.id == "C").unwrap();

    assert!(a.y < b.y && b.y < c.y);
    assert_eq!(layout.edges.len(), 2);
}

#[test]
fn layouts_lr_direction() {
    let graph = parse_flowchart("flowchart LR\nA-->B\n").expect("parse failed");
    let layout = layout_flowchart(&graph, &LayoutStyle::default());

    let a = layout.nodes.iter().find(|n| n.id == "A").unwrap();
    let b = layout.nodes.iter().find(|n| n.id == "B").unwrap();

    assert!(a.x < b.x);
}

#[test]
fn inserts_dummy_nodes_for_long_edges() {
    let graph = parse_flowchart("flowchart TB\nA-->B-->C\nA-->C\n").expect("parse failed");
    let layout = layout_flowchart(&graph, &LayoutStyle::default());
    let dummy_count = layout.nodes.iter().filter(|n| n.is_dummy).count();
    assert!(dummy_count > 0);
}

#[test]
fn subgraph_bounds_do_not_overlap_for_siblings() {
    let graph = parse_flowchart(
        "flowchart TB\n\
         subgraph client[Client]\n\
         A-->B\n\
         end\n\
         subgraph server[Server]\n\
         C-->D\n\
         end\n\
         B-->C\n",
    )
    .expect("parse failed");
    let style = LayoutStyle {
        layer_gap: 120.0,
        node_gap: 40.0,
        ..LayoutStyle::default()
    };
    let layout = layout_flowchart(&graph, &style);
    let bounds = subgraph_bounds(&layout, 12.0);
    let top_level: Vec<_> = bounds.iter().filter(|b| !b.path.contains('/')).collect();
    assert!(top_level.len() >= 2);
    for i in 0..top_level.len() {
        for j in (i + 1)..top_level.len() {
            let a = top_level[i];
            let b = top_level[j];
            assert!(!rects_overlap(a, b), "subgraphs overlap: {} and {}", a.path, b.path);
        }
    }
}

fn rects_overlap(a: &merdraw_layout::LayoutSubgraphBounds, b: &merdraw_layout::LayoutSubgraphBounds) -> bool {
    a.left < b.right && a.right > b.left && a.top < b.bottom && a.bottom > b.top
}
