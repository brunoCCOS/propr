use propr::codegen::config::{Env, Generator};
use propr::typechecker::Sig;

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

/// Compares `actual` against the committed golden file `tests/golden/<name>.tikz`.
/// Set `UPDATE_GOLDEN=1` to rewrite the golden file with `actual` instead of
/// asserting equality (used to regenerate snapshots after an intentional
/// output change).
fn assert_golden(name: &str, actual: &str) {
    let path = format!("{}/tests/golden/{}.tikz", env!("CARGO_MANIFEST_DIR"), name);
    if std::env::var("UPDATE_GOLDEN").is_ok() {
        std::fs::write(&path, actual).expect("failed to write golden file");
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read golden file {}: {}", path, e));
    assert_eq!(
        expected, actual,
        "output for `{}` does not match golden file {}.\n\
         If this change is intentional, regenerate with:\n\
         UPDATE_GOLDEN=1 rtk cargo test --test golden {}",
        name, path, name
    );
}

#[test]
fn golden_id_3() {
    let out = propr::compile("id(3)", &test_env()).unwrap();
    assert_golden("id_3", &out);
}

#[test]
fn golden_swap_1_2() {
    let out = propr::compile("swap(1,2)", &test_env()).unwrap();
    assert_golden("swap_1_2", &out);
}

#[test]
fn golden_copy_seq_mult() {
    let out = propr::compile("copy ; mult", &test_env()).unwrap();
    assert_golden("copy_seq_mult", &out);
}

#[test]
fn golden_mult_tensor_copy() {
    let out = propr::compile("mult * copy", &test_env()).unwrap();
    assert_golden("mult_tensor_copy", &out);
}

#[test]
fn golden_copy_seq_mult_tensor_id_1() {
    let out = propr::compile("(copy ; mult) * id(1)", &test_env()).unwrap();
    assert_golden("copy_seq_mult_tensor_id_1", &out);
}

#[test]
fn golden_equation_copy_seq_mult_eq_id_1() {
    let out = propr::compile("copy ; mult = id(1)", &test_env()).unwrap();
    assert_golden("equation_copy_seq_mult_eq_id_1", &out);
}

#[test]
fn golden_chain_id_1_eq_id_1_subset_id_1() {
    let out = propr::compile("id(1) = id(1) ⊆ id(1)", &test_env()).unwrap();
    assert_golden("chain_id_1_eq_id_1_subset_id_1", &out);
}

/// Compiles one golden output to a full LaTeX document and runs `latexmk -pdf`
/// on it, verifying the generated TikZ actually typesets. Requires `latexmk`
/// on PATH; run manually (`rtk cargo test --test golden -- --ignored`).
#[test]
#[ignore]
fn latexmk_smoke() {
    // Uses only built-in id/swap wiring (no custom `\pic`s), so the smoke doc
    // needs no generator-pic definitions to compile.
    let tikz = propr::compile("id(1) = id(1) ⊆ id(1)", &Env::default()).unwrap();

    let doc = format!(
        "\\documentclass{{standalone}}\n\
         \\usepackage{{tikz}}\n\
         \\usetikzlibrary{{arrows.meta}}\n\
         \\begin{{document}}\n\
         {}\n\
         \\end{{document}}\n",
        tikz
    );

    let dir = std::env::temp_dir().join(format!("propr-latexmk-smoke-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("failed to create temp dir");
    let tex_path = dir.join("smoke.tex");
    std::fs::write(&tex_path, doc).expect("failed to write smoke.tex");

    let output = match std::process::Command::new("latexmk")
        .arg("-pdf")
        .arg("-interaction=nonstopmode")
        .arg("-halt-on-error")
        .arg("smoke.tex")
        .current_dir(&dir)
        .output()
    {
        Ok(o) => o,
        Err(e) => panic!(
            "failed to run `latexmk` (is it installed and on PATH?): {}",
            e
        ),
    };

    assert!(
        output.status.success(),
        "latexmk failed to compile the golden TikZ output:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
