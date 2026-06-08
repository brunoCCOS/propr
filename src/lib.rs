pub mod codegen;
pub mod lexer;
pub mod parser;
pub mod typechecker;

pub fn compile(expr: &str, env: &codegen::config::Env) -> Result<String, String> {
    let ast = parser::engine::parse(expr)?;
    typechecker::check(&ast, env)?;
    codegen::renderer::generate(&ast, env)
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
}
