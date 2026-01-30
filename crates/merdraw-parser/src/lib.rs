mod ast;
mod lexer;
mod parser;

pub use ast::{Direction, Edge, EdgeArrow, EdgeStyle, Graph, Node, NodeShape, Subgraph};
pub use parser::parse_flowchart;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub offset: usize,
}

impl ParseError {
    pub fn new(message: String, offset: usize) -> Self {
        Self { message, offset }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at byte {}", self.message, self.offset)
    }
}

impl std::error::Error for ParseError {}
