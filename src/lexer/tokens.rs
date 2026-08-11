#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Number(i32),
    Letter(char),
    Ident(String),
    Id,
    Swap,
    Lparen,
    Rparen,
    Comma,
    Comp,
    Tensor,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub pos: usize,
}
