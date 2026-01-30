use merdraw_parser::{parse_flowchart, Direction, EdgeArrow, EdgeStyle, NodeShape};

#[test]
fn parses_minimal_flowchart() {
    let graph = parse_flowchart("flowchart TB\n").expect("parse failed");
    assert_eq!(graph.direction, Direction::TB);
    assert!(graph.nodes.is_empty());
    assert!(graph.edges.is_empty());
}

#[test]
fn parses_nodes_with_shapes() {
    let input = "flowchart LR\nA\nB[Box]\nC(Round)\nD((Circle))\nE{Decision}\nF{{Hex}}\n";
    let graph = parse_flowchart(input).expect("parse failed");

    let a = graph.nodes.iter().find(|n| n.id == "A").unwrap();
    assert_eq!(a.shape, NodeShape::Plain);
    assert!(a.label.is_none());

    let b = graph.nodes.iter().find(|n| n.id == "B").unwrap();
    assert_eq!(b.shape, NodeShape::Bracket);
    assert_eq!(b.label.as_deref(), Some("Box"));

    let c = graph.nodes.iter().find(|n| n.id == "C").unwrap();
    assert_eq!(c.shape, NodeShape::Round);
    assert_eq!(c.label.as_deref(), Some("Round"));

    let d = graph.nodes.iter().find(|n| n.id == "D").unwrap();
    assert_eq!(d.shape, NodeShape::Circle);
    assert_eq!(d.label.as_deref(), Some("Circle"));

    let e = graph.nodes.iter().find(|n| n.id == "E").unwrap();
    assert_eq!(e.shape, NodeShape::Diamond);
    assert_eq!(e.label.as_deref(), Some("Decision"));

    let f = graph.nodes.iter().find(|n| n.id == "F").unwrap();
    assert_eq!(f.shape, NodeShape::Hexagon);
    assert_eq!(f.label.as_deref(), Some("Hex"));
}

#[test]
fn parses_graph_alias_and_td_direction() {
    let input = "graph TD\nA-->B\n";
    let graph = parse_flowchart(input).expect("parse failed");
    assert_eq!(graph.direction, Direction::TB);
    assert_eq!(graph.edges.len(), 1);

    let first = &graph.edges[0];
    assert_eq!(first.from, "A");
    assert_eq!(first.to, "B");
    assert!(first.label.is_none());
    assert_eq!(first.style, EdgeStyle::Solid);
    assert_eq!(first.arrow, EdgeArrow::Forward);
}

#[test]
fn parses_edge_styles_and_labels() {
    let input = "flowchart TD\nA-->B\nB---C\nC-.->D\nD-.-E\nE==>F\nF===G\nG-->|go|H\nI[Box]-->J\n";
    let graph = parse_flowchart(input).expect("parse failed");
    assert_eq!(graph.edges.len(), 8);

    let first = &graph.edges[0];
    assert_eq!(first.style, EdgeStyle::Solid);
    assert_eq!(first.arrow, EdgeArrow::Forward);

    let second = &graph.edges[1];
    assert_eq!(second.style, EdgeStyle::Solid);
    assert_eq!(second.arrow, EdgeArrow::None);

    let third = &graph.edges[2];
    assert_eq!(third.style, EdgeStyle::Dotted);
    assert_eq!(third.arrow, EdgeArrow::Forward);

    let fourth = &graph.edges[3];
    assert_eq!(fourth.style, EdgeStyle::Dotted);
    assert_eq!(fourth.arrow, EdgeArrow::None);

    let fifth = &graph.edges[4];
    assert_eq!(fifth.style, EdgeStyle::Thick);
    assert_eq!(fifth.arrow, EdgeArrow::Forward);

    let sixth = &graph.edges[5];
    assert_eq!(sixth.style, EdgeStyle::Thick);
    assert_eq!(sixth.arrow, EdgeArrow::None);

    let seventh = &graph.edges[6];
    assert_eq!(seventh.label.as_deref(), Some("go"));

    let eighth = &graph.edges[7];
    assert_eq!(eighth.from, "I");
    assert_eq!(eighth.to, "J");
    let i_node = graph.nodes.iter().find(|n| n.id == "I").unwrap();
    assert_eq!(i_node.label.as_deref(), Some("Box"));
}

#[test]
fn ignores_comments_and_blank_lines() {
    let input = "flowchart TB\n%% comment\n\nA-->B\n";
    let graph = parse_flowchart(input).expect("parse failed");
    assert_eq!(graph.edges.len(), 1);
}

#[test]
fn errors_on_missing_header() {
    let err = parse_flowchart("A-->B").unwrap_err();
    assert!(err.message.contains("flowchart"));
}

#[test]
fn errors_on_bad_arrow() {
    assert!(parse_flowchart("flowchart TB\nA--B\n").is_err());
}

#[test]
fn parses_chained_edges() {
    let input = "flowchart TB\nA-->B-->C\n";
    let graph = parse_flowchart(input).expect("parse failed");
    assert_eq!(graph.edges.len(), 2);
    assert_eq!(graph.edges[0].from, "A");
    assert_eq!(graph.edges[0].to, "B");
    assert_eq!(graph.edges[1].from, "B");
    assert_eq!(graph.edges[1].to, "C");
}

#[test]
fn parses_subgraphs() {
    let input = "flowchart TB\nsubgraph group1 \"Outer Group\"\nA-->B-->C\nsubgraph inner\nD\nend\nend\n";
    let graph = parse_flowchart(input).expect("parse failed");
    assert_eq!(graph.subgraphs.len(), 1);
    let outer = &graph.subgraphs[0];
    assert_eq!(outer.id, "group1");
    assert_eq!(outer.title.as_deref(), Some("Outer Group"));
    assert!(outer.nodes.contains(&"A".to_string()));
    assert!(outer.nodes.contains(&"B".to_string()));
    assert!(outer.nodes.contains(&"C".to_string()));
    assert_eq!(outer.subgraphs.len(), 1);
    let inner = &outer.subgraphs[0];
    assert_eq!(inner.id, "inner");
    assert!(inner.nodes.contains(&"D".to_string()));
}
