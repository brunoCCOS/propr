pub mod codegen;
pub mod lexer;
pub mod parser;
pub mod typechecker;

pub fn compile(expr: &str, env: &codegen::config::Env) -> Result<String, String> {
    let ast = parser::engine::parse(expr)?;
    typechecker::check(&ast, env)?;
    codegen::renderer::generate(&ast, env)
}
