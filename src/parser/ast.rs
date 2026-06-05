#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Comp(Box<Expr>, Box<Expr>),
    Tensor(Box<Expr>, Box<Expr>),
    Id(u32),
    Swap(u32, u32),
    Gen { name: String, args: Vec<u32> },
}
