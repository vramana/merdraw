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
}

impl Graph {
    pub fn new(direction: Direction) -> Self {
        Self {
            direction,
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }
}
