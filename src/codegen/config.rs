use serde::Deserialize;
use std::collections::HashMap;

use crate::typechecker::Sig;

#[derive(Debug, Deserialize)]
struct Config {
    generators: HashMap<String, Generator>,
}

fn default_one() -> f32 {
    1.0
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Generator {
    pub sig: Sig,
    #[serde(default)]
    pub params: Vec<String>,
    pub visual_arity: Option<u32>,
    pub visual_coarity: Option<u32>,
    #[serde(default)]
    pub symbol: String,
    #[serde(default)]
    pub pic: String,
    #[serde(default = "default_one")]
    pub width: f32,
    #[serde(default = "default_one")]
    pub height: f32,
}

pub type Env = HashMap<String, Generator>;

pub fn load_config(path: &str) -> Result<Env, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("failed to read config: {}", e))?;
    parse_config(&content)
}

fn parse_config(content: &str) -> Result<Env, String> {
    let config: Config = toml::from_str(content).map_err(|e| format!("invalid config: {}", e))?;

    let mut env = Env::default();
    for (name, raw) in config.generators {
        env.insert(
            name.clone(),
            Generator {
                sig: raw.sig,
                params: raw.params,
                visual_arity: raw.visual_arity,
                visual_coarity: raw.visual_coarity,
                symbol: raw.symbol,
                pic: raw.pic,
                width: raw.width,
                height: raw.height,
            },
        );
    }
    Ok(env)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_generator() {
        let toml = r#"
        [generators.mult]
        sig.arity = 2
        sig.coarity = 1
        "#;

        let env = parse_config(toml).unwrap();
        assert_eq!(env.get("mult").unwrap().sig.arity, 2);
        assert_eq!(env.get("mult").unwrap().sig.coarity, 1);
    }
    #[test]
    fn defaults() {
        let toml = r#"
        [generators.foo]
        sig.arity = 0
        sig.coarity = 1
        "#;
        let env = parse_config(toml).unwrap();
        let generator = env.get("foo").unwrap();

        assert_eq!(generator.pic, ""); // default
        assert_eq!(generator.width, 1.0); // default
        assert_eq!(generator.height, 1.0); // default
        assert!(generator.params.is_empty()); // default
        assert_eq!(generator.symbol, ""); // default
        assert_eq!(generator.visual_arity, None); // absent
        assert_eq!(generator.visual_coarity, None); // absent
    }

    #[test]
    fn pic_defaults_to_empty() {
        let toml = r#"
        [generators.foo]
        sig.arity = 0
        sig.coarity = 1
        "#;
        assert!(
            parse_config(toml)
                .unwrap()
                .get("foo")
                .unwrap()
                .pic
                .is_empty()
        );
    }

    #[test]
    fn invalid_toml_errors() {
        assert!(parse_config("garbage [[[").is_err());
    }
}
