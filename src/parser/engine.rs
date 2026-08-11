use crate::{
    lexer::{
        scan::Lexer,
        tokens::{Token, TokenKind},
    },
    parser::ast::{Arg, Expr},
};

struct Parser {
    toks: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser {
            toks: tokens,
            pos: 0,
        }
    }

    fn peek(&self) -> &Token {
        &self.toks[self.pos]
    }

    fn advance(&mut self) {
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
    }

    pub fn parse_expr(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_term()?;
        while self.peek().kind == TokenKind::Comp {
            self.advance();
            let right = self.parse_term()?;
            left = Expr::Comp(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_term(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_atom()?;
        while self.peek().kind == TokenKind::Tensor {
            self.advance();
            let right = self.parse_atom()?;
            left = Expr::Tensor(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_atom(&mut self) -> Result<Expr, String> {
        let kind = self.peek().kind.clone();
        let pos = self.peek().pos;
        match kind {
            TokenKind::Id => {
                self.advance();
                let args = self.parse_fixed_args(1)?;
                Ok(Expr::Id(args[0].to_u32().expect(
                    "Argument for identity should be a positive integer",
                )))
            }
            TokenKind::Swap => {
                self.advance();
                let args = self.parse_fixed_args(2)?;
                let arg0 = args[0]
                    .to_u32()
                    .expect("Argument for swap should be a positive integer");
                let arg1 = args[1]
                    .to_u32()
                    .expect("Argument for swap should be a positive integer");
                Ok(Expr::Swap(arg0, arg1))
            }
            TokenKind::Ident(name) => {
                self.advance();
                let args = if self.peek().kind == TokenKind::Lparen {
                    self.parse_variadic_args()?
                } else {
                    Vec::new()
                };
                Ok(Expr::Gen { name, args })
            }
            TokenKind::Lparen => {
                self.advance();
                let inner = self.parse_expr()?;
                if self.peek().kind != TokenKind::Rparen {
                    return Err(format!("expected ')' at position {}", self.peek().pos));
                }
                self.advance();
                Ok(inner)
            }
            other => Err(format!("unexpected token {:?} at position {}", other, pos)),
        }
    }

    fn parse_fixed_args(&mut self, n: usize) -> Result<Vec<Arg>, String> {
        if self.peek().kind != TokenKind::Lparen {
            return Err(format!("expected '(' at position {}", self.peek().pos));
        }
        self.advance();

        let mut out = Vec::with_capacity(n);

        for i in 0..n {
            if i > 0 {
                if self.peek().kind != TokenKind::Comma {
                    return Err(format!("expected ',' at position {}", self.peek().pos));
                }
                self.advance();
            }

            let token = self.peek();

            let arg = match &token.kind {
                TokenKind::Number(value) => Arg::Number(*value),
                TokenKind::Letter(value) => Arg::Letter(*value),
                kind => {
                    return Err(format!(
                        "expected number or letter at position {}, got {:?}",
                        token.pos, kind
                    ));
                }
            };
            out.push(arg);
            self.advance();
        }

        if self.peek().kind != TokenKind::Rparen {
            return Err(format!("expected ')' at position {}", self.peek().pos));
        }

        self.advance();
        Ok(out)
    }

    // '(' NUMBER (',' NUMBER)* ')' — any count (for generator calls).
    fn parse_variadic_args(&mut self) -> Result<Vec<Arg>, String> {
        if self.peek().kind != TokenKind::Lparen {
            return Err(format!("expected '(' at position {}", self.peek().pos));
        }
        self.advance();

        let mut out = Vec::new();
        let mut i = 0;
        loop {
            if i > 0 {
                if self.peek().kind != TokenKind::Comma {
                    break;
                }
                self.advance();
            }
            let token = self.peek();

            let arg = match &token.kind {
                TokenKind::Number(value) => Arg::Number(*value),
                TokenKind::Letter(value) => Arg::Letter(*value),
                kind => {
                    return Err(format!(
                        "expected number or letter at position {}, got {:?}",
                        token.pos, kind
                    ));
                }
            };
            out.push(arg);
            self.advance();
            i += 1;
        }

        if self.peek().kind != TokenKind::Rparen {
            return Err(format!("expected ')' at position {}", self.peek().pos));
        }
        self.advance();
        Ok(out)
    }
}

pub fn parse(input: &str) -> Result<Expr, String> {
    let tokens = Lexer::new(input).tokenize();
    let mut p = Parser::new(tokens);
    let expr = p.parse_expr()?;
    if p.peek().kind != TokenKind::Eof {
        return Err(format!(
            "unexpected token {:?} at position {}",
            p.peek().kind,
            p.peek().pos
        ));
    }
    Ok(expr)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generator(name: &str) -> Expr {
        Expr::Gen {
            name: name.into(),
            args: Vec::new(),
        }
    }
    fn comp(l: Expr, r: Expr) -> Expr {
        Expr::Comp(Box::new(l), Box::new(r))
    }
    fn tensor(l: Expr, r: Expr) -> Expr {
        Expr::Tensor(Box::new(l), Box::new(r))
    }

    #[test]
    fn parse_atoms() {
        assert_eq!(parse("id(3)").unwrap(), Expr::Id(3));
        assert_eq!(parse("swap(1,2)").unwrap(), Expr::Swap(1, 2));
        assert_eq!(parse("foo").unwrap(), generator("foo"));
    }

    #[test]
    fn parse_precedence() {
        assert_eq!(
            parse("a * b ; c * d").unwrap(),
            comp(
                tensor(generator("a"), generator("b")),
                tensor(generator("c"), generator("d"))
            )
        );
    }

    #[test]
    fn parse_left_assoc() {
        assert_eq!(
            parse("a ; b ; c").unwrap(),
            comp(comp(generator("a"), generator("b")), generator("c"))
        );
    }

    #[test]
    fn parse_parens() {
        assert_eq!(
            parse("a ; (b ; c)").unwrap(),
            comp(generator("a"), comp(generator("b"), generator("c")))
        );
    }

    #[test]
    fn parse_errors() {
        for bad in ["id", "id(", "id()", "swap(1)", "(a;b", "a;", ";a", "a b"] {
            assert!(parse(bad).is_err(), "{bad:?}: expected error");
        }
    }
}
