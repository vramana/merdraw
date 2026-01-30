use std::collections::HashMap;

use crate::ast::{Direction, Edge, EdgeArrow, EdgeStyle, Graph, Node, NodeShape};
use crate::lexer::{Lexer, TokenKind};
use crate::ParseError;

pub fn parse_flowchart(input: &str) -> Result<Graph, ParseError> {
    let parser = Parser::new(input)?;
    parser.parse_flowchart()
}

struct Parser<'a> {
    lexer: Lexer<'a>,
    current: crate::lexer::Token,
    graph: Graph,
    nodes_by_id: HashMap<String, usize>,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Result<Self, ParseError> {
        let mut lexer = Lexer::new(input);
        let first = lexer.next_token()?;
        Ok(Self {
            lexer,
            current: first,
            graph: Graph::new(Direction::TB),
            nodes_by_id: HashMap::new(),
        })
    }

    fn parse_flowchart(mut self) -> Result<Graph, ParseError> {
        self.expect_header()?;
        self.advance()?;

        if let TokenKind::Direction(dir) = self.current.kind.clone() {
            self.graph.direction = dir;
            self.advance()?;
        }

        while self.current.kind != TokenKind::Eof {
            match self.current.kind.clone() {
                TokenKind::Newline => {
                    self.advance()?;
                }
                TokenKind::Ident(id) => {
                    self.advance()?;
                    self.parse_statement(id)?;
                }
                _ => {
                    return Err(self.error_here("expected identifier or newline"));
                }
            }
        }

        Ok(self.graph)
    }

    fn parse_statement(&mut self, id: String) -> Result<(), ParseError> {
        match self.current.kind.clone() {
            TokenKind::EdgeOp(style, arrow) => self.parse_edge(id, style, arrow),
            TokenKind::LabelBracket(label) => {
                self.advance()?;
                self.upsert_node(id.clone(), Some(label), NodeShape::Bracket);
                self.parse_edge_after_node(id)
            }
            TokenKind::LabelRound(label) => {
                self.advance()?;
                self.upsert_node(id.clone(), Some(label), NodeShape::Round);
                self.parse_edge_after_node(id)
            }
            TokenKind::LabelCircle(label) => {
                self.advance()?;
                self.upsert_node(id.clone(), Some(label), NodeShape::Circle);
                self.parse_edge_after_node(id)
            }
            TokenKind::LabelDiamond(label) => {
                self.advance()?;
                self.upsert_node(id.clone(), Some(label), NodeShape::Diamond);
                self.parse_edge_after_node(id)
            }
            TokenKind::LabelHexagon(label) => {
                self.advance()?;
                self.upsert_node(id.clone(), Some(label), NodeShape::Hexagon);
                self.parse_edge_after_node(id)
            }
            TokenKind::Newline | TokenKind::Eof => {
                self.upsert_node(id, None, NodeShape::Plain);
                Ok(())
            }
            _ => Err(self.error_here("expected edge, label, or end of line")),
        }
    }

    fn parse_edge_after_node(&mut self, from: String) -> Result<(), ParseError> {
        match self.current.kind.clone() {
            TokenKind::EdgeOp(style, arrow) => self.parse_edge(from, style, arrow),
            TokenKind::Newline | TokenKind::Eof => Ok(()),
            _ => Err(self.error_here("expected edge or end of line")),
        }
    }

    fn parse_edge(&mut self, from: String, style: EdgeStyle, arrow: EdgeArrow) -> Result<(), ParseError> {
        self.advance()?;
        let mut label = None;
        if let TokenKind::LabelPipe(text) = self.current.kind.clone() {
            label = Some(text);
            self.advance()?;
        }

        let to = match self.current.kind.clone() {
            TokenKind::Ident(id) => {
                self.advance()?;
                id
            }
            _ => return Err(self.error_here("expected destination node id")),
        };

        self.upsert_node(from.clone(), None, NodeShape::Plain);
        self.upsert_node(to.clone(), None, NodeShape::Plain);
        self.graph.edges.push(Edge {
            from,
            to,
            label,
            style,
            arrow,
        });
        Ok(())
    }

    fn upsert_node(&mut self, id: String, label: Option<String>, shape: NodeShape) {
        if let Some(&idx) = self.nodes_by_id.get(&id) {
            if label.is_some() {
                let node = &mut self.graph.nodes[idx];
                node.label = label;
                node.shape = shape;
            }
            return;
        }

        let node = Node { id: id.clone(), label, shape };
        let idx = self.graph.nodes.len();
        self.graph.nodes.push(node);
        self.nodes_by_id.insert(id, idx);
    }

    fn expect_header(&self) -> Result<(), ParseError> {
        match self.current.kind {
            TokenKind::KwFlowchart | TokenKind::KwGraph => Ok(()),
            _ => Err(self.error_here("expected 'flowchart' or 'graph' header")),
        }
    }

    fn advance(&mut self) -> Result<(), ParseError> {
        self.current = self.lexer.next_token()?;
        Ok(())
    }

    fn error_here(&self, message: &str) -> ParseError {
        ParseError::new(message.to_string(), self.current.start)
    }
}
