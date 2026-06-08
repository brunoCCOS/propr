use serde::Deserialize;

use crate::codegen::config::Env;
use crate::parser::ast;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Sig {
    pub arity: u32,
    pub coarity: u32,
}

pub fn check(expr: &ast::Expr, env: &Env) -> Result<Sig, String> {
    match expr {
        ast::Expr::Id(n) => Ok(Sig {
            arity: *n,
            coarity: *n,
        }),
        ast::Expr::Swap(n, m) => Ok(Sig {
            arity: n + m,
            coarity: n + m,
        }),
        ast::Expr::Gen { name, args: _ } => {
            let generator = env
                .get(name)
                .ok_or_else(|| format!("unknown generator: {}", name))?;
            Ok(Sig {
                arity: generator.sig.arity,
                coarity: generator.sig.coarity,
            })
        }
        ast::Expr::Tensor(left, right) => {
            let left_sig = check(left, env)?;
            let right_sig = check(right, env)?;
            Ok(Sig {
                arity: left_sig.arity + right_sig.arity,
                coarity: left_sig.coarity + right_sig.coarity,
            })
        }
        ast::Expr::Comp(left, right) => {
            let left_sig = check(left, env)?;
            let right_sig = check(right, env)?;
            if left_sig.coarity != right_sig.arity {
                return Err(format!(
                    "composition mismatch: left has coarity {}, right has arity {}",
                    left_sig.coarity, right_sig.arity
                ));
            }
            Ok(Sig {
                arity: left_sig.arity,
                coarity: right_sig.coarity,
            })
        }
    }
}
