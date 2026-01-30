use crate::{Direction, EdgeArrow, EdgeStyle, ParseError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    KwFlowchart,
    KwGraph,
    Direction(Direction),
    Ident(String),
    EdgeOp(EdgeStyle, EdgeArrow),
    LabelBracket(String),
    LabelRound(String),
    LabelCircle(String),
    LabelDiamond(String),
    LabelHexagon(String),
    LabelPipe(String),
    Newline,
    Eof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub start: usize,
    pub end: usize,
}

pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
    len: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            pos: 0,
            len: input.len(),
        }
    }

    pub fn next_token(&mut self) -> Result<Token, ParseError> {
        let bytes = self.input.as_bytes();
        while self.pos < self.len {
            let b = bytes[self.pos];
            if b == b' ' || b == b'\t' || b == b'\r' {
                self.pos += 1;
                continue;
            }

            if b == b'%' && self.pos + 1 < self.len && bytes[self.pos + 1] == b'%' {
                self.pos += 2;
                while self.pos < self.len && bytes[self.pos] != b'\n' {
                    self.pos += 1;
                }
                continue;
            }

            if b == b'\n' {
                let start = self.pos;
                self.pos += 1;
                return Ok(Token {
                    kind: TokenKind::Newline,
                    start,
                    end: self.pos,
                });
            }

            if let Some(token) = self.read_edge_op()? {
                return Ok(token);
            }

            if b == b'[' {
                return self.read_bracket_label();
            }

            if b == b'|' {
                return self.read_pipe_label();
            }

            if b == b'(' {
                return self.read_round_label();
            }

            if b == b'{' {
                return self.read_brace_label();
            }

            if is_ident_start(b) {
                return self.read_ident();
            }

            return Err(ParseError::new(
                format!("unexpected character '{}'", b as char),
                self.pos,
            ));
        }

        Ok(Token {
            kind: TokenKind::Eof,
            start: self.pos,
            end: self.pos,
        })
    }

    fn read_ident(&mut self) -> Result<Token, ParseError> {
        let start = self.pos;
        let bytes = self.input.as_bytes();
        self.pos += 1;
        while self.pos < self.len && is_ident_continue(bytes[self.pos]) {
            self.pos += 1;
        }
        let text = &self.input[start..self.pos];
        let kind = match text {
            "flowchart" => TokenKind::KwFlowchart,
            "graph" => TokenKind::KwGraph,
            "TB" => TokenKind::Direction(Direction::TB),
            "TD" => TokenKind::Direction(Direction::TB),
            "BT" => TokenKind::Direction(Direction::BT),
            "LR" => TokenKind::Direction(Direction::LR),
            "RL" => TokenKind::Direction(Direction::RL),
            _ => TokenKind::Ident(text.to_string()),
        };
        Ok(Token {
            kind,
            start,
            end: self.pos,
        })
    }

    fn read_bracket_label(&mut self) -> Result<Token, ParseError> {
        let start = self.pos;
        let search_start = self.pos + 1;
        if let Some(end_rel) = self.input[search_start..].find(']') {
            let end = search_start + end_rel;
            let label = self.input[search_start..end].to_string();
            self.pos = end + 1;
            Ok(Token {
                kind: TokenKind::LabelBracket(label),
                start,
                end: self.pos,
            })
        } else {
            Err(ParseError::new("unterminated '[' label".to_string(), start))
        }
    }

    fn read_pipe_label(&mut self) -> Result<Token, ParseError> {
        let start = self.pos;
        let search_start = self.pos + 1;
        if let Some(end_rel) = self.input[search_start..].find('|') {
            let end = search_start + end_rel;
            let label = self.input[search_start..end].to_string();
            self.pos = end + 1;
            Ok(Token {
                kind: TokenKind::LabelPipe(label),
                start,
                end: self.pos,
            })
        } else {
            Err(ParseError::new("unterminated '|' label".to_string(), start))
        }
    }

    fn read_round_label(&mut self) -> Result<Token, ParseError> {
        let start = self.pos;
        let bytes = self.input.as_bytes();
        if self.pos + 1 < self.len && bytes[self.pos + 1] == b'(' {
            let search_start = self.pos + 2;
            if let Some(end_rel) = self.input[search_start..].find("))") {
                let end = search_start + end_rel;
                let label = self.input[search_start..end].to_string();
                self.pos = end + 2;
                return Ok(Token {
                    kind: TokenKind::LabelCircle(label),
                    start,
                    end: self.pos,
                });
            }
            return Err(ParseError::new("unterminated '(( ))' label".to_string(), start));
        }

        if self.pos + 1 < self.len && bytes[self.pos + 1] == b'"' {
            let search_start = self.pos + 2;
            if let Some(end_rel) = self.input[search_start..].find('"') {
                let end = search_start + end_rel;
                let label = self.input[search_start..end].to_string();
                let close = end + 1;
                if close < self.len && bytes[close] == b')' {
                    self.pos = close + 1;
                    return Ok(Token {
                        kind: TokenKind::LabelRound(label),
                        start,
                        end: self.pos,
                    });
                }
                return Err(ParseError::new("expected ')' after round label".to_string(), close));
            }
            return Err(ParseError::new("unterminated round label".to_string(), start));
        }

        let search_start = self.pos + 1;
        if let Some(end_rel) = self.input[search_start..].find(')') {
            let end = search_start + end_rel;
            let label = self.input[search_start..end].to_string();
            self.pos = end + 1;
            return Ok(Token {
                kind: TokenKind::LabelRound(label),
                start,
                end: self.pos,
            });
        }
        Err(ParseError::new("unterminated '( )' label".to_string(), start))
    }

    fn read_brace_label(&mut self) -> Result<Token, ParseError> {
        let start = self.pos;
        let bytes = self.input.as_bytes();
        if self.pos + 1 < self.len && bytes[self.pos + 1] == b'{' {
            let search_start = self.pos + 2;
            if let Some(end_rel) = self.input[search_start..].find("}}") {
                let end = search_start + end_rel;
                let label = self.input[search_start..end].to_string();
                self.pos = end + 2;
                return Ok(Token {
                    kind: TokenKind::LabelHexagon(label),
                    start,
                    end: self.pos,
                });
            }
            return Err(ParseError::new("unterminated '{{ }}' label".to_string(), start));
        }

        let search_start = self.pos + 1;
        if let Some(end_rel) = self.input[search_start..].find('}') {
            let end = search_start + end_rel;
            let label = self.input[search_start..end].to_string();
            self.pos = end + 1;
            return Ok(Token {
                kind: TokenKind::LabelDiamond(label),
                start,
                end: self.pos,
            });
        }
        Err(ParseError::new("unterminated '{ }' label".to_string(), start))
    }

    fn read_edge_op(&mut self) -> Result<Option<Token>, ParseError> {
        let bytes = self.input.as_bytes();
        if self.pos >= self.len {
            return Ok(None);
        }
        let start = self.pos;
        match bytes[self.pos] {
            b'-' => {
                if self.pos + 3 < self.len
                    && bytes[self.pos + 1] == b'.'
                    && bytes[self.pos + 2] == b'-'
                    && bytes[self.pos + 3] == b'>'
                {
                    self.pos += 4;
                    return Ok(Some(Token {
                        kind: TokenKind::EdgeOp(EdgeStyle::Dotted, EdgeArrow::Forward),
                        start,
                        end: self.pos,
                    }));
                }
                if self.pos + 2 < self.len
                    && bytes[self.pos + 1] == b'.'
                    && bytes[self.pos + 2] == b'-'
                {
                    self.pos += 3;
                    return Ok(Some(Token {
                        kind: TokenKind::EdgeOp(EdgeStyle::Dotted, EdgeArrow::None),
                        start,
                        end: self.pos,
                    }));
                }
                if self.pos + 2 < self.len
                    && bytes[self.pos + 1] == b'-'
                    && bytes[self.pos + 2] == b'>'
                {
                    self.pos += 3;
                    return Ok(Some(Token {
                        kind: TokenKind::EdgeOp(EdgeStyle::Solid, EdgeArrow::Forward),
                        start,
                        end: self.pos,
                    }));
                }
                if self.pos + 2 < self.len && bytes[self.pos + 1] == b'-' && bytes[self.pos + 2] == b'-' {
                    self.pos += 3;
                    return Ok(Some(Token {
                        kind: TokenKind::EdgeOp(EdgeStyle::Solid, EdgeArrow::None),
                        start,
                        end: self.pos,
                    }));
                }
            }
            b'=' => {
                if self.pos + 2 < self.len
                    && bytes[self.pos + 1] == b'='
                    && bytes[self.pos + 2] == b'>'
                {
                    self.pos += 3;
                    return Ok(Some(Token {
                        kind: TokenKind::EdgeOp(EdgeStyle::Thick, EdgeArrow::Forward),
                        start,
                        end: self.pos,
                    }));
                }
                if self.pos + 2 < self.len
                    && bytes[self.pos + 1] == b'='
                    && bytes[self.pos + 2] == b'='
                {
                    self.pos += 3;
                    return Ok(Some(Token {
                        kind: TokenKind::EdgeOp(EdgeStyle::Thick, EdgeArrow::None),
                        start,
                        end: self.pos,
                    }));
                }
            }
            _ => {}
        }
        Ok(None)
    }
}

fn is_ident_start(b: u8) -> bool {
    (b'A'..=b'Z').contains(&b) || (b'a'..=b'z').contains(&b) || b == b'_'
}

fn is_ident_continue(b: u8) -> bool {
    is_ident_start(b) || (b'0'..=b'9').contains(&b)
}
