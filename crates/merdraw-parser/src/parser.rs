use std::collections::HashMap;

use crate::ast::{Direction, Edge, EdgeArrow, EdgeStyle, Graph, Node, NodeShape, Subgraph};
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
                TokenKind::KwSubgraph => {
                    let subgraph = self.parse_subgraph()?;
                    self.graph.subgraphs.push(subgraph);
                }
                TokenKind::KwEnd => {
                    return Err(self.error_here("unexpected 'end' outside subgraph"));
                }
                TokenKind::Ident(id) => {
                    self.advance()?;
                    let mut subgraph = None;
                    self.parse_statement(id, &mut subgraph)?;
                }
                _ => {
                    return Err(self.error_here("expected identifier, 'subgraph', or newline"));
                }
            }
        }

        Ok(self.graph)
    }

    fn parse_statement(&mut self, id: String, subgraph: &mut Option<&mut Subgraph>) -> Result<(), ParseError> {
        if let Some(current) = subgraph.as_deref_mut() {
            current.add_node(&id);
        }

        match self.current.kind.clone() {
            TokenKind::EdgeOp(style, arrow) => {
                let subgraph_ref = subgraph.as_deref_mut();
                self.parse_edge_chain(id, style, arrow, subgraph_ref)
            }
            TokenKind::LabelBracket(label) => {
                self.advance()?;
                self.upsert_node(id.clone(), Some(label), NodeShape::Bracket);
                if let Some(current) = subgraph.as_deref_mut() {
                    current.add_node(&id);
                }
                self.parse_edge_after_labeled_node(id, subgraph)
            }
            TokenKind::LabelRound(label) => {
                self.advance()?;
                self.upsert_node(id.clone(), Some(label), NodeShape::Round);
                if let Some(current) = subgraph.as_deref_mut() {
                    current.add_node(&id);
                }
                self.parse_edge_after_labeled_node(id, subgraph)
            }
            TokenKind::LabelCircle(label) => {
                self.advance()?;
                self.upsert_node(id.clone(), Some(label), NodeShape::Circle);
                if let Some(current) = subgraph.as_deref_mut() {
                    current.add_node(&id);
                }
                self.parse_edge_after_labeled_node(id, subgraph)
            }
            TokenKind::LabelDiamond(label) => {
                self.advance()?;
                self.upsert_node(id.clone(), Some(label), NodeShape::Diamond);
                if let Some(current) = subgraph.as_deref_mut() {
                    current.add_node(&id);
                }
                self.parse_edge_after_labeled_node(id, subgraph)
            }
            TokenKind::LabelHexagon(label) => {
                self.advance()?;
                self.upsert_node(id.clone(), Some(label), NodeShape::Hexagon);
                if let Some(current) = subgraph.as_deref_mut() {
                    current.add_node(&id);
                }
                self.parse_edge_after_labeled_node(id, subgraph)
            }
            TokenKind::Newline | TokenKind::Eof => {
                self.upsert_node(id, None, NodeShape::Plain);
                Ok(())
            }
            _ => Err(self.error_here("expected edge, label, or end of line")),
        }
    }

    fn parse_edge_after_labeled_node(
        &mut self,
        from: String,
        subgraph: &mut Option<&mut Subgraph>,
    ) -> Result<(), ParseError> {
        match self.current.kind.clone() {
            TokenKind::EdgeOp(style, arrow) => {
                let subgraph_ref = subgraph.as_deref_mut();
                self.parse_edge_chain(from, style, arrow, subgraph_ref)
            }
            TokenKind::Newline | TokenKind::Eof => Ok(()),
            _ => Err(self.error_here("expected edge or end of line")),
        }
    }

    fn parse_edge_chain(
        &mut self,
        mut from: String,
        mut style: EdgeStyle,
        mut arrow: EdgeArrow,
        mut subgraph: Option<&mut Subgraph>,
    ) -> Result<(), ParseError> {
        loop {
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
            if let Some(current) = subgraph.as_deref_mut() {
                current.add_node(&from);
                current.add_node(&to);
            }
            self.graph.edges.push(Edge {
                from: from.clone(),
                to: to.clone(),
                label,
                style,
                arrow,
            });

            match self.current.kind.clone() {
                TokenKind::EdgeOp(next_style, next_arrow) => {
                    from = to;
                    style = next_style;
                    arrow = next_arrow;
                }
                _ => break,
            }
        }
        Ok(())
    }

    fn parse_subgraph(&mut self) -> Result<Subgraph, ParseError> {
        self.advance()?;
        let id = match self.current.kind.clone() {
            TokenKind::Ident(id) => {
                self.advance()?;
                id
            }
            _ => return Err(self.error_here("expected subgraph identifier")),
        };

        let mut title = None;
        match self.current.kind.clone() {
            TokenKind::StringLiteral(label)
            | TokenKind::LabelBracket(label)
            | TokenKind::LabelRound(label)
            | TokenKind::LabelCircle(label)
            | TokenKind::LabelDiamond(label)
            | TokenKind::LabelHexagon(label) => {
                title = Some(label);
                self.advance()?;
            }
            _ => {}
        }

        let mut subgraph = Subgraph::new(id, title);
        while self.current.kind != TokenKind::Eof {
            match self.current.kind.clone() {
                TokenKind::Newline => {
                    self.advance()?;
                }
                TokenKind::KwSubgraph => {
                    let child = self.parse_subgraph()?;
                    subgraph.subgraphs.push(child);
                }
                TokenKind::KwEnd => {
                    self.advance()?;
                    return Ok(subgraph);
                }
                TokenKind::Ident(id) => {
                    self.advance()?;
                    let mut current = Some(&mut subgraph);
                    self.parse_statement(id, &mut current)?;
                }
                _ => {
                    return Err(self.error_here("expected identifier, 'subgraph', or 'end'"));
                }
            }
        }

        Err(self.error_here("expected 'end' to close subgraph"))
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
