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
        ast::Expr::Gen { name, args } => {
            let generator = env
                .get(name)
                .ok_or_else(|| format!("unknown generator: {}", name))?;
            if args.len() != generator.params.len() {
                return Err(format!(
                    "generator {} expects {} argument(s), got {}",
                    name,
                    generator.params.len(),
                    args.len()
                ));
            }
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

pub fn check_equation(equation: &ast::Equation, env: &Env) -> Result<Sig, String> {
    let mut sigs = Vec::with_capacity(equation.sides.len());
    for side in &equation.sides {
        sigs.push(check(side, env)?);
    }
    let first_sig = sigs[0];
    for (side, sig) in equation.sides.iter().zip(sigs.iter()) {
        if *sig != first_sig {
            return Err(format!(
                "equation side mismatch: {} (sig {}/{}) vs {} (sig {}/{})",
                equation.sides[0], first_sig.arity, first_sig.coarity, side, sig.arity, sig.coarity
            ));
        }
    }
    Ok(first_sig)
}

#[cfg(test)]
mod check_equation_tests {
    use super::*;
    use crate::codegen::config::{Env, Generator};
    use crate::parser::engine::parse_equation;

    fn make_generator(arity: u32, coarity: u32, params: Vec<&str>) -> Generator {
        Generator {
            sig: Sig { arity, coarity },
            params: params.into_iter().map(String::from).collect(),
            visual_arity: None,
            visual_coarity: None,
            symbol: String::new(),
            pic: String::new(),
            width: 1.0,
            height: 1.0,
        }
    }

    fn mult_copy_env() -> Env {
        let mut env = Env::default();
        env.insert("mult".into(), make_generator(2, 1, vec![]));
        env.insert("copy".into(), make_generator(1, 2, vec![]));
        env
    }

    #[test]
    fn equation_all_sides_matching_sig() {
        let env = mult_copy_env();
        let eq = parse_equation("copy ; mult = id(1)").unwrap();
        let sig = check_equation(&eq, &env).unwrap();
        assert_eq!(
            sig,
            Sig {
                arity: 1,
                coarity: 1
            }
        );
    }

    #[test]
    fn equation_mismatched_sigs_errors_with_both_sides() {
        let env = mult_copy_env();
        let eq = parse_equation("mult = copy").unwrap();
        let err = check_equation(&eq, &env).unwrap_err();
        assert!(err.contains("mult"), "error was: {}", err);
        assert!(err.contains("copy"), "error was: {}", err);
    }

    #[test]
    fn equation_chain_all_matching() {
        let env = Env::default();
        let eq = parse_equation("id(1) = id(1) ⊆ id(1)").unwrap();
        let sig = check_equation(&eq, &env).unwrap();
        assert_eq!(
            sig,
            Sig {
                arity: 1,
                coarity: 1
            }
        );
    }

    #[test]
    fn equation_chain_one_bad_side_errors() {
        let env = Env::default();
        let eq = parse_equation("id(1) = id(1) ⊆ id(2)").unwrap();
        assert!(check_equation(&eq, &env).is_err());
    }

    #[test]
    fn bare_expression_equation_checks_to_own_sig() {
        let env = Env::default();
        let eq = parse_equation("id(3)").unwrap();
        let sig = check_equation(&eq, &env).unwrap();
        assert_eq!(
            sig,
            Sig {
                arity: 3,
                coarity: 3
            }
        );
    }

    #[test]
    fn equation_side_composition_mismatch_propagates() {
        let env = mult_copy_env();
        let eq = parse_equation("id(3) ; mult = id(1)").unwrap();
        let err = check_equation(&eq, &env).unwrap_err();
        assert!(err.contains("composition mismatch"), "error was: {}", err);
    }

    #[test]
    fn gen_arg_count_mismatch_errors_with_expected_and_got() {
        let mut env = Env::default();
        env.insert("f".into(), make_generator(1, 1, vec!["n"]));
        let expr = crate::parser::engine::parse("f").unwrap();
        let err = check(&expr, &env).unwrap_err();
        assert!(err.contains('1'), "expected count missing: {}", err);
        assert!(err.contains('0'), "got count missing: {}", err);
        assert!(err.contains('f'), "generator name missing: {}", err);
    }

    #[test]
    fn gen_arg_count_matches_params_passes() {
        let mut env = Env::default();
        env.insert("f".into(), make_generator(1, 1, vec!["n"]));
        let expr = crate::parser::engine::parse("f(5)").unwrap();
        assert!(check(&expr, &env).is_ok());
    }

    #[test]
    fn gen_with_no_params_and_no_args_still_passes() {
        let mut env = Env::default();
        env.insert("mult".into(), make_generator(2, 1, vec![]));
        let expr = crate::parser::engine::parse("mult").unwrap();
        assert!(check(&expr, &env).is_ok());
    }
}
