//! Hand-written lexer for the Behavior DSL.
//!
//! Produces a flat stream of tokens with source spans. Whitespace and
//! `//` line comments are skipped. The lexer is total: any unrecognised
//! character becomes an `Unknown` token and the parser decides how to report.

use crate::ast::Span;
use crate::error::BehaviorDslError;

/// A lexical token.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    /// Identifier or keyword (the parser decides).
    Ident(String),
    /// Integer literal. The string form is kept so we can report it verbatim on
    /// errors.
    Int(i64),
    /// Float literal.
    Float(f64),
    /// String literal (already unescaped).
    String(String),
    /// `(`, `)`, `{`, `}`, `[`, `]`.
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    /// `,`, `;`, `:`, `.`, `@`.
    Comma,
    Semi,
    Colon,
    Dot,
    At,
    /// `true`, `false`.
    True,
    False,
    /// End-of-file sentinel.
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

/// Lexer state over a source string.
pub struct Lexer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Lexer {
            src,
            bytes: src.as_bytes(),
            pos: 0,
        }
    }

    /// Consume the whole source and return the token stream. Returns an error
    /// only on malformed literals — unknown characters are reported as
    /// `UnexpectedToken` from the parser.
    pub fn tokenize(mut self) -> Result<Vec<Token>, BehaviorDslError> {
        let mut out = Vec::new();
        loop {
            self.skip_trivia();
            if self.pos >= self.bytes.len() {
                out.push(Token {
                    kind: TokenKind::Eof,
                    span: Span::new(self.pos, self.pos),
                });
                return Ok(out);
            }
            let start = self.pos;
            let c = self.bytes[self.pos];
            let tok = match c {
                b'(' => self.single(TokenKind::LParen),
                b')' => self.single(TokenKind::RParen),
                b'{' => self.single(TokenKind::LBrace),
                b'}' => self.single(TokenKind::RBrace),
                b'[' => self.single(TokenKind::LBracket),
                b']' => self.single(TokenKind::RBracket),
                b',' => self.single(TokenKind::Comma),
                b';' => self.single(TokenKind::Semi),
                b':' => self.single(TokenKind::Colon),
                b'.' => {
                    // Could be start of `.5` style float — but the DSL doesn't
                    // support that; treat `.` as a punctuator only.
                    self.single(TokenKind::Dot)
                }
                b'@' => self.single(TokenKind::At),
                b'"' => self.string()?,
                b'-' | b'0'..=b'9' => self.number()?,
                b'_' | b'a'..=b'z' | b'A'..=b'Z' => self.ident_or_keyword(),
                _ => {
                    // Unrecognised byte — emit as Ident containing the raw
                    // character so the parser can report a precise error.
                    let ch = self.src[start..].chars().next().unwrap_or('?');
                    let len = ch.len_utf8();
                    self.pos += len;
                    Token {
                        kind: TokenKind::Ident(ch.to_string()),
                        span: Span::new(start, start + len),
                    }
                }
            };
            out.push(tok);
        }
    }

    fn single(&mut self, kind: TokenKind) -> Token {
        let start = self.pos;
        self.pos += 1;
        Token {
            kind,
            span: Span::new(start, self.pos),
        }
    }

    fn skip_trivia(&mut self) {
        loop {
            while self.pos < self.bytes.len() {
                let c = self.bytes[self.pos];
                if c == b' ' || c == b'\t' || c == b'\r' || c == b'\n' {
                    self.pos += 1;
                } else {
                    break;
                }
            }
            if self.pos + 1 < self.bytes.len()
                && self.bytes[self.pos] == b'/'
                && self.bytes[self.pos + 1] == b'/'
            {
                while self.pos < self.bytes.len() && self.bytes[self.pos] != b'\n' {
                    self.pos += 1;
                }
                continue;
            }
            break;
        }
    }

    fn string(&mut self) -> Result<Token, BehaviorDslError> {
        let start = self.pos;
        self.pos += 1; // consume opening quote
        let mut value = String::new();
        while self.pos < self.bytes.len() {
            let c = self.bytes[self.pos];
            if c == b'"' {
                self.pos += 1;
                return Ok(Token {
                    kind: TokenKind::String(value),
                    span: Span::new(start, self.pos),
                });
            }
            if c == b'\\' {
                self.pos += 1;
                if self.pos >= self.bytes.len() {
                    return Err(BehaviorDslError::InvalidStringLiteral {
                        reason: "unterminated escape".to_string(),
                        span: Span::new(start, self.pos),
                    });
                }
                let esc = self.bytes[self.pos];
                let ch = match esc {
                    b'"' => '"',
                    b'\\' => '\\',
                    b'n' => '\n',
                    b't' => '\t',
                    b'r' => '\r',
                    b'0' => '\0',
                    other => {
                        return Err(BehaviorDslError::InvalidStringLiteral {
                            reason: format!("unknown escape `\\{}`", other as char),
                            span: Span::new(start, self.pos + 1),
                        });
                    }
                };
                value.push(ch);
                self.pos += 1;
                continue;
            }
            if c == b'\n' {
                return Err(BehaviorDslError::InvalidStringLiteral {
                    reason: "newline inside string literal".to_string(),
                    span: Span::new(start, self.pos),
                });
            }
            // Push the full UTF-8 char.
            let ch = self.src[self.pos..].chars().next().unwrap_or('?');
            let len = ch.len_utf8();
            value.push(ch);
            self.pos += len;
        }
        Err(BehaviorDslError::InvalidStringLiteral {
            reason: "unterminated string literal".to_string(),
            span: Span::new(start, self.pos),
        })
    }

    fn number(&mut self) -> Result<Token, BehaviorDslError> {
        let start = self.pos;
        if self.bytes[self.pos] == b'-' {
            self.pos += 1;
        }
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        let mut is_float = false;
        if self.pos + 1 < self.bytes.len()
            && self.bytes[self.pos] == b'.'
            && self.bytes[self.pos + 1].is_ascii_digit()
        {
            is_float = true;
            self.pos += 1;
            while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }
        let literal = &self.src[start..self.pos];
        let span = Span::new(start, self.pos);
        let kind = if is_float {
            TokenKind::Float(literal.parse::<f64>().map_err(|_| {
                BehaviorDslError::InvalidNumberLiteral {
                    literal: literal.to_string(),
                    span,
                }
            })?)
        } else {
            TokenKind::Int(literal.parse::<i64>().map_err(|_| {
                BehaviorDslError::InvalidNumberLiteral {
                    literal: literal.to_string(),
                    span,
                }
            })?)
        };
        Ok(Token { kind, span })
    }

    fn ident_or_keyword(&mut self) -> Token {
        let start = self.pos;
        while self.pos < self.bytes.len() {
            let c = self.bytes[self.pos];
            if c == b'_' || c.is_ascii_alphanumeric() {
                self.pos += 1;
            } else {
                break;
            }
        }
        let text = &self.src[start..self.pos];
        let span = Span::new(start, self.pos);
        let kind = match text {
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            _ => TokenKind::Ident(text.to_string()),
        };
        Token { kind, span }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_simple() {
        let toks = Lexer::new("behavior foo { version 1 }").tokenize().unwrap();
        let kinds: Vec<_> = toks.iter().map(|t| t.kind.clone()).collect();
        assert!(matches!(kinds[0], TokenKind::Ident(ref s) if s == "behavior"));
        assert!(matches!(kinds[1], TokenKind::Ident(ref s) if s == "foo"));
        assert!(matches!(kinds[2], TokenKind::LBrace));
        assert!(matches!(kinds[3], TokenKind::Ident(ref s) if s == "version"));
        assert!(matches!(kinds[4], TokenKind::Int(1)));
        assert!(matches!(kinds[5], TokenKind::RBrace));
        assert!(matches!(kinds.last(), Some(TokenKind::Eof)));
    }

    #[test]
    fn tokenize_string_with_escapes() {
        let toks = Lexer::new(r#" "hi\nthere" "#).tokenize().unwrap();
        assert!(matches!(toks[0].kind, TokenKind::String(ref s) if s == "hi\nthere"));
    }

    #[test]
    fn tokenize_floats_and_ints() {
        let toks = Lexer::new("1 2.5 -3").tokenize().unwrap();
        assert!(matches!(toks[0].kind, TokenKind::Int(1)));
        assert!(matches!(toks[1].kind, TokenKind::Float(f) if (f - 2.5).abs() < 1e-9));
        assert!(matches!(toks[2].kind, TokenKind::Int(-3)));
    }

    #[test]
    fn line_comments_are_skipped() {
        let toks = Lexer::new("// a comment\nfoo").tokenize().unwrap();
        assert!(matches!(toks[0].kind, TokenKind::Ident(ref s) if s == "foo"));
    }
}
