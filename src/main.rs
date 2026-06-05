use propr::{compile, typechecker::Env};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: propr <expression>");
        std::process::exit(1);
    }

    let expr = args.join(" ");
    let env = Env::default();

    match compile(&expr, &env) {
        Ok(tikz) => {
            print!("{}", tikz);
        }
        Err(err) => {
            eprintln!("propr: {}", err);
            std::process::exit(1);
        }
    }
}
