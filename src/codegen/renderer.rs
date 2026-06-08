use crate::codegen::config::Env;
use crate::parser::ast::Expr;
use std::fmt::Write;

const COMP_GAP: f32 = 0.25;

#[derive(Debug, Clone)]
struct Layout {
    width: f32,
    height: f32,
    left: Vec<String>,
    right: Vec<String>,
    body: String,
}

struct Renderer<'a> {
    env: &'a Env,
    next_id: usize,
}

impl<'a> Renderer<'a> {
    fn new(env: &'a Env) -> Self {
        Self { env, next_id: 0 }
    }

    fn fresh(&mut self, prefix: &str) -> String {
        self.next_id += 1;
        format!("{}{}", prefix, self.next_id)
    }

    fn render(&mut self, expr: &Expr) -> Result<Layout, String> {
        match expr {
            Expr::Id(n) => Ok(self.render_id(*n)),
            Expr::Swap(m, n) => Ok(self.render_swap(*m, *n)),
            Expr::Gen { name, args: _ } => self.render_gen(name),
            Expr::Tensor(top, bottom) => self.render_tensor(top, bottom),
            Expr::Comp(left, right) => self.render_comp(left, right),
        }
    }

    fn render_id(&mut self, n: u32) -> Layout {
        if n == 0 {
            return Layout {
                width: 1.0,
                height: 1.0,
                left: Vec::new(),
                right: Vec::new(),
                body: String::new(),
            };
        }

        let mut body = String::new();
        let in_anchor = self.fresh("id_in_");
        let out_anchor = self.fresh("id_out_");
        let _ = writeln!(&mut body, "  \\coordinate ({}) at (0,0.500);", in_anchor);
        let _ = writeln!(
            &mut body,
            "  \\coordinate ({}) at (1.000,0.500);",
            out_anchor
        );
        let _ = writeln!(&mut body, "  \\draw ({}) -- ({});", in_anchor, out_anchor);

        Layout {
            width: 1.0,
            height: 1.0,
            left: vec![in_anchor; n as usize],
            right: vec![out_anchor; n as usize],
            body,
        }
    }

    fn render_swap(&mut self, m: u32, n: u32) -> Layout {
        let mut body = String::new();
        let in_top = self.fresh("sw_in_top_");
        let in_bottom = self.fresh("sw_in_bottom_");
        let out_top = self.fresh("sw_out_top_");
        let out_bottom = self.fresh("sw_out_bottom_");

        let _ = writeln!(&mut body, "  \\coordinate ({}) at (0,1.000);", in_top);
        let _ = writeln!(&mut body, "  \\coordinate ({}) at (0,0.000);", in_bottom);
        let _ = writeln!(&mut body, "  \\coordinate ({}) at (1.000,1.000);", out_top);
        let _ = writeln!(
            &mut body,
            "  \\coordinate ({}) at (1.000,0.000);",
            out_bottom
        );
        let _ = writeln!(
            &mut body,
            "  \\draw ({}) .. controls (0.500,0.000) and (0.500,1.000) .. ({});",
            in_bottom, out_top
        );
        let _ = writeln!(
            &mut body,
            "  \\draw ({}) .. controls (0.500,1.000) and (0.500,0.000) .. ({});",
            in_top, out_bottom
        );

        let mut left = vec![String::new(); (m + n) as usize];
        let mut right = vec![String::new(); (m + n) as usize];

        for i in 0..m {
            left[i as usize] = in_top.clone();
            right[(n + i) as usize] = out_bottom.clone();
        }
        for j in 0..n {
            left[(m + j) as usize] = in_bottom.clone();
            right[j as usize] = out_top.clone();
        }

        Layout {
            width: 1.0,
            height: 1.0,
            left,
            right,
            body,
        }
    }

    fn render_gen(&mut self, name: &str) -> Result<Layout, String> {
        let generator = self
            .env
            .get(name)
            .ok_or_else(|| format!("unknown generator: {}", name))?;

        let arity = generator.sig.arity;
        let coarity = generator.sig.coarity;
        let visual_arity = generator.visual_arity.unwrap_or(arity);
        let visual_coarity = generator.visual_coarity.unwrap_or(coarity);

        if arity > 0 && visual_arity == 0 {
            return Err(format!(
                "{} has arity {}, visual_arity cannot be 0",
                name, arity
            ));
        }
        if coarity > 0 && visual_coarity == 0 {
            return Err(format!(
                "{} has coarity {}, visual_coarity cannot be 0",
                name, coarity
            ));
        }
        if visual_arity > 0 && arity % visual_arity != 0 {
            return Err(format!(
                "{} visual_arity {} does not divide arity {}",
                name, visual_arity, arity
            ));
        }
        if visual_coarity > 0 && coarity % visual_coarity != 0 {
            return Err(format!(
                "{} visual_coarity {} does not divide coarity {}",
                name, visual_coarity, coarity
            ));
        }

        let pic = if generator.pic.is_empty() {
            name
        } else {
            &generator.pic
        };
        let width = if generator.width > 0.0 {
            generator.width
        } else {
            1.0
        };
        let height = if generator.height > 0.0 {
            generator.height
        } else {
            1.0
        };

        let pic_id = self.fresh("g");
        let mut body = String::new();
        let _ = writeln!(
            &mut body,
            "  \\pic ({}) at ({:.3},{:.3}) {{{}}};",
            pic_id,
            width / 2.0,
            height / 2.0,
            pic
        );

        let mut left = vec![String::new(); arity as usize];
        if arity > 0 {
            let bundle = arity / visual_arity;
            for i in 0..arity {
                left[i as usize] = format!("{}-in-{}", pic_id, i / bundle);
            }
        }

        let mut right = vec![String::new(); coarity as usize];
        if coarity > 0 {
            let bundle = coarity / visual_coarity;
            for i in 0..coarity {
                right[i as usize] = format!("{}-out-{}", pic_id, i / bundle);
            }
        }

        Ok(Layout {
            width,
            height,
            left,
            right,
            body,
        })
    }

