use crate::lexer::tokens::{Token, TokenKind};

pub struct Lexer {
    src: Vec<char>,
    pos: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Lexer {
        Lexer {
            src: input.chars().collect(),
            pos: 0,
        }
    }

    pub fn peek(&self) -> Option<char> {
        if self.pos >= self.src.len() {
            None
        } else {
            Some(self.src[self.pos])
        }
    }

    pub fn advance(&mut self) -> char {
        let c = self.src[self.pos];
        self.pos += 1;
        c
    }

    pub fn skip_whitespace(&mut self) {
        while self.pos < self.src.len() && self.src[self.pos].is_whitespace() {
            self.pos += 1;
        }
    }

    pub fn advance_token(&mut self) -> Result<Token, String> {
        self.skip_whitespace();
        let start = self.pos;

        if self.pos >= self.src.len() {
            return Ok(Token {
                kind: TokenKind::Eof,
                pos: start,
            });
        };

        let c = match self.peek() {
            Some(c) => c,
            None => {
                return Ok(Token {
                    kind: TokenKind::Eof,
                    pos: start,
                });
            }
        };

        match c {
            '(' => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::Lparen,
                    pos: start,
                })
            }
            ')' => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::Rparen,
                    pos: start,
                })
            }
            ';' => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::Comp,
                    pos: start,
                })
            }
            '*' => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::Tensor,
                    pos: start,
                })
            }
            ',' => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::Comma,
                    pos: start,
                })
            }
            c if c.is_ascii_digit() => {
                while self.pos < self.src.len() && self.src[self.pos].is_ascii_digit() {
                    self.pos += 1;
                }
                let n = self.src[start..self.pos]
                    .iter()
                    .collect::<String>()
                    .parse::<u32>()
                    .unwrap();
                Ok(Token {
                    kind: TokenKind::Number(n as i32),
                    pos: start,
                })
            }
            c if c.is_alphabetic() || c == '_' => {
                while self.pos < self.src.len()
                    && (self.src[self.pos].is_alphabetic()
                        || self.src[self.pos].is_ascii_digit()
                        || self.src[self.pos] == '_')
                {
                    self.pos += 1;
                }
                let word = self.src[start..self.pos].iter().collect::<String>();
                let kind = match word.as_str() {
                    "id" => TokenKind::Id,
                    "swap" => TokenKind::Swap,
                    // A single alphabetic character becomes Letter.
                    _ if word.chars().count() == 1 => {
                        TokenKind::Letter(word.chars().next().unwrap())
                    }
                    _ => TokenKind::Ident(word),
                };
                Ok(Token { kind, pos: start })
            }
            other => Err(format!(
                "unexpected character {:?} at position {}",
                other, start
            )),
        }
    }

    pub fn tokenize(&mut self) -> Vec<Token> {
        let mut out: Vec<Token> = Vec::new();
        loop {
            let t = self.advance_token().expect("lex error");
            let is_eof = t.kind == TokenKind::Eof;
            out.push(t);
            if is_eof {
                return out;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Lex an input and return just the kinds, like Go's kinds() helper.
    fn kinds(input: &str) -> Vec<TokenKind> {
        Lexer::new(input)
            .tokenize()
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    #[test]
    fn tokenize_basic() {
        use TokenKind::*;

        assert_eq!(kinds("id(2)"), vec![Id, Lparen, Number(2), Rparen, Eof]);
        assert_eq!(
            kinds("swap(1,3)"),
            vec![Swap, Lparen, Number(1), Comma, Number(3), Rparen, Eof]
        );
        assert_eq!(
            kinds("f ; g"),
            vec![Ident("f".into()), Comp, Ident("g".into()), Eof]
        );
        assert_eq!(
            kinds("f * g"),
            vec![Ident("f".into()), Tensor, Ident("g".into()), Eof]
        );
        assert_eq!(
            kinds("(f;g)*h"),
            vec![
                Lparen,
                Ident("f".into()),
                Comp,
                Ident("g".into()),
                Rparen,
                Tensor,
                Ident("h".into()),
                Eof
            ]
        );
        assert_eq!(kinds("  \t\n"), vec![Eof]);
    }

    #[test]
    fn tokenize_ident_values() {
        use TokenKind::*;
        assert_eq!(
            kinds("multiplication foo_bar x1"),
            vec![
                Ident("multiplication".into()),
                Ident("foo_bar".into()),
                Ident("x1".into()),
                Eof
            ]
        );
    }

    #[test]
    #[should_panic]
    fn tokenize_rejects_unknown_char() {
        Lexer::new("f @ g").tokenize();
    }
}
