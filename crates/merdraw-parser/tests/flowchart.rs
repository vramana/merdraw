use merdraw_parser::{parse_flowchart, Direction, NodeShape};

#[test]
fn parses_minimal_flowchart() {
    let graph = parse_flowchart("flowchart TB\n").expect("parse failed");
    assert_eq!(graph.direction, Direction::TB);
    assert!(graph.nodes.is_empty());
    assert!(graph.edges.is_empty());
}

#[test]
fn parses_nodes_with_shapes() {
    let input = "flowchart LR\nA\nB[Box]\nC(\"Round\")\n";
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
}

#[test]
fn parses_edges_and_labels() {
    let input = "flowchart TD\nA-->B\nB-->|go|C\n";
    let graph = parse_flowchart(input).expect("parse failed");
    assert_eq!(graph.edges.len(), 2);

    let first = &graph.edges[0];
    assert_eq!(first.from, "A");
    assert_eq!(first.to, "B");
    assert!(first.label.is_none());

    let second = &graph.edges[1];
    assert_eq!(second.from, "B");
    assert_eq!(second.to, "C");
    assert_eq!(second.label.as_deref(), Some("go"));
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