    fn render_tensor(&mut self, top: &Expr, bottom: &Expr) -> Result<Layout, String> {
        let top_layout = self.render(top)?;
        let bottom_layout = self.render(bottom)?;

        let width = top_layout.width.max(bottom_layout.width);
        let height = top_layout.height + bottom_layout.height;
        let top_x = (width - top_layout.width) / 2.0;
        let bottom_x = (width - bottom_layout.width) / 2.0;

        let mut body = String::new();
        Self::emit_scoped(&mut body, top_x, bottom_layout.height, &top_layout.body);
        Self::emit_scoped(&mut body, bottom_x, 0.0, &bottom_layout.body);

        let top_left = self.reanchor_to(&mut body, &top_layout.left, top_x, 0.0);
        let top_right = self.reanchor_to(
            &mut body,
            &top_layout.right,
            top_x + top_layout.width,
            width,
        );
        let bottom_left = self.reanchor_to(&mut body, &bottom_layout.left, bottom_x, 0.0);
        let bottom_right = self.reanchor_to(
            &mut body,
            &bottom_layout.right,
            bottom_x + bottom_layout.width,
            width,
        );

        let left = top_left.into_iter().chain(bottom_left).collect();
        let right = top_right.into_iter().chain(bottom_right).collect();

        Ok(Layout {
            width,
            height,
            left,
            right,
            body,
        })
    }

    fn render_comp(&mut self, left: &Expr, right: &Expr) -> Result<Layout, String> {
        let left_layout = self.render(left)?;
        let right_layout = self.render(right)?;

        if left_layout.right.len() != right_layout.left.len() {
            return Err(format!(
                "composition mismatch: left has coarity {}, right has arity {}",
                left_layout.right.len(),
                right_layout.left.len()
            ));
        }

        let height = left_layout.height.max(right_layout.height);
        let left_y = (height - left_layout.height) / 2.0;
        let right_y = (height - right_layout.height) / 2.0;
        let right_x = left_layout.width + COMP_GAP;
        let width = left_layout.width + COMP_GAP + right_layout.width;

        let mut body = String::new();
        Self::emit_scoped(&mut body, 0.0, left_y, &left_layout.body);
        Self::emit_scoped(&mut body, right_x, right_y, &right_layout.body);

        for (l, r) in left_layout.right.iter().zip(right_layout.left.iter()) {
            let _ = writeln!(&mut body, "  \\draw ({}) to[out=0,in=180] ({});", l, r);
        }

        Ok(Layout {
            width,
            height,
            left: left_layout.left,
            right: right_layout.right,
            body,
        })
    }

    fn reanchor_to(
        &mut self,
        body: &mut String,
        anchors: &[String],
        current_x: f32,
        target_x: f32,
    ) -> Vec<String> {
        if (current_x - target_x).abs() < 1e-6 {
            return anchors.to_vec();
        }

        let mut out = Vec::with_capacity(anchors.len());
        for anchor in anchors {
            let exposed = self.fresh("a");
            let _ = writeln!(
                body,
                "  \\coordinate ({}) at ({:.3},0 |- {});",
                exposed, target_x, anchor
            );
            let _ = writeln!(body, "  \\draw ({}) -- ({});", anchor, exposed);
            out.push(exposed);
        }
        out
    }

    fn emit_scoped(body: &mut String, x: f32, y: f32, child: &str) {
        if child.is_empty() {
            return;
        }
        if x.abs() < 1e-6 && y.abs() < 1e-6 {
            body.push_str(child);
            return;
        }
        let _ = writeln!(body, "  \\begin{{scope}}[shift={{({:.3},{:.3})}}]", x, y);
        body.push_str(child);
        let _ = writeln!(body, "  \\end{{scope}}");
    }
}

pub fn generate(expr: &Expr, env: &Env) -> Result<String, String> {
    let mut renderer = Renderer::new(env);
    let layout = renderer.render(expr)?;
    let mut out = String::new();
    out.push_str("\\begin{tikzpicture}\n");
    out.push_str(&layout.body);
    out.push_str("\\end{tikzpicture}\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{codegen::config::Generator, typechecker::Sig};

    fn env() -> Env {
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
    fn wraps_tikzpicture() {
        let out = generate(&Expr::Id(1), &env()).expect("generate id");
        assert!(out.contains("\\begin{tikzpicture}"));
        assert!(out.contains("\\end{tikzpicture}"));
    }

    #[test]
    fn emits_pic_for_generator() {
        let out = generate(
            &Expr::Gen {
                name: "mult".into(),
                args: vec![],
            },
            &env(),
        )
        .expect("generate gen");
        assert!(out.contains("\\pic"));
        assert!(out.contains("multiplication"));
    }

    #[test]
    fn reports_unknown_generator() {
        let err = generate(
            &Expr::Gen {
                name: "missing".into(),
                args: vec![],
            },
            &env(),
        )
        .expect_err("expected unknown generator error");
        assert!(err.contains("unknown generator"));
    }
}
