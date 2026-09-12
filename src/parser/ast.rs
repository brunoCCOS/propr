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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelOp {
    Eq,
    Subset,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Equation {
    pub sides: Vec<Expr>,
    pub ops: Vec<RelOp>,
}

impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::Comp(l, r) => write!(f, "{} ; {}", l, r),
            Expr::Tensor(l, r) => {
                write_tensor_operand(f, l)?;
                write!(f, " * ")?;
                write_tensor_operand(f, r)
            }
            Expr::Id(n) => write!(f, "id({})", n),
            Expr::Swap(m, n) => write!(f, "swap({},{})", m, n),
            Expr::Gen { name, args } => {
                write!(f, "{}", name)?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ",")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
        }
    }
}

fn write_tensor_operand(f: &mut std::fmt::Formatter<'_>, expr: &Expr) -> std::fmt::Result {
    if matches!(expr, Expr::Comp(_, _)) {
        write!(f, "({})", expr)
    } else {
        write!(f, "{}", expr)
    }
}

#[cfg(test)]
mod display_tests {
    use super::*;
    use crate::parser::engine::parse;

    #[test]
    fn display_comp_and_tensor_minimal_parens() {
        // ';' binds looser than '*': a Comp nested inside a Tensor operand
        // needs parens; the reverse (Tensor inside Comp) does not.
        let expr = parse("a ; (b * c)").unwrap();
        assert_eq!(format!("{}", expr), "a ; b * c");

        let expr = parse("(a ; b) * c").unwrap();
        assert_eq!(format!("{}", expr), "(a ; b) * c");
    }

    #[test]
    fn display_id_swap_gen() {
        assert_eq!(format!("{}", Expr::Id(3)), "id(3)");
        assert_eq!(format!("{}", Expr::Swap(1, 2)), "swap(1,2)");
        assert_eq!(
            format!(
                "{}",
                Expr::Gen {
                    name: "f".into(),
                    args: vec![Arg::Number(2), Arg::Number(3)]
                }
            ),
            "f(2,3)"
        );
    }
}
