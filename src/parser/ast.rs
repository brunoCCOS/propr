#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arg {
    Number(i32),
    Letter(char),
}

impl Arg {
    pub fn to_u32(&self) -> Option<u32> {
        match self {
            Arg::Number(val) => u32::try_from(*val).ok(),
            Arg::Letter(_) => None,
        }
    }
}

impl std::fmt::Display for Arg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Arg::Number(val) => write!(f, "{}", val),
            Arg::Letter(ch) => write!(f, "{}", ch),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Comp(Box<Expr>, Box<Expr>),
    Tensor(Box<Expr>, Box<Expr>),
    Id(u32),
    Swap(u32, u32),
    Gen { name: String, args: Vec<Arg> },
}
