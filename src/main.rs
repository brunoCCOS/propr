use clap::Parser;
use propr::{codegen::config, compile};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Expression to compile
    expr: String,

    /// Path to generators.toml
    #[arg(short, long)]
    config: Option<String>,
}

fn main() {
    let args = Args::parse();

    let env: config::Env = match args.config {
        Some(ref path) => config::load_config(path).unwrap_or_else(|e| {
            eprintln!("propr: config error: {}", e);
            std::process::exit(1);
        }),
        None => config::Env::default(),
    };

    let expr = &args.expr;

    match compile(expr, &env) {
        Ok(tikz) => {
            print!("{}", tikz);
        }
        Err(err) => {
            eprintln!("propr: {}", err);
            std::process::exit(1);
        }
    }
}
