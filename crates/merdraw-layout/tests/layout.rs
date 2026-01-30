use merdraw_layout::{layout_flowchart, LayoutStyle};
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
