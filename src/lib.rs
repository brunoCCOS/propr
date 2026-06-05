pub mod codegen;
pub mod lexer;
pub mod parser;
pub mod typechecker;

pub fn compile(expr: &str, env: &typechecker::Env) -> Result<String, String> {
    let ast = parser::parser::parse(expr)?;
    typechecker::check(&ast, env)?;
    codegen::generate(&ast, env)
}
