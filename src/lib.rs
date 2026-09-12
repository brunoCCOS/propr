pub mod codegen;
pub mod lexer;
pub mod parser;
pub mod typechecker;

pub fn compile(expr: &str, env: &codegen::config::Env) -> Result<String, String> {
    let equation = parser::engine::parse_equation(expr)?;
    typechecker::check_equation(&equation, env)?;
    codegen::renderer::generate_equation(&equation, env)
}

#[cfg(test)]
mod tests {
    use crate::codegen::config::{Env, Generator};
    use crate::typechecker::Sig;

    fn test_env() -> Env {
        let mut env = Env::default();
        env.insert(
            "mult".into(),
            Generator {
                sig: Sig {
                    arity: 2,
                    coarity: 1,
                },
                params: vec![],
                visual_arity: None,
                visual_coarity: None,
                symbol: String::new(),
                pic: "multiplication".into(),
                width: 1.0,
                height: 1.0,
            },
        );
        env.insert(
            "copy".into(),
            Generator {
                sig: Sig {
                    arity: 1,
                    coarity: 2,
                },
                params: vec![],
                visual_arity: None,
                visual_coarity: None,
                symbol: String::new(),
                pic: "copy".into(),
                width: 1.0,
                height: 1.0,
            },
        );
        env
    }

    #[test]
    fn compile_id() {
        let out = super::compile("id(1)", &test_env()).unwrap();
        assert!(out.contains("\\begin{tikzpicture}"));
        assert!(out.contains("\\end{tikzpicture}"));
    }

    #[test]
    fn compile_swap() {
        let out = super::compile("swap(1,2)", &test_env()).unwrap();
        assert!(out.contains("\\begin{tikzpicture}"));
    }

    #[test]
    fn compile_generator() {
        let out = super::compile("mult", &test_env()).unwrap();
        assert!(out.contains("multiplication"));
    }

    #[test]
    fn compile_composition_mismatch() {
        let err = super::compile("id(3) ; mult", &test_env()).unwrap_err();
        assert!(err.contains("composition mismatch"));
    }

    #[test]
    fn compile_unknown_generator() {
        let err = super::compile("nonexistent", &test_env()).unwrap_err();
        assert!(err.contains("unknown generator"));
    }

    #[test]
    fn compile_tensor() {
        let out = super::compile("mult * copy", &test_env()).unwrap();
        assert!(out.contains("multiplication"));
        assert!(out.contains("copy"));
    }

    #[test]
    fn compile_empty_env_no_custom() {
        let env = Env::default();
        let out = super::compile("id(1)", &env).unwrap();
        assert!(out.contains("\\begin{tikzpicture}"));
    }

    #[test]
    fn compile_syntax_error() {
        let err = super::compile("id(", &test_env()).unwrap_err();
        assert!(!err.is_empty());
    }

    // (2) Subset relation renders "\subseteq"; the ASCII alias "<=" produces
    // an identical relation symbol.
    #[test]
    fn compile_subset_relation_renders_subseteq() {
        let out = super::compile("id(1) ⊆ id(1)", &test_env()).unwrap();
        assert!(out.contains("\\subseteq"), "got: {}", out);
    }

    #[test]
    fn compile_ascii_subset_alias_matches_unicode() {
        let unicode_out = super::compile("id(1) ⊆ id(1)", &test_env()).unwrap();
        let ascii_out = super::compile("id(1) <= id(1)", &test_env()).unwrap();
        let extract_symbol = |s: &str| -> bool { s.contains("\\subseteq") };
        assert_eq!(
            extract_symbol(&unicode_out),
            extract_symbol(&ascii_out),
            "ascii <= alias should render the same relation symbol as ⊆: {} vs {}",
            unicode_out,
            ascii_out
        );
        assert!(extract_symbol(&ascii_out), "got: {}", ascii_out);
    }

    // (5) Typecheck runs before rendering: equation-mismatch errors surface
    // through compile(), unknown-generator errors surface for equations too,
    // and a bare expression (equation with one side, zero ops) still works
    // and has no relation node.
    #[test]
    fn compile_equation_sig_mismatch_errors() {
        let err = super::compile("mult = copy", &test_env()).unwrap_err();
        assert!(!err.is_empty(), "expected a sig-mismatch error");
    }

    #[test]
    fn compile_equation_unknown_generator_errors() {
        let env = Env::default();
        let err = super::compile("f = g", &env).unwrap_err();
        assert!(err.contains("unknown generator"), "got: {}", err);
    }

    #[test]
    fn compile_bare_expression_still_works_with_no_relation_node() {
        let out = super::compile("copy ; mult", &test_env()).unwrap();
        assert!(out.contains("\\begin{tikzpicture}"));
        assert!(!out.contains("$=$"));
        assert!(!out.contains("\\subseteq"));
    }

    // (6) Equation sig-check precedes rendering: a mismatched-sig equation
    // must fail with Err, never produce TikZ output.
    #[test]
    fn compile_equation_sig_check_precedes_rendering() {
        let result = super::compile("id(2) = id(1)", &test_env());
        assert!(
            result.is_err(),
            "expected sig-check to reject id(2) = id(1) before rendering, got: {:?}",
            result
        );
    }
}
