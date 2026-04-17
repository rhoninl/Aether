//! Recursive-descent parser for the Behavior DSL.
//!
//! Accepts a source string; produces a [`Module`] AST.
//!
//! ```text
//! behavior <Name> {
//!     @caps(Cap1, Cap2)        // optional, default empty
//!     version <n>
//!     <node>
//! }
//! ```
//! where `<node>` is either a verb call (`verb(arg, ...);` or no-semi at the
//! top of a combinator block), or a combinator block:
//! ```text
//! sequence { <node>* }
//! selector { <node>* }
//! parallel { <node>* }
//! invert { <node> }
//! retry(n) { <node>* }
//! timeout(ms) { <node>* }
//! ```

use crate::ast::*;
use crate::caps::{Capability, CapabilitySet};
use crate::error::{BehaviorDslError, BehaviorDslResult};
use crate::lexer::{Lexer, Token, TokenKind};

/// Parse the given source string into a [`Module`].
pub fn parse(source: &str) -> BehaviorDslResult<Module> {
    let tokens = Lexer::new(source).tokenize()?;
    let mut parser = Parser::new(tokens);
    parser.parse_module()
}

/// Reserved identifiers that may not be used as bindings.
pub const RESERVED: &[&str] = &[
    "behavior", "version", "caps", "self", "true", "false", "if", "else", "let", "return", "fn",
    "spawn", "move", "damage", "trigger", "dialogue", "sequence", "selector", "parallel", "invert",
    "retry", "timeout",
];

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn bump(&mut self) -> Token {
        let tok = self.tokens[self.pos].clone();
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    fn eat_ident(&mut self, keyword: &str) -> BehaviorDslResult<Token> {
        let tok = self.peek().clone();
        match &tok.kind {
            TokenKind::Ident(s) if s == keyword => {
                self.bump();
                Ok(tok)
            }
            _ => Err(self.unexpected(keyword)),
        }
    }

    fn eat(&mut self, target: &TokenKind) -> BehaviorDslResult<Token> {
        let tok = self.peek().clone();
        if std::mem::discriminant(&tok.kind) == std::mem::discriminant(target) {
            self.bump();
            Ok(tok)
        } else {
            Err(self.unexpected(&format_kind(target)))
        }
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Eof)
    }

    fn unexpected(&self, expected: &str) -> BehaviorDslError {
        let tok = self.peek();
        if matches!(tok.kind, TokenKind::Eof) {
            BehaviorDslError::UnexpectedEof {
                expected: expected.to_string(),
            }
        } else {
            BehaviorDslError::UnexpectedToken {
                found: format_kind(&tok.kind),
                expected: expected.to_string(),
                span: tok.span,
            }
        }
    }

    fn parse_module(&mut self) -> BehaviorDslResult<Module> {
        // "behavior" NAME "{" header body "}"
        let start = self.peek().span;
        match &self.peek().kind {
            TokenKind::Ident(s) if s == "behavior" => {
                self.bump();
            }
            _ => return Err(BehaviorDslError::MissingModuleHeader),
        }
        let name = self.expect_ident("behavior name")?;
        self.eat(&TokenKind::LBrace)?;

        // Optional @caps(...)
        let mut caps = CapabilitySet::new();
        if matches!(self.peek().kind, TokenKind::At) {
            self.bump(); // @
            self.eat_ident("caps")?;
            self.eat(&TokenKind::LParen)?;
            if !matches!(self.peek().kind, TokenKind::RParen) {
                loop {
                    let tok = self.peek().clone();
                    let (name, span) = match &tok.kind {
                        TokenKind::Ident(s) => (s.clone(), tok.span),
                        _ => return Err(self.unexpected("capability name")),
                    };
                    self.bump();
                    let cap = Capability::from_name(&name).ok_or(
                        BehaviorDslError::UnknownCapability { name, span },
                    )?;
                    caps.insert(cap);
                    if matches!(self.peek().kind, TokenKind::Comma) {
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
            self.eat(&TokenKind::RParen)?;
        }

        // Required: version <int>
        let v_tok = self.peek().clone();
        match &v_tok.kind {
            TokenKind::Ident(s) if s == "version" => {
                self.bump();
            }
            _ => {
                return Err(BehaviorDslError::MissingVersion { span: v_tok.span });
            }
        }
        let version = match &self.peek().kind {
            TokenKind::Int(n) if *n >= 0 => {
                let v = *n as u32;
                self.bump();
                v
            }
            _ => {
                return Err(self.unexpected("non-negative integer version"));
            }
        };

        // Root node.
        let root = self.parse_node()?;

        self.eat(&TokenKind::RBrace)?;
        let end = if self.pos == 0 {
            start
        } else {
            self.tokens[self.pos - 1].span
        };
        let span = start.join(end);
        if !self.at_eof() {
            return Err(self.unexpected("end of file"));
        }
        Ok(Module {
            name,
            caps,
            version,
            root,
            span,
        })
    }

    fn expect_ident(&mut self, what: &str) -> BehaviorDslResult<String> {
        let tok = self.peek().clone();
        match &tok.kind {
            TokenKind::Ident(s) => {
                self.bump();
                Ok(s.clone())
            }
            _ => Err(self.unexpected(what)),
        }
    }

    fn parse_node(&mut self) -> BehaviorDslResult<Node> {
        let tok = self.peek().clone();
        let TokenKind::Ident(name) = &tok.kind else {
            return Err(self.unexpected("verb or combinator"));
        };
        let start = tok.span;

        // Combinator?
        if let Some(comb) = Combinator::from_name(name) {
            self.bump();
            let parameter = match comb {
                Combinator::Retry | Combinator::Timeout => {
                    self.eat(&TokenKind::LParen)?;
                    let t = self.peek().clone();
                    let TokenKind::Int(n) = t.kind else {
                        return Err(self.unexpected("integer parameter"));
                    };
                    self.bump();
                    if matches!(comb, Combinator::Retry) && n <= 0 {
                        return Err(BehaviorDslError::RetryNonPositive {
                            value: n,
                            span: t.span,
                        });
                    }
                    self.eat(&TokenKind::RParen)?;
                    Some(n)
                }
                _ => None,
            };
            self.eat(&TokenKind::LBrace)?;
            let mut children = Vec::new();
            while !matches!(self.peek().kind, TokenKind::RBrace) {
                let child = self.parse_node()?;
                children.push(child);
                // Optional trailing semicolon between nodes.
                if matches!(self.peek().kind, TokenKind::Semi) {
                    self.bump();
                }
            }
            let close = self.eat(&TokenKind::RBrace)?;
            if children.is_empty() {
                return Err(BehaviorDslError::EmptyBody {
                    span: start.join(close.span),
                });
            }
            return Ok(Node {
                kind: NodeKind::Combinator {
                    combinator: comb,
                    parameter,
                    children,
                },
                span: start.join(close.span),
            });
        }

        // Verb?
        if let Some(verb) = Verb::from_name(name) {
            self.bump();
            self.eat(&TokenKind::LParen)?;
            let mut args = Vec::new();
            if !matches!(self.peek().kind, TokenKind::RParen) {
                loop {
                    let expr = self.parse_expr()?;
                    args.push(expr);
                    if matches!(self.peek().kind, TokenKind::Comma) {
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
            let close = self.eat(&TokenKind::RParen)?;
            // Optional semicolon after a verb call.
            if matches!(self.peek().kind, TokenKind::Semi) {
                self.bump();
            }
            return Ok(Node {
                kind: NodeKind::Verb { verb, args },
                span: start.join(close.span),
            });
        }

        // Otherwise: unknown — prefer combinator error if it looks like one.
        if [
            "seq", "sel", "par", "inv", "repeat", "retries", "until",
        ]
        .contains(&name.as_str())
        {
            return Err(BehaviorDslError::UnknownCombinator {
                name: name.clone(),
                span: start,
            });
        }
        Err(BehaviorDslError::UnknownVerb {
            name: name.clone(),
            span: start,
        })
    }

    fn parse_expr(&mut self) -> BehaviorDslResult<Expr> {
        let tok = self.peek().clone();
        match tok.kind {
            TokenKind::Int(n) => {
                self.bump();
                Ok(Expr::literal(Literal::Int(n), tok.span))
            }
            TokenKind::Float(f) => {
                self.bump();
                Ok(Expr::literal(Literal::Float(f), tok.span))
            }
            TokenKind::True => {
                self.bump();
                Ok(Expr::literal(Literal::Bool(true), tok.span))
            }
            TokenKind::False => {
                self.bump();
                Ok(Expr::literal(Literal::Bool(false), tok.span))
            }
            TokenKind::String(ref s) => {
                let value = s.clone();
                self.bump();
                Ok(Expr::literal(Literal::String(value), tok.span))
            }
            TokenKind::LBracket => {
                self.bump();
                let mut elements = Vec::new();
                if !matches!(self.peek().kind, TokenKind::RBracket) {
                    loop {
                        elements.push(self.parse_expr()?);
                        if matches!(self.peek().kind, TokenKind::Comma) {
                            self.bump();
                        } else {
                            break;
                        }
                    }
                }
                let close = self.eat(&TokenKind::RBracket)?;
                Ok(Expr::literal(
                    Literal::List(elements),
                    tok.span.join(close.span),
                ))
            }
            TokenKind::LBrace => {
                self.bump();
                let mut entries = Vec::new();
                if !matches!(self.peek().kind, TokenKind::RBrace) {
                    loop {
                        let key_tok = self.peek().clone();
                        let key = match key_tok.kind {
                            TokenKind::String(s) => s,
                            _ => return Err(self.unexpected("string key")),
                        };
                        self.bump();
                        self.eat(&TokenKind::Colon)?;
                        let value = self.parse_expr()?;
                        entries.push((key, value));
                        if matches!(self.peek().kind, TokenKind::Comma) {
                            self.bump();
                        } else {
                            break;
                        }
                    }
                }
                let close = self.eat(&TokenKind::RBrace)?;
                Ok(Expr::literal(
                    Literal::Map(entries),
                    tok.span.join(close.span),
                ))
            }
            TokenKind::Ident(ref name) => {
                let name = name.clone();
                self.bump();
                if name == "vec3" {
                    self.eat(&TokenKind::LParen)?;
                    let x = self.parse_expr()?;
                    self.eat(&TokenKind::Comma)?;
                    let y = self.parse_expr()?;
                    self.eat(&TokenKind::Comma)?;
                    let z = self.parse_expr()?;
                    let close = self.eat(&TokenKind::RParen)?;
                    Ok(Expr::literal(
                        Literal::Vec3(Box::new(x), Box::new(y), Box::new(z)),
                        tok.span.join(close.span),
                    ))
                } else if name == "option" {
                    self.eat(&TokenKind::LParen)?;
                    let label = self.parse_expr()?;
                    self.eat(&TokenKind::Comma)?;
                    let id = self.parse_expr()?;
                    let close = self.eat(&TokenKind::RParen)?;
                    let span = tok.span.join(close.span);
                    Ok(Expr {
                        kind: ExprKind::DialogueOption(DialogueOption {
                            label: Box::new(label),
                            id: Box::new(id),
                            span,
                        }),
                        span,
                    })
                } else {
                    Ok(Expr::ident(name, tok.span))
                }
            }
            TokenKind::Eof => Err(BehaviorDslError::UnexpectedEof {
                expected: "expression".to_string(),
            }),
            _ => Err(self.unexpected("expression")),
        }
    }
}

fn format_kind(kind: &TokenKind) -> String {
    match kind {
        TokenKind::Ident(s) => s.clone(),
        TokenKind::Int(n) => n.to_string(),
        TokenKind::Float(f) => f.to_string(),
        TokenKind::String(s) => format!("\"{}\"", s),
        TokenKind::LParen => "(".to_string(),
        TokenKind::RParen => ")".to_string(),
        TokenKind::LBrace => "{".to_string(),
        TokenKind::RBrace => "}".to_string(),
        TokenKind::LBracket => "[".to_string(),
        TokenKind::RBracket => "]".to_string(),
        TokenKind::Comma => ",".to_string(),
        TokenKind::Semi => ";".to_string(),
        TokenKind::Colon => ":".to_string(),
        TokenKind::Dot => ".".to_string(),
        TokenKind::At => "@".to_string(),
        TokenKind::True => "true".to_string(),
        TokenKind::False => "false".to_string(),
        TokenKind::Eof => "<eof>".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_module() {
        let src = "behavior Empty { version 1 sequence { trigger(\"hello\", {}); } }";
        let m = parse(src).unwrap();
        assert_eq!(m.name, "Empty");
        assert_eq!(m.version, 1);
        match &m.root.kind {
            NodeKind::Combinator { combinator, .. } => {
                assert_eq!(*combinator, Combinator::Sequence);
            }
            _ => panic!("expected sequence"),
        }
    }

    #[test]
    fn parse_module_with_caps() {
        let src = "behavior Foo { @caps(Movement, Economy) version 2 spawn(\"coin\", vec3(0,0,0)); }";
        let m = parse(src).unwrap();
        assert!(m.caps.contains(Capability::Movement));
        assert!(m.caps.contains(Capability::Economy));
    }

    #[test]
    fn missing_module_header_reports_cleanly() {
        let err = parse("// nothing").unwrap_err();
        assert_eq!(err.code(), "BDSL-E0015");
    }

    #[test]
    fn unknown_combinator_flagged() {
        let src = "behavior Foo { version 1 until { spawn(\"x\", vec3(0,0,0)); } }";
        let err = parse(src).unwrap_err();
        assert_eq!(err.code(), "BDSL-E0018");
    }

    #[test]
    fn retry_with_zero_is_error() {
        let src = "behavior Foo { version 1 retry(0) { spawn(\"x\", vec3(0,0,0)); } }";
        let err = parse(src).unwrap_err();
        assert_eq!(err.code(), "BDSL-E0020");
    }

    #[test]
    fn empty_body_is_error() {
        let src = "behavior Foo { version 1 sequence { } }";
        let err = parse(src).unwrap_err();
        assert_eq!(err.code(), "BDSL-E0019");
    }
}
