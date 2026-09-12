use crate::codegen::config::Env;
use crate::codegen::layout::{Anchor, Layout, MIN_PITCH};
use crate::parser::ast::{Arg, Equation, Expr, RelOp};
use std::fmt::Write;

const COMP_GAP: f32 = 0.25;
const SIDE_GAP: f32 = 1.0;

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
            Expr::Gen { name, args } => self.render_gen(name, args),
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

        let height = n as f32 * MIN_PITCH;
        let mut body = String::new();
        let mut left = Vec::with_capacity(n as usize);
        let mut right = Vec::with_capacity(n as usize);

        for i in 0..n {
            let y = (i + 1) as f32 * MIN_PITCH;
            let in_anchor = self.fresh("id_in_");
            let out_anchor = self.fresh("id_out_");
            let _ = writeln!(&mut body, "  \\coordinate ({}) at (0,{:.3});", in_anchor, y);
            let _ = writeln!(
                &mut body,
                "  \\coordinate ({}) at (1.000,{:.3});",
                out_anchor, y
            );
            let _ = writeln!(&mut body, "  \\draw ({}) -- ({});", in_anchor, out_anchor);
            left.push(Anchor::known(in_anchor, y));
            right.push(Anchor::known(out_anchor, y));
        }

        Layout {
            width: 1.0,
            height,
            left,
            right,
            body,
        }
    }

    fn render_swap(&mut self, m: u32, n: u32) -> Layout {
        let total = m + n;
        let height = total as f32 * MIN_PITCH;
        let slot_y = |slot: u32| (total - slot) as f32 * MIN_PITCH;

        let mut body = String::new();
        let mut left_slots = Vec::with_capacity(total as usize);
        let mut right_slots = Vec::with_capacity(total as usize);

        for slot in 0..total {
            let y = slot_y(slot);
            let l = self.fresh("sw_in_");
            let r = self.fresh("sw_out_");
            let _ = writeln!(&mut body, "  \\coordinate ({}) at (0,{:.3});", l, y);
            let _ = writeln!(&mut body, "  \\coordinate ({}) at (1.000,{:.3});", r, y);
            left_slots.push(Anchor::known(l, y));
            right_slots.push(Anchor::known(r, y));
        }

        let mut left = Vec::with_capacity(total as usize);
        let mut right = vec![None; total as usize];

        for i in 0..m {
            let l = left_slots[i as usize].clone();
            let r_slot = (n + i) as usize;
            let _ = writeln!(
                &mut body,
                "  \\draw ({}) .. controls (0.500,{:.3}) and (0.500,{:.3}) .. ({});",
                l.name,
                l.y.unwrap(),
                right_slots[r_slot].y.unwrap(),
                right_slots[r_slot].name
            );
            right[r_slot] = Some(right_slots[r_slot].clone());
            left.push(l);
        }
        for j in 0..n {
            let l = left_slots[(m + j) as usize].clone();
            let r_slot = j as usize;
            let _ = writeln!(
                &mut body,
                "  \\draw ({}) .. controls (0.500,{:.3}) and (0.500,{:.3}) .. ({});",
                l.name,
                l.y.unwrap(),
                right_slots[r_slot].y.unwrap(),
                right_slots[r_slot].name
            );
            right[r_slot] = Some(right_slots[r_slot].clone());
            left.push(l);
        }

        let right = right.into_iter().map(|a| a.unwrap()).collect();

        Layout {
            width: 1.0,
            height,
            left,
            right,
            body,
        }
    }

    fn render_gen(&mut self, name: &str, args: &[Arg]) -> Result<Layout, String> {
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

        if args.len() != generator.params.len() {
            return Err(format!(
                "generator {} expects {} argument(s), got {}",
                name,
                generator.params.len(),
                args.len()
            ));
        }

        let args_list = args
            .iter()
            .zip(generator.params.iter())
            .map(|(v, p)| format!("{}={}", p, v))
            .collect::<Vec<_>>()
            .join(", ");
        let args_str = if !args_list.is_empty() {
            format!("[{}]", args_list)
        } else {
            "".to_string()
        };
        let pic_id = self.fresh("g");
        let mut body = String::new();
        let _ = writeln!(
            &mut body,
            "  \\pic{} ({}) at ({:.3},{:.3}) {{{}}};",
            args_str,
            pic_id,
            width / 2.0,
            height / 2.0,
            pic
        );

        let mut left = Vec::with_capacity(arity as usize);
        if arity > 0 {
            let bundle = arity / visual_arity;
            for i in 0..arity {
                left.push(Anchor::unknown(format!("{}-in-{}", pic_id, i / bundle)));
            }
        }

        let mut right = Vec::with_capacity(coarity as usize);
        if coarity > 0 {
            let bundle = coarity / visual_coarity;
            for i in 0..coarity {
                right.push(Anchor::unknown(format!("{}-out-{}", pic_id, i / bundle)));
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

        let top_off = bottom_layout.height;
        let top_left_shifted: Vec<_> = top_layout.left.iter().map(|a| a.shifted(top_off)).collect();
        let top_right_shifted: Vec<_> = top_layout
            .right
            .iter()
            .map(|a| a.shifted(top_off))
            .collect();

        let top_left = self.reanchor_to(&mut body, &top_left_shifted, top_x, 0.0);
        let top_right = self.reanchor_to(
            &mut body,
            &top_right_shifted,
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

        let left_right_shifted: Vec<_> = left_layout
            .right
            .iter()
            .map(|a| a.shifted(left_y))
            .collect();
        let right_left_shifted: Vec<_> = right_layout
            .left
            .iter()
            .map(|a| a.shifted(right_y))
            .collect();

        let max_abs_dy = left_right_shifted
            .iter()
            .zip(right_left_shifted.iter())
            .map(|(l, r)| match (l.y, r.y) {
                (Some(ly), Some(ry)) => (ly - ry).abs(),
                _ => left_layout.height.max(right_layout.height),
            })
            .fold(0.0_f32, f32::max);

        let gap = COMP_GAP.max(0.5 * max_abs_dy);
        let right_x = left_layout.width + gap;
        let width = left_layout.width + gap + right_layout.width;

        let mut body = String::new();
        Self::emit_scoped(&mut body, 0.0, left_y, &left_layout.body);
        Self::emit_scoped(&mut body, right_x, right_y, &right_layout.body);

        for (l, r) in left_right_shifted.iter().zip(right_left_shifted.iter()) {
            let _ = writeln!(
                &mut body,
                "  \\draw ({}) to[out=0,in=180] ({});",
                l.name, r.name
            );
        }

        let left: Vec<_> = left_layout.left.iter().map(|a| a.shifted(left_y)).collect();
        let right: Vec<_> = right_layout
            .right
            .iter()
            .map(|a| a.shifted(right_y))
            .collect();

        Ok(Layout {
            width,
            height,
            left,
            right,
            body,
        })
    }

    fn reanchor_to(
        &mut self,
        body: &mut String,
        anchors: &[Anchor],
        current_x: f32,
        target_x: f32,
    ) -> Vec<Anchor> {
        if (current_x - target_x).abs() < 1e-6 {
            return anchors.to_vec();
        }

        let mut out = Vec::with_capacity(anchors.len());
        for anchor in anchors {
            let exposed = self.fresh("a");
            let _ = writeln!(
                body,
                "  \\coordinate ({}) at ({:.3},0 |- {});",
                exposed, target_x, anchor.name
            );
            let _ = writeln!(body, "  \\draw ({}) -- ({});", anchor.name, exposed);
            out.push(Anchor {
                name: exposed,
                y: anchor.y,
            });
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

pub fn generate_equation(equation: &Equation, env: &Env) -> Result<String, String> {
    let mut renderer = Renderer::new(env);
    let layouts: Vec<Layout> = equation
        .sides
        .iter()
        .map(|side| renderer.render(side))
        .collect::<Result<Vec<_>, _>>()?;

    let max_height = layouts.iter().map(|l| l.height).fold(0.0_f32, f32::max);

    let mut body = String::new();
    let mut x = 0.0_f32;
    for (i, layout) in layouts.iter().enumerate() {
        let y = (max_height - layout.height) / 2.0;
        Renderer::emit_scoped(&mut body, x, y, &layout.body);
        let side_end = x + layout.width;

        if i < equation.ops.len() {
            let gap_mid = side_end + SIDE_GAP / 2.0;
            let symbol = match equation.ops[i] {
                RelOp::Eq => "$=$",
                RelOp::Subset => "$\\subseteq$",
            };
            let node_id = renderer.fresh("rel");
            let _ = writeln!(
                &mut body,
                "  \\node ({}) at ({:.3},{:.3}) {{{}}};",
                node_id,
                gap_mid,
                max_height / 2.0,
                symbol
            );
        }

        x = side_end + SIDE_GAP;
    }

    let mut out = String::new();
    out.push_str("\\begin{tikzpicture}\n");
    out.push_str(&body);
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

    // Parses all "(x,y)" style numeric coordinate pairs out of TikZ output,
    // e.g. from "\coordinate (foo) at (1.000,0.500);" or pic "at (x,y)".
    fn parse_coords(out: &str) -> Vec<(f32, f32)> {
        let re =
            regex::Regex::new(r"\(\s*(-?[0-9]+(?:\.[0-9]+)?)\s*,\s*(-?[0-9]+(?:\.[0-9]+)?)\s*\)")
                .unwrap();
        re.captures_iter(out)
            .map(|c| {
                let x: f32 = c[1].parse().unwrap();
                let y: f32 = c[2].parse().unwrap();
                (x, y)
            })
            .collect()
    }

    // Invariant (1): id(n) must give each wire its own anchor pair, spread
    // vertically -- not one shared line for all n wires.
    #[test]
    fn id_gives_each_wire_a_distinct_anchor_and_y() {
        let out = generate(&Expr::Id(2), &env()).expect("generate id(2)");
        let draw_re = regex::Regex::new(r"\\draw \(([^)]+)\) -- \(([^)]+)\);").unwrap();
        let draws: Vec<_> = draw_re.captures_iter(&out).collect();
        assert_eq!(
            draws.len(),
            2,
            "id(2) should emit two separate wire \\draw lines, got: {}",
            out
        );
        let coord_re =
            regex::Regex::new(r"\\coordinate \(([^)]+)\) at \(([^,]+),([^)]+)\);").unwrap();
        let mut left_ys = std::collections::HashSet::new();
        for cap in coord_re.captures_iter(&out) {
            let name = &cap[1];
            if name.contains("in") {
                let y: f32 = cap[3].trim().parse().unwrap();
                left_ys.insert((y * 1000.0).round() as i64);
            }
        }
        assert_eq!(
            left_ys.len(),
            2,
            "id(2) left anchors should have 2 distinct y coordinates, got {:?} from: {}",
            left_ys,
            out
        );
    }

    // Invariant (2): minimum wire pitch grows layout height with wire count.
    #[test]
    fn id_height_grows_with_minimum_wire_pitch() {
        let out = generate(&Expr::Id(4), &env()).expect("generate id(4)");
        let coords = parse_coords(&out);
        let max_y = coords.iter().map(|(_, y)| *y).fold(f32::MIN, f32::max);
        assert!(
            max_y >= 4.0 * 0.5 - 1e-3,
            "id(4) picture height (max y = {}) should be >= 4 * min pitch (0.5), out: {}",
            max_y,
            out
        );
    }

    // Invariant (3): renderer's own comp-arity guard must fire even when
    // called directly on a hand-built mismatched Expr (bypassing typecheck).
    #[test]
    fn comp_arity_mismatch_is_reported_not_silent() {
        let mismatched = Expr::Comp(
            Box::new(Expr::Id(3)),
            Box::new(Expr::Gen {
                name: "mult".into(),
                args: vec![],
            }),
        );
        let err = generate(&mismatched, &env()).expect_err("expected composition mismatch error");
        assert!(err.contains("composition mismatch"), "got: {}", err);
    }

    // Invariant (4): args/params zip must be guarded -- extra args than
    // params must not be silently dropped.
    #[test]
    fn extra_generator_args_are_rejected_not_dropped() {
        let mut e = env();
        e.insert(
            "scaled".into(),
            Generator {
                sig: Sig {
                    arity: 1,
                    coarity: 1,
                },
                params: vec!["k".into()],
                visual_arity: None,
                visual_coarity: None,
                symbol: String::new(),
                pic: "scaled".into(),
                width: 1.0,
                height: 1.0,
            },
        );
        let out = generate(
            &Expr::Gen {
                name: "scaled".into(),
                args: vec![Arg::Number(1), Arg::Number(2)],
            },
            &e,
        );
        assert!(
            out.is_err(),
            "generator called with more args than params should error, not silently zip-truncate"
        );
    }

    // Invariant (5): composing two forms emits a connecting wire per wire
    // crossing the composition boundary (coarity of left == arity of right).
    #[test]
    fn composition_emits_one_connecting_wire_per_wire() {
        let comp = Expr::Comp(
            Box::new(Expr::Gen {
                name: "copy".into(),
                args: vec![],
            }),
            Box::new(Expr::Tensor(
                Box::new(Expr::Gen {
                    name: "mult".into(),
                    args: vec![],
                }),
                Box::new(Expr::Id(0)),
            )),
        );
        // copy: 1 -> 2 ; (mult * id(0)): arity 2 -> coarity 1: valid comp.
        let out = generate(&comp, &env()).expect("generate composition");
        assert_eq!(
            out.matches("\\pic").count(),
            2,
            "expected two pics: {}",
            out
        );
        // At least one connecting draw beyond the two forms' own bodies.
        assert!(
            out.matches("\\draw").count() >= 1,
            "expected at least one connecting wire draw: {}",
            out
        );
    }

    // Invariant (6): composition gap adapts to the maximum vertical offset
    // it must absorb, rather than staying pinned at the old fixed 0.25.
    #[test]
    fn composition_gap_grows_with_vertical_offset() {
        let tall_tensor = Expr::Tensor(
            Box::new(Expr::Gen {
                name: "copy".into(),
                args: vec![],
            }),
            Box::new(Expr::Gen {
                name: "copy".into(),
                args: vec![],
            }),
        ); // height 2, coarity 4
        let matching = Expr::Tensor(
            Box::new(Expr::Gen {
                name: "mult".into(),
                args: vec![],
            }),
            Box::new(Expr::Gen {
                name: "mult".into(),
                args: vec![],
            }),
        ); // arity 4
        let comp = Expr::Comp(Box::new(tall_tensor), Box::new(matching));
        let out = generate(&comp, &env()).expect("generate tall composition");

        // The composition's right-hand block is placed in a
        // \begin{scope}[shift={(right_x,...)}] whose x offset is
        // left_width + gap. Since both sides here have width 1.0, the
        // largest x-shift present directly reveals the gap used.
        let shift_re = regex::Regex::new(r"shift=\{\(([^,]+),[^)]+\)\}").unwrap();
        let left_width = 1.0; // both tensor branches use width-1.0 generators
        let max_shift_x = shift_re
            .captures_iter(&out)
            .map(|c| c[1].trim().parse::<f32>().unwrap())
            .fold(f32::MIN, f32::max);
        assert!(
            max_shift_x > f32::MIN,
            "expected at least one scope shift in output: {}",
            out
        );
        let gap = max_shift_x - left_width;
        assert!(
            gap > 0.25 + 1e-3,
            "composition gap ({}) should grow beyond fixed 0.25 for tall compositions: {}",
            gap,
            out
        );
    }

    // --- Step 5: generate_equation ---

    // (1) A single-relation equation renders in ONE tikzpicture, with a "$=$"
    // relation node, and content from both sides (mult/copy pics and an id
    // wire).
    #[test]
    fn generate_equation_single_relation_one_tikzpicture_with_both_sides() {
        let equation =
            crate::parser::engine::parse_equation("copy ; mult = id(1)").expect("parse equation");
        let out = generate_equation(&equation, &env()).expect("generate equation");
        assert_eq!(
            out.matches("\\begin{tikzpicture}").count(),
            1,
            "expected exactly one tikzpicture, got: {}",
            out
        );
        assert!(
            out.contains("$=$"),
            "expected an $=$ relation node: {}",
            out
        );
        assert!(out.contains("multiplication"), "missing mult pic: {}", out);
        assert!(out.contains("copy"), "missing copy pic: {}", out);
        assert!(
            out.contains("id_in_") || out.contains("id_out_"),
            "missing id(1) wire anchors: {}",
            out
        );
    }

    // (3) A chain "a = b ⊆ c" produces exactly one $=$ node, one $\subseteq$
    // node, and still a single tikzpicture.
    #[test]
    fn generate_equation_chain_has_one_of_each_relation_node_and_one_tikzpicture() {
        let equation = crate::parser::engine::parse_equation("id(1) = id(1) ⊆ id(1)")
            .expect("parse chain equation");
        let out = generate_equation(&equation, &env()).expect("generate chain");
        assert_eq!(
            out.matches("\\begin{tikzpicture}").count(),
            1,
            "expected exactly one tikzpicture for a chain: {}",
            out
        );
        assert_eq!(
            out.matches("$=$").count(),
            1,
            "expected exactly one $=$ node: {}",
            out
        );
        assert_eq!(
            out.matches("\\subseteq").count(),
            1,
            "expected exactly one \\subseteq node: {}",
            out
        );
    }

    // (4) No coordinate-name collisions across sides: every \coordinate name
    // must be unique even though both sides are structurally identical
    // id(1) expressions rendered by the same Renderer instance.
    #[test]
    fn generate_equation_coordinate_names_are_unique_across_sides() {
        let equation =
            crate::parser::engine::parse_equation("id(1) = id(1)").expect("parse equation");
        let out = generate_equation(&equation, &env()).expect("generate equation");
        let coord_re = regex::Regex::new(r"\\coordinate \(([^)]+)\)").unwrap();
        let names: Vec<&str> = coord_re
            .captures_iter(&out)
            .map(|c| c.get(1).unwrap().as_str())
            .collect();
        let unique: std::collections::HashSet<&str> = names.iter().copied().collect();
        assert_eq!(
            unique.len(),
            names.len(),
            "coordinate names must be unique across all sides, got: {:?} in: {}",
            names,
            out
        );
    }
}
