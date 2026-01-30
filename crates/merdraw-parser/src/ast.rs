#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Direction {
    TB,
    BT,
    LR,
    RL,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeShape {
    Plain,
    Bracket,
    Round,
    Circle,
    Diamond,
    Hexagon,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub id: String,
    pub label: Option<String>,
    pub shape: NodeShape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EdgeStyle {
    Solid,
    Dotted,
    Thick,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EdgeArrow {
    None,
    Forward,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub style: EdgeStyle,
    pub arrow: EdgeArrow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Graph {
    pub direction: Direction,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub subgraphs: Vec<Subgraph>,
}

impl Graph {
    pub fn new(direction: Direction) -> Self {
        Self {
            direction,
            nodes: Vec::new(),
            edges: Vec::new(),
            subgraphs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subgraph {
    pub id: String,
    pub title: Option<String>,
    pub nodes: Vec<String>,
    pub subgraphs: Vec<Subgraph>,
}

impl Subgraph {
    pub fn new(id: String, title: Option<String>) -> Self {
        Self {
            id,
            title,
            nodes: Vec::new(),
            subgraphs: Vec::new(),
        }
    }

    pub fn add_node(&mut self, id: &str) {
        if !self.nodes.iter().any(|existing| existing == id) {
            self.nodes.push(id.to_string());
        }
    }
}
