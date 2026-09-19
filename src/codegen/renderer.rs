use crate::codegen::config::Env;
use crate::codegen::layout::{Anchor, Layout, MIN_PITCH};
use crate::parser::ast::{Arg, Equation, Expr, RelOp};
use std::fmt::Write;

const COMP_GAP: f32 = 0.25;
const SIDE_GAP: f32 = 1.0;

/// `generator.tikz`'s header comment documents the port convention: pics
/// are drawn in the fixed unit frame x,y in [-0.5,0.5], and for n tips on a
/// side the k-th tip (k=0..n-1, counted TOP-first) sits at
/// y_frac = 0.5 - k/(n-1) (y_frac = 0 when n<=1). Shared by `render_gen`
/// (to place ports) and by this file's tests (to independently verify a
/// pic's absolute port y from parsed `\pic ... at (x,y) {..};` output)
/// instead of duplicating the formula.
fn pic_port_y_frac(k: u32, n: u32) -> f32 {
    if n <= 1 {
        0.0
    } else {
        0.5 - (k as f32) / ((n - 1) as f32)
    }
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
            let y = (i as f32 + 0.5) * MIN_PITCH;
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
        let slot_y = |slot: u32| (total as f32 - 0.5 - slot as f32) * MIN_PITCH;

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
                l.name, l.y, right_slots[r_slot].y, right_slots[r_slot].name
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
                l.name, l.y, right_slots[r_slot].y, right_slots[r_slot].name
            );
            right[r_slot] = Some(right_slots[r_slot].clone());
            left.push(l);
        }

        let mut right: Vec<Anchor> = right.into_iter().map(|a| a.unwrap()).collect();

        // `left`/`right` were built in slot order (slot 0 = highest y, see
        // `slot_y` above), i.e. top-first -- disagreeing with
        // `render_id`/`render_gen`'s bottom-first convention (vector index
        // 0 = lowest y). Reverse both exposed vectors so vector position 0
        // means "physically bottom-most port" here too. This only
        // relabels which vector slot exposes which already-built anchor;
        // the `\draw ... controls ...` commands above (the actual swap
        // routing) are untouched, so e.g. swap(1,1)'s physically-bottom
        // input still ends, internally, at the physically-top output.
        left.reverse();
        right.reverse();

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
        // Ports sit at `height/2 + pic_port_y_frac(k, n)` (see `port_y`
        // below), i.e. always within `height/2 +/- 0.5` of the box centre.
        // If the box were the generator's own configured `height` alone (or
        // the 1.0 default), a multi-port generator (e.g. `mult`, 2 ports on
        // one side) would place its ports exactly at the box edges, with no
        // margin -- while `render_id`/`render_swap` give every wire a
        // half-`MIN_PITCH` margin (`y = (i+0.5)*MIN_PITCH`, box height
        // `n*MIN_PITCH`). That mismatch is exactly the residual bend a
        // pic-boundary composition (e.g. `id(2);mult`) would otherwise keep.
        // Fix: grow the box to fit `MIN_PITCH`-spaced ports too --
        // `max(visual_arity, visual_coarity, 1) * MIN_PITCH` -- and, when the
        // config gives an explicit `height` larger than that, honour the
        // larger one (`max`, not "config wins"): a user-requested taller box
        // must never be *shrunk* by this margin fix.
        //
        // This is a real trade-off, not a pure win: `visual_arity` and
        // `visual_coarity` can differ (e.g. `mult`, 2-in/1-out), and only
        // one box height is used for both sides, so the *minority* side
        // (here, the single output) gets `height/2` margin on each side of
        // its one port instead of the `MIN_PITCH/2` margin `render_id`
        // gives every wire -- its port lands at the box's exact vertical
        // centre, not at an `(i+0.5)*MIN_PITCH`-spaced slot. Composing that
        // minority side against `id`/`swap`/another generator can still
        // bend (pinned at exactly `(mult * ge) ; id(2)`'s +/-0.25 by
        // `mult_tensor_ge_seq_id2_bends_by_documented_asymmetric_margin`).
        // We keep `max` anyway because it never makes two *distinct* wires
        // share a y (the majority side's ports stay correctly, uniformly
        // `MIN_PITCH`-spaced), which is the property
        // `mult_tensor_mult_has_distinct_port_ys` pins down.
        let port_count_height = visual_arity.max(visual_coarity).max(1) as f32 * MIN_PITCH;
        let height = generator.height.max(port_count_height);

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

        // The pic is placed at its own center (width/2, height/2, see the
        // `\pic` line above), and `render_gen`'s `\pic` invocation carries
        // no `scale=`/transform key, so `pic_port_y_frac`'s unit frame is
        // never stretched vertically by `height` -- absolute port y is
        // height/2 + y_frac (not height/2 + y_frac*height).
        let port_y = |k: u32, n: u32| -> f32 { height / 2.0 + pic_port_y_frac(k, n) };

        // `render_id`/`render_swap` number wires bottom-first (vector index
        // 0 = lowest y), but generator.tikz's own port *names* are top-first
        // (`-in-0`/`-out-0` sit highest). Reverse the visual-index -> vector-
        // position mapping here -- not the names, and not their true
        // geometric y, both of which are fixed by generator.tikz -- so
        // vector position 0 always means "physically bottom-most wire" like
        // every other `Layout` in this file. Without this, `render_comp`
        // would zip e.g. `id(2)`'s bottom wire (vector index 0) against a
        // generator's *top* port (also vector index 0) and cross the wires
        // while still reporting them "straight" (equal vector-index y).
        let mut left = Vec::with_capacity(arity as usize);
        if arity > 0 {
            let bundle = arity / visual_arity;
            for k in (0..visual_arity).rev() {
                let y = port_y(k, visual_arity);
                for _ in 0..bundle {
                    left.push(Anchor::known(format!("{}-in-{}", pic_id, k), y));
                }
            }
        }

        let mut right = Vec::with_capacity(coarity as usize);
        if coarity > 0 {
            let bundle = coarity / visual_coarity;
            for k in (0..visual_coarity).rev() {
                let y = port_y(k, visual_coarity);
                for _ in 0..bundle {
                    right.push(Anchor::known(format!("{}-out-{}", pic_id, k), y));
                }
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

        // `render_id`/`render_swap`/`render_gen` all number wires
        // bottom-first (vector index 0 = physically lowest port). Tensor's
        // own top/bottom placement is unrelated to that convention -- `top`
        // is still drawn physically above `bottom` (`top_off` above is
        // unchanged) -- but the *vector order* used to expose the combined
        // boundary must also be bottom-first (bottom's own ports first,
        // then top's) so `render_comp` zips this Tensor's ports against
        // another bottom-first Layout (e.g. a generator, or another Tensor)
        // without crossing them.
        let left = bottom_left.into_iter().chain(top_left).collect();
        let right = bottom_right.into_iter().chain(top_right).collect();

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

        // Prefer port-aligned placement over blunt bbox-centering: shift
        // `right` by the mean of the paired boundary anchors' (left.right[i],
        // right.left[i]) y-differences so the wires crossing the
        // composition boundary are straight (equal pitch) or symmetrically
        // bent around the mean (differing pitch). Every `Anchor` in this
        // renderer carries a known y (`render_id`/`render_swap`/`render_gen`
        // all supply one; `layout::Anchor` has no `unknown` constructor), so
        // the only remaining reason to fall back to bbox-centering is a
        // degenerate empty boundary (arity/coarity 0), where there is no
        // pair to average a dy over.
        let has_boundary = !left_layout.right.is_empty();

        let (left_shift0, right_shift0) = if has_boundary {
            let n = left_layout.right.len() as f32;
            let sum: f32 = left_layout
                .right
                .iter()
                .zip(right_layout.left.iter())
                .map(|(l, r)| l.y - r.y)
                .sum();
            let dy = sum / n;
            (0.0_f32, dy)
        } else {
            let height0 = left_layout.height.max(right_layout.height);
            (
                (height0 - left_layout.height) / 2.0,
                (height0 - right_layout.height) / 2.0,
            )
        };

        // Renormalize so the composite's minimum y is 0 and its height
        // covers both shifted bodies (no clipping, no stray negative y).
        let min_y = left_shift0.min(right_shift0);
        let max_y = (left_shift0 + left_layout.height).max(right_shift0 + right_layout.height);
        let height = max_y - min_y;
        let left_y = left_shift0 - min_y;
        let right_y = right_shift0 - min_y;

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
            .map(|(l, r)| (l.y - r.y).abs())
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
            out.push(Anchor::known(exposed, anchor.y));
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

    // Mirrors the six generators from `.local/material/generators.toml`
    // exercised by the comp-wire-alignment validator expressions (same
    // sig/visual arities and pic names), spelled out in-crate so these
    // tests run in CI without reading the git-excluded `.local/` tree.
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
                visual_arity: Some(2),
                visual_coarity: Some(1),
                symbol: String::new(),
                pic: "multiplication".into(),
                width: 1.0,
                height: 1.0,
            },
        );
        env.insert(
            "comult".into(),
            Generator {
                sig: Sig {
                    arity: 1,
                    coarity: 2,
                },
                params: vec![],
                visual_arity: Some(1),
                visual_coarity: Some(2),
                symbol: String::new(),
                pic: "comultiplication".into(),
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
                visual_arity: Some(1),
                visual_coarity: Some(2),
                symbol: String::new(),
                pic: "copy".into(),
                width: 1.0,
                height: 1.0,
            },
        );
        env.insert(
            "ge".into(),
            Generator {
                sig: Sig {
                    arity: 1,
                    coarity: 1,
                },
                params: vec![],
                visual_arity: None,
                visual_coarity: None,
                symbol: String::new(),
                pic: String::new(),
                width: 1.0,
                height: 1.0,
            },
        );
        env.insert(
            "codiscard".into(),
            Generator {
                sig: Sig {
                    arity: 0,
                    coarity: 1,
                },
                params: vec![],
                visual_arity: None,
                visual_coarity: None,
                symbol: String::new(),
                pic: "codiscard".into(),
                width: 1.0,
                height: 1.0,
            },
        );
        env.insert(
            "antipode".into(),
            Generator {
                sig: Sig {
                    arity: 1,
                    coarity: 1,
                },
                params: vec![],
                visual_arity: None,
                visual_coarity: None,
                symbol: String::new(),
                pic: "antipode".into(),
                width: 1.0,
                height: 1.0,
            },
        );
        env
    }

    // A synthetic 5-port generator with no real `.local/material/generator.tikz`
    // counterpart, used only to construct a genuine pitch *mismatch* against
    // `render_id`'s constant `MIN_PITCH`. Every real generator in `env()`/
    // `generators.toml` has at most 2 visual ports, whose pitch (port
    // spacing over the fixed +/-0.5 unit frame, `1.0/(n-1)`, independent of
    // box height) now exactly equals `MIN_PITCH` (round-3 rework item 3) --
    // so no *real* generator's own max-port side still exercises a residual
    // bend or a grown composition gap on its own. A 5-port pic's pitch
    // (`1.0/4 = 0.25`) does not match `MIN_PITCH` (`1.0`),
    // reproducing that case without inventing a fictitious real generator.
    fn env_with_penta() -> Env {
        let mut e = env();
        e.insert(
            "penta".into(),
            Generator {
                sig: Sig {
                    arity: 5,
                    coarity: 0,
                },
                params: vec![],
                visual_arity: Some(5),
                visual_coarity: None,
                symbol: String::new(),
                pic: "penta".into(),
                width: 1.0,
                height: 1.0,
            },
        );
        e
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
            max_y >= 3.0 * MIN_PITCH - 1e-3,
            "id(4) picture height (max y = {}) should be >= (4-1) * MIN_PITCH ({}), out: {}",
            max_y,
            MIN_PITCH,
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
    //
    // Neither `Tensor(copy,copy) ; Tensor(mult,mult)` nor `Id(3) ; Swap(1,2)`
    // (this test's expressions across earlier rework rounds) still exercise
    // a growing gap: `render_swap`'s bottom-first fix (round 3) made any
    // `Id(n) ; Swap` boundary exactly straight, and `render_gen`'s box-
    // margin fix (round 4, BLOCKER 1) made any 2-port-generator boundary
    // exactly straight too. See the test body below for the current
    // `Id(5) ; penta` (5-port) example and why *that* mismatch remains.
    #[test]
    fn composition_gap_grows_with_vertical_offset() {
        // `Id(n)` composed directly with a `Swap` of the same total wire
        // count is now *always* exactly straight regardless of the m/n
        // split (both reduce to the same bottom-first `(k+0.5)*MIN_PITCH`
        // sequence -- see `render_swap`'s round-3 bottom-first fix), so it
        // can no longer exercise a growing gap. Use `Id(5)` against the
        // synthetic 5-port `penta` generator instead, whose pitch (0.25)
        // genuinely mismatches `MIN_PITCH` (1.0).
        let comp = Expr::Comp(
            Box::new(Expr::Id(5)),
            Box::new(Expr::Gen {
                name: "penta".into(),
                args: vec![],
            }),
        );
        let out = generate(&comp, &env_with_penta()).expect("generate tall composition");
        assert_pic_port_counts_match_env(&out, &env_with_penta());

        // The composition's right-hand block is placed in a
        // \begin{scope}[shift={(right_x,...)}] whose x offset is
        // left_width + gap. Since both sides here have width 1.0, the
        // largest x-shift present directly reveals the gap used.
        let shift_re = regex::Regex::new(r"shift=\{\(([^,]+),[^)]+\)\}").unwrap();
        let left_width = 1.0; // id(5)/penta both use width 1.0
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

    // --- comp-wire-alignment regression tests (see
    // .local/specs/comp-wire-alignment.md) ---
    //
    // These simulate TikZ's own coordinate semantics well enough to check
    // straightness: `\coordinate` captures an *absolute* canvas position at
    // definition time (the sum of all enclosing `\begin{scope}[shift=...]`
    // offsets plus its own literal (x,y)). Plain numeric `\coordinate`
    // declarations are resolved directly; `reanchor_to`'s `0 |- REF` calc
    // coordinates inherit REF's already-resolved absolute y verbatim
    // (matching real TikZ semantics: a named coordinate's y is captured at
    // definition time, independent of the scope surrounding a *later*
    // reference to it), and REF is always resolved earlier in the same
    // forward pass since `render_tensor`/`render_comp` only ever reanchor
    // anchors already emitted. Resolving calc chains is safe now that every
    // `Anchor` in the renderer is `known` (`render_id`/`render_swap`/
    // `render_gen` all supply a y; `layout::Anchor` no longer even has an
    // `unknown` constructor) -- there is no more "coincidentally known member of an
    // otherwise-unknown, bbox-centering-fallback pairing" case to guard
    // against.
    //
    // Generator `\pic` ports (e.g. "g3-out-0") are *not* literal
    // `\coordinate`s -- generator.tikz defines them inside the pic macro
    // body, invisible to this text -- but `render_gen` places every port at
    // a known y (see `pic_port_y_frac`), so this resolver reconstructs that
    // same absolute y from the emitted `\pic ... at (px,py) {..};` line
    // (py is always height/2) plus `pic_port_y_frac(k, n)`, where n is the
    // number of distinct port indices referenced anywhere in the output for
    // that pic id/side (sufficient for every port actually exercised by a
    // composition boundary, which is everything these tests check).
    fn absolute_y_by_name(tikz: &str) -> std::collections::HashMap<String, f32> {
        let scope_re =
            regex::Regex::new(r#"^\s*\\begin\{scope\}\[shift=\{\(([^,]+),([^)]+)\)\}\]"#).unwrap();
        let literal_re = regex::Regex::new(
            r"^\s*\\coordinate \(([^)]+)\) at \(\s*(-?[0-9]+(?:\.[0-9]+)?)\s*,\s*(-?[0-9]+(?:\.[0-9]+)?)\s*\);",
        )
        .unwrap();
        let pic_re = regex::Regex::new(
            r"^\s*\\pic(?:\[[^\]]*\])? \(([^)]+)\) at \(\s*(-?[0-9]+(?:\.[0-9]+)?)\s*,\s*(-?[0-9]+(?:\.[0-9]+)?)\s*\) \{",
        )
        .unwrap();
        let calc_re =
            regex::Regex::new(r"^\s*\\coordinate \(([^)]+)\) at \([^,]+,\s*0\s*\|-\s*([^)]+)\);")
                .unwrap();
        let port_ref_re = regex::Regex::new(r"([A-Za-z0-9_]+)-(in|out)-([0-9]+)").unwrap();

        // Pre-pass: how many distinct k's are referenced for each
        // (pic_id, side) anywhere in the output -- independent of scope
        // nesting, so it can run before the sequential scope-tracking pass.
        let mut port_count: std::collections::HashMap<(String, &str), u32> =
            std::collections::HashMap::new();
        for caps in port_ref_re.captures_iter(tikz) {
            let pic_id = caps[1].to_string();
            let side = if &caps[2] == "in" { "in" } else { "out" };
            let k: u32 = caps[3].parse().unwrap();
            let entry = port_count.entry((pic_id, side)).or_insert(0);
            *entry = (*entry).max(k + 1);
        }

        let mut stack: Vec<(f32, f32)> = vec![(0.0, 0.0)];
        let mut abs_y: std::collections::HashMap<String, f32> = std::collections::HashMap::new();

        for line in tikz.lines() {
            if let Some(caps) = scope_re.captures(line) {
                let dx: f32 = caps[1].trim().parse().unwrap();
                let dy: f32 = caps[2].trim().parse().unwrap();
                let top = *stack.last().unwrap();
                stack.push((top.0 + dx, top.1 + dy));
            } else if line.trim_start().starts_with("\\end{scope}") {
                stack.pop();
            } else if let Some(caps) = literal_re.captures(line) {
                let name = caps[1].to_string();
                let y: f32 = caps[3].trim().parse().unwrap();
                let top = *stack.last().unwrap();
                abs_y.insert(name, top.1 + y);
            } else if let Some(caps) = pic_re.captures(line) {
                let pic_id = caps[1].to_string();
                let py: f32 = caps[3].trim().parse().unwrap();
                let top = *stack.last().unwrap();
                for side in ["in", "out"] {
                    if let Some(&n) = port_count.get(&(pic_id.clone(), side)) {
                        for k in 0..n {
                            let name = format!("{}-{}-{}", pic_id, side, k);
                            let y = top.1 + py + pic_port_y_frac(k, n);
                            abs_y.insert(name, y);
                        }
                    }
                }
            } else if let Some(caps) = calc_re.captures(line) {
                let name = caps[1].to_string();
                let reference = caps[2].trim();
                if let Some(&y) = abs_y.get(reference) {
                    abs_y.insert(name, y);
                }
            }
        }
        abs_y
    }

    // Returns (endpoint_a, endpoint_b) for every composition-boundary wire
    // (`render_comp`'s `to[out=0,in=180]` draws).
    fn comp_wire_endpoints(tikz: &str) -> Vec<(String, String)> {
        let draw_re =
            regex::Regex::new(r"\\draw \(([^)]+)\) to\[out=0,in=180\] \(([^)]+)\);").unwrap();
        draw_re
            .captures_iter(tikz)
            .map(|c| (c[1].to_string(), c[2].to_string()))
            .collect()
    }

    // `absolute_y_by_name` infers each pic's port count `n` by scraping
    // max(k)+1 from the `-in-k`/`-out-k` names actually referenced in the
    // output text -- sufficient for every port a composition boundary
    // exercises, but silently wrong if some *unexercised* higher-numbered
    // port existed (it wouldn't, since a composition boundary always
    // touches every port on that side). This cross-checks that inference
    // against ground truth from `env`'s `visual_arity`/`visual_coarity`,
    // so a future change that stops exercising every port can't silently
    // make the scrape-based `n` wrong without a test noticing.
    fn assert_pic_port_counts_match_env(tikz: &str, env: &Env) {
        let pic_macro_re =
            regex::Regex::new(r"^\s*\\pic(?:\[[^\]]*\])? \(([^)]+)\) at \([^)]+\) \{([^}]+)\};")
                .unwrap();
        let port_ref_re = regex::Regex::new(r"([A-Za-z0-9_]+)-(in|out)-([0-9]+)").unwrap();

        let mut pic_macro: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for line in tikz.lines() {
            if let Some(caps) = pic_macro_re.captures(line) {
                pic_macro.insert(caps[1].to_string(), caps[2].to_string());
            }
        }

        // `render_gen`'s own macro-name choice: `generator.pic` if set,
        // else the generator's own key (see `render_gen`).
        let mut macro_visuals: std::collections::HashMap<String, (u32, u32)> =
            std::collections::HashMap::new();
        for (name, generator) in env.iter() {
            let macro_name = if generator.pic.is_empty() {
                name.clone()
            } else {
                generator.pic.clone()
            };
            let va = generator.visual_arity.unwrap_or(generator.sig.arity);
            let vc = generator.visual_coarity.unwrap_or(generator.sig.coarity);
            macro_visuals.insert(macro_name, (va, vc));
        }

        let mut scraped: std::collections::HashMap<(String, &str), u32> =
            std::collections::HashMap::new();
        for caps in port_ref_re.captures_iter(tikz) {
            let pic_id = caps[1].to_string();
            let side = if &caps[2] == "in" { "in" } else { "out" };
            let k: u32 = caps[3].parse().unwrap();
            let entry = scraped.entry((pic_id, side)).or_insert(0);
            *entry = (*entry).max(k + 1);
        }

        for ((pic_id, side), n) in &scraped {
            let macro_name = pic_macro
                .get(pic_id)
                .unwrap_or_else(|| panic!("no \\pic line found for id {}: {}", pic_id, tikz));
            let (va, vc) = macro_visuals.get(macro_name).copied().unwrap_or_else(|| {
                panic!("no env generator uses pic macro {}: {}", macro_name, tikz)
            });
            let expected = if *side == "in" { va } else { vc };
            assert_eq!(
                *n, expected,
                "scraped port count for {} side {} ({}) is {} but env's visual_{} says {}: {}",
                pic_id, side, macro_name, n, side, expected, tikz
            );
        }
    }

    // Round-3 rework item 2: ground truth for `pic_port_y_frac`, independent
    // of `render_gen`'s own use of it (and independent of the git-excluded
    // `.local/material/generator.tikz`, which this test does not read). A
    // hand-written fixture in the exact `generator.tikz` coordinate syntax
    // (see that file's header comment for the convention: top-first,
    // y_k = 0.5 - k/(n-1), y_0 = 0 for n = 1) for a 1-port, 2-port, and
    // 3-port pic; a tiny parser extracts each `-in-k`/`-out-k` coordinate's
    // declared y, and asserts it equals `pic_port_y_frac(k, n)` -- so a
    // typo in the production formula (or a future pic violating the
    // convention, like the four real `generator.tikz` pics this rework
    // fixed) cannot be masked by testing the formula against its own
    // output. Each block below uses only one side (`in` xor `out`) so a
    // block's coordinate count unambiguously gives that side's `n`.
    #[test]
    fn pic_port_y_frac_matches_generator_tikz_coordinate_syntax_fixture() {
        let fixture = r#"
  onept/.pic = {
    \coordinate (-in-0) at (-0.5, 0);
    \draw (-in-0) to (0,0);
  },
  twopt/.pic = {
    \coordinate (-out-0) at (0.5, 0.5);
    \coordinate (-out-1) at (0.5, -0.5);
    \draw (0,0) to (-out-0);
  },
  threept/.pic = {
    \coordinate (-in-0) at (-0.5, 0.5);
    \coordinate (-in-1) at (-0.5, 0);
    \coordinate (-in-2) at (-0.5, -0.5);
  },
"#;
        let coord_re = regex::Regex::new(
            r"\\coordinate \(-(?:in|out)-([0-9]+)\) at \([^,]+,\s*(-?[0-9]+(?:\.[0-9]+)?)\s*\);",
        )
        .unwrap();

        let mut blocks: Vec<Vec<(u32, f32)>> = Vec::new();
        let mut current: Vec<(u32, f32)> = Vec::new();
        for line in fixture.lines() {
            if line.contains(".pic = {") {
                current = Vec::new();
            } else if line.trim_start().starts_with('}') {
                blocks.push(std::mem::take(&mut current));
            } else if let Some(caps) = coord_re.captures(line) {
                let k: u32 = caps[1].parse().unwrap();
                let y: f32 = caps[2].trim().parse().unwrap();
                current.push((k, y));
            }
        }
        assert_eq!(
            blocks.len(),
            3,
            "expected 3 pic blocks (1/2/3-port) in the fixture, parsed {:?}",
            blocks
        );

        for block in blocks {
            let n = block.len() as u32;
            assert!(n >= 1, "expected a non-empty port block");
            for (k, y) in block {
                let expected = pic_port_y_frac(k, n);
                assert!(
                    (y - expected).abs() < 1e-6,
                    "pic_port_y_frac({}, {}) = {} but generator.tikz-syntax fixture declares y = {}",
                    k,
                    n,
                    expected,
                    y
                );
            }
        }
    }

    // MINOR 7 (round-4 rework): the fixture test above proves
    // `pic_port_y_frac` against a hand-written, in-crate stand-in for
    // `generator.tikz`'s syntax, but never against the *real*,
    // git-excluded file -- so a real pic drifting from the convention
    // (like the 4 pics round 3 found, or the 2dmatrix/2drelation stub-
    // routing and coaffine port-name bugs round 4 found) would go
    // undetected by CI. This test reads the real file when present and
    // checks every declared `-in-k`/`-out-k` y against `pic_port_y_frac`
    // *and* every x against `+/-(width/2)` for each `generators.toml`
    // entry using that pic (matching `render_gen`'s own
    // `pic = if generator.pic.is_empty() { name } else { &generator.pic }`
    // fallback). `#[ignore]`d (not run by default `cargo test`) since
    // `.local/` is git-excluded and won't exist in every checkout/CI --
    // run explicitly with `cargo test -- --ignored` when `.local/` is
    // present.
    #[test]
    #[ignore]
    fn pic_port_y_frac_matches_real_generator_tikz() {
        let tikz_path = format!(
            "{}/.local/material/generator.tikz",
            env!("CARGO_MANIFEST_DIR")
        );
        let toml_path = format!(
            "{}/.local/material/generators.toml",
            env!("CARGO_MANIFEST_DIR")
        );
        let tikz = match std::fs::read_to_string(&tikz_path) {
            Ok(t) => t,
            Err(_) => return, // `.local/` not present in this checkout; nothing to check.
        };
        let generators = crate::codegen::config::load_config(&toml_path)
            .expect("load .local/material/generators.toml");

        let pic_name_re = regex::Regex::new(r"^\s*([A-Za-z0-9]+)/\.pic\s*=\s*\{").unwrap();
        let coord_re = regex::Regex::new(
            r"\\coordinate \(-(in|out)-([0-9]+)\)\s*at\s*\(\s*(-?[0-9]+(?:\.[0-9]+)?)\s*,\s*(-?[0-9]+(?:\.[0-9]+)?)\s*\);",
        )
        .unwrap();

        // pic name -> side -> k -> (x, y)
        type PortsBySide<'a> =
            std::collections::HashMap<&'a str, std::collections::HashMap<u32, (f32, f32)>>;
        let mut current_pic: Option<String> = None;
        let mut pics: std::collections::HashMap<String, PortsBySide> =
            std::collections::HashMap::new();
        for line in tikz.lines() {
            if let Some(caps) = pic_name_re.captures(line) {
                current_pic = Some(caps[1].to_string());
            } else if let Some(caps) = coord_re.captures(line) {
                let side = if &caps[1] == "in" { "in" } else { "out" };
                let k: u32 = caps[2].parse().unwrap();
                let x: f32 = caps[3].trim().parse().unwrap();
                let y: f32 = caps[4].trim().parse().unwrap();
                let pic_name = current_pic
                    .as_ref()
                    .unwrap_or_else(|| panic!("coordinate outside any /.pic block: {}", line));
                pics.entry(pic_name.clone())
                    .or_default()
                    .entry(side)
                    .or_default()
                    .insert(k, (x, y));
            }
        }
        assert!(
            !pics.is_empty(),
            "expected to parse at least one pic from {}",
            tikz_path
        );

        for (name, generator) in &generators {
            let pic_name = if generator.pic.is_empty() {
                name.as_str()
            } else {
                generator.pic.as_str()
            };
            let Some(sides) = pics.get(pic_name) else {
                continue; // pic not defined in generator.tikz (e.g. a test-only generator elsewhere); not this test's concern.
            };
            let half_width = generator.width / 2.0;
            for (side, effective_n) in [
                ("in", generator.visual_arity.unwrap_or(generator.sig.arity)),
                (
                    "out",
                    generator.visual_coarity.unwrap_or(generator.sig.coarity),
                ),
            ] {
                if effective_n == 0 {
                    continue;
                }
                let expected_x = if side == "in" {
                    -half_width
                } else {
                    half_width
                };
                let ports = sides.get(side).unwrap_or_else(|| {
                    panic!(
                        "generator {} (pic {}) expects {} {}-port(s) but generator.tikz has none",
                        name, pic_name, effective_n, side
                    )
                });
                assert_eq!(
                    ports.len() as u32,
                    effective_n,
                    "generator {} (pic {}): generator.tikz has {} {}-port(s), generators.toml visual count says {}",
                    name,
                    pic_name,
                    ports.len(),
                    side,
                    effective_n
                );
                for (&k, &(x, y)) in ports {
                    let expected_y = pic_port_y_frac(k, effective_n);
                    assert!(
                        (y - expected_y).abs() < 1e-6,
                        "generator {} (pic {}) -{}-{}: y = {} but pic_port_y_frac({}, {}) = {}",
                        name,
                        pic_name,
                        side,
                        k,
                        y,
                        k,
                        effective_n,
                        expected_y
                    );
                    assert!(
                        (x - expected_x).abs() < 1e-6,
                        "generator {} (pic {}) -{}-{}: x = {} but expected +/-(width/2) = {} (width = {})",
                        name,
                        pic_name,
                        side,
                        k,
                        x,
                        expected_x,
                        generator.width
                    );
                }
            }
        }
    }

    // Asserts every composition wire whose *both* endpoints resolve to a
    // known absolute y has equal y (straight wire), per spec requirement 1.
    // Now that generator pic ports also resolve (see `absolute_y_by_name`),
    // this covers real generator-to-generator wires too, not just id/swap
    // ones; only a boundary genuinely mixing a non-conforming (unresolved)
    // port would be skipped, which does not occur for any generator in
    // `env()`/`generators.toml` today. Returns how many pairs were actually
    // checked, so callers can assert the check wasn't vacuous where it
    // shouldn't be.
    fn assert_comp_wires_straight_when_known(tikz: &str) -> usize {
        assert_comp_wires_straight_when_known_tol(tikz, 1e-3)
    }

    // Round-3 rework item 3: the spec's acceptance criterion is judged at
    // tolerance 1e-4 (tighter than the 1e-3 used elsewhere in this file for
    // synthetic Id/Tensor-only fixtures, where the looser bound was never
    // load-bearing). `{:.3}`-formatted output coordinates round to the
    // nearest 0.0005, so two independently-rounded but logically-equal
    // values could in principle differ by up to ~0.001 -- 1e-4 is tight
    // enough to catch a real bend while still passing on every expression
    // this file actually asserts exact straightness for (verified by
    // running each below; none hit that rounding edge case in practice).
    fn assert_comp_wires_exactly_straight(tikz: &str) -> usize {
        assert_comp_wires_straight_when_known_tol(tikz, 1e-4)
    }

    fn assert_comp_wires_straight_when_known_tol(tikz: &str, tol: f32) -> usize {
        let abs_y = absolute_y_by_name(tikz);
        let mut checked = 0;
        for (a, b) in comp_wire_endpoints(tikz) {
            if let (Some(&ya), Some(&yb)) = (abs_y.get(&a), abs_y.get(&b)) {
                assert!(
                    (ya - yb).abs() < tol,
                    "composition wire ({} -> {}) is not straight: y {} vs {}\nfull output:\n{}",
                    a,
                    b,
                    ya,
                    yb,
                    tikz
                );
                checked += 1;
            }
        }
        checked
    }

    // R1/R3: `f;g` with a *constant*-pitch boundary (Id(2) into a Tensor
    // whose top branch is shifted up by an unrelated blank Id(0) lane) must
    // align `g` to `f`'s ports by the mean dy, not center each side
    // independently on the combined bbox. Left = Id(2) (right ys 0.5,1.0);
    // Right = Id(2) tensor Id(0) (left ys 1.5,2.0, offset by the blank
    // lane's height): old bbox-centering shifted these by different amounts
    // (0.5 vs 0.0) and left them bent (diff 0.5 on both wires); the fix
    // shifts Right by the constant dy (-1.0) so both wires are exactly
    // straight.
    #[test]
    fn comp_aligns_constant_pitch_ports_instead_of_bbox_centering() {
        let expr = Expr::Comp(
            Box::new(Expr::Id(2)),
            Box::new(Expr::Tensor(Box::new(Expr::Id(2)), Box::new(Expr::Id(0)))),
        );
        let out = generate(&expr, &env()).expect("generate comp");
        let checked = assert_comp_wires_straight_when_known(&out);
        assert_eq!(
            checked, 2,
            "expected both id(2) wires to be checkable coordinates: {}",
            out
        );

        // R3: renormalized height must cover both shifted bodies (old
        // height was 2.0 too, but via blunt centering); gap must fall back
        // to COMP_GAP since aligned ports have max_abs_dy == 0.
        let coords = parse_coords(&out);
        let max_y = coords.iter().map(|(_, y)| *y).fold(f32::MIN, f32::max);
        let min_y = coords.iter().map(|(_, y)| *y).fold(f32::MAX, f32::min);
        assert!(
            min_y >= -1e-3,
            "expected no negative y after renormalization: {}",
            out
        );
        assert!(
            max_y - min_y <= 2.0 + 1e-3,
            "expected renormalized height ~2.0, got span {}: {}",
            max_y - min_y,
            out
        );
    }

    // R1: when the boundary pitch differs per-wire (no single dy makes every
    // pair exactly equal), the fix must still shift by the *mean* dy,
    // producing a symmetric bend: the wires' signed deviations from
    // straight must average to zero, not an arbitrary bbox-centered offset.
    // `render_gen`'s box-margin fix (BLOCKER 1, round 4) makes a boundary
    // between two *uniformly* `MIN_PITCH`-pitched sides -- a single
    // generator's own max-port side (e.g. `mult`'s 2-in side), `id`, `swap`,
    // or a tensor of equal-height blocks -- exactly straight, not merely
    // mean-zero. `generator.tikz`'s unit frame is never stretched by a
    // taller box, though -- only *margin* grows, not the ports' own
    // +/-0.5 span -- so an n>=3-port generator's internal pitch
    // (`1.0/(n-1)`, e.g. 0.25 for n=5) is still strictly less than
    // `MIN_PITCH`. `Id(5)` against the synthetic 5-port `penta` generator
    // (see `env_with_penta`) demonstrates this real, remaining,
    // architectural mismatch (not a crossing bug: still mean-zero,
    // symmetric, non-crossing). A *different* remaining mismatch --
    // asymmetric visual_arity/visual_coarity within one generator, e.g.
    // `mult`'s 2-in/1-out -- is pinned down separately by
    // `mult_tensor_ge_seq_id2_bends_by_documented_asymmetric_margin` below.
    #[test]
    fn comp_shifts_by_mean_dy_for_differing_pitch() {
        let expr = Expr::Comp(
            Box::new(Expr::Id(5)),
            Box::new(Expr::Gen {
                name: "penta".into(),
                args: vec![],
            }),
        );
        let out = generate(&expr, &env_with_penta()).expect("generate comp");
        assert_pic_port_counts_match_env(&out, &env_with_penta());
        let abs_y = absolute_y_by_name(&out);
        let pairs = comp_wire_endpoints(&out);
        assert_eq!(pairs.len(), 5, "expected five composition wires: {}", out);
        let resolved: Vec<(f32, f32)> = pairs.iter().map(|(a, b)| (abs_y[a], abs_y[b])).collect();
        assert_no_crossing(&resolved, &out);
        let diffs: Vec<f32> = resolved.iter().map(|(a, b)| a - b).collect();
        assert_mean_dy_zero(&diffs, &out);
        assert!(
            diffs.iter().any(|d| d.abs() > 1e-3),
            "expected a genuine bend (differing pitch), got diffs {:?}: {}",
            diffs,
            out
        );
    }

    // Pinning test (round 5, item 1): the box-height fix's asymmetric-
    // visual-arity trade-off (see the comment on `port_count_height` in
    // `render_gen`) is an accepted, deliberate limitation, not a bug -- but
    // it must not silently change magnitude. `mult` (2-in/1-out) tensored
    // with `ge` (1-out) exposes a coarity-2 boundary `[ge-out (0.5),
    // mult-out (2.0)]` against `id(2)`'s arity-2 boundary `[0.5, 1.5]`
    // (mult's single output sits at the box's exact centre, `height/2`,
    // with `height/2` margin on each side, instead of the `MIN_PITCH/2`
    // margin every `id` wire gets) -- a genuine, expected +/-0.25 mean-zero
    // bend, matching the hand-check in the round-5 task spec exactly.
    #[test]
    fn mult_tensor_ge_seq_id2_bends_by_documented_asymmetric_margin() {
        let expr = Expr::Comp(
            Box::new(Expr::Tensor(
                Box::new(Expr::Gen {
                    name: "mult".into(),
                    args: vec![],
                }),
                Box::new(Expr::Gen {
                    name: "ge".into(),
                    args: vec![],
                }),
            )),
            Box::new(Expr::Id(2)),
        );
        let out = generate(&expr, &env()).expect("generate (mult*ge);id(2)");
        assert_pic_port_counts_match_env(&out, &env());
        let abs_y = absolute_y_by_name(&out);
        let pairs = comp_wire_endpoints(&out);
        assert_eq!(pairs.len(), 2, "expected two composition wires: {}", out);
        let diffs: Vec<f32> = pairs.iter().map(|(a, b)| abs_y[a] - abs_y[b]).collect();
        let resolved: Vec<(f32, f32)> = pairs.iter().map(|(a, b)| (abs_y[a], abs_y[b])).collect();
        assert_mean_dy_zero(&diffs, &out);
        assert_no_crossing(&resolved, &out);
        for d in &diffs {
            assert!(
                (d.abs() - 0.25).abs() < 1e-3,
                "expected the documented +/-0.25 asymmetric-margin bend, got diffs {:?}: {}",
                diffs,
                out
            );
        }
    }

    // R2/R4: nested inside a tensor ("comp inside tensor"), the same
    // constant-pitch alignment must survive `render_tensor`'s x-only
    // reanchoring plus its uniform vertical `top_off` shift.
    #[test]
    fn comp_alignment_survives_nesting_inside_tensor() {
        let comp = Expr::Comp(
            Box::new(Expr::Id(2)),
            Box::new(Expr::Tensor(Box::new(Expr::Id(2)), Box::new(Expr::Id(0)))),
        );
        let expr = Expr::Tensor(Box::new(comp), Box::new(Expr::Id(1)));
        let out = generate(&expr, &env()).expect("generate tensor-of-comp");
        let checked = assert_comp_wires_straight_when_known(&out);
        assert_eq!(
            checked, 2,
            "expected the nested comp's wires to remain checkable: {}",
            out
        );
    }

    // R2/R4: nested as the right-hand side of an outer comp ("tensor inside
    // comp", exercised directly by `comp_aligns_constant_pitch_ports_...`
    // above) composed *again* with a further Id(1) -- two `render_comp`
    // hops in a row -- must not reintroduce a bend at either boundary.
    #[test]
    fn comp_alignment_survives_nesting_inside_another_comp() {
        let inner = Expr::Comp(
            Box::new(Expr::Id(2)),
            Box::new(Expr::Tensor(Box::new(Expr::Id(2)), Box::new(Expr::Id(0)))),
        );
        let expr = Expr::Comp(Box::new(inner), Box::new(Expr::Id(2)));
        let out = generate(&expr, &env()).expect("generate comp-of-comp");
        let checked = assert_comp_wires_straight_when_known(&out);
        assert_eq!(
            checked, 4,
            "expected both boundaries' wires (2 + 2) to be checkable: {}",
            out
        );
    }

    // --- render_tensor bottom-first reordering regression tests (rework
    // requirements 1-2 from the reviewer) ---

    // Requirement 1: `render_tensor`'s reordering must relocate `Swap`'s
    // whole exposed sub-vector as a block (bottom branch first, top branch
    // second) without reaching *into* `Swap`'s own internal port order,
    // which stays whatever `render_swap` already produced (swap rendering
    // is an explicit spec non-goal -- this only guards that nesting it in
    // a `Tensor` doesn't scramble which name ends up in which slot).
    #[test]
    fn render_tensor_relocates_swaps_whole_block_bottom_first() {
        let generators = env();
        let mut r = Renderer::new(&generators);
        let layout = r
            .render(&Expr::Tensor(
                Box::new(Expr::Swap(1, 1)),
                Box::new(Expr::Id(1)),
            ))
            .expect("render tensor(swap, id(1))");
        assert_eq!(
            layout.right.len(),
            3,
            "expected 1 (id) + 2 (swap) ports: {:?}",
            layout.right.iter().map(|a| &a.name).collect::<Vec<_>>()
        );
        assert!(
            layout.right[0].name.starts_with("id_out_"),
            "expected id(1) (bottom branch) at vector position 0: {:?}",
            layout.right.iter().map(|a| &a.name).collect::<Vec<_>>()
        );
        assert!(
            layout.right[1].name.starts_with("sw_out_")
                && layout.right[2].name.starts_with("sw_out_"),
            "expected swap's own 2-port block at vector positions 1,2, in swap's own exposed bottom-first order: {:?}",
            layout.right.iter().map(|a| &a.name).collect::<Vec<_>>()
        );
    }

    // Requirement 1 (continued): composing `Tensor(Swap(1,1), Id(1))` with
    // an identically-shaped boundary must still connect id(1)'s wire to
    // id(1)'s wire and swap's ports to swap's ports -- not a wire scrambled
    // across the block boundary -- proving the reordering didn't corrupt
    // *which physical wire* ends up at which vector position on either
    // side of a `render_comp`.
    #[test]
    fn swap_nested_in_tensor_still_connects_matching_physical_wires_through_comp() {
        let shape = || Expr::Tensor(Box::new(Expr::Swap(1, 1)), Box::new(Expr::Id(1)));
        let expr = Expr::Comp(Box::new(shape()), Box::new(shape()));
        let out = generate(&expr, &env()).expect("generate comp(tensor(swap,id), tensor(swap,id))");
        let pairs = comp_wire_endpoints(&out);
        assert_eq!(pairs.len(), 3, "expected 3 composition wires: {}", out);
        assert!(
            pairs
                .iter()
                .any(|(a, b)| a.contains("id_") && b.contains("id_")),
            "expected id(1)'s wire to connect id-to-id, not to a swap port: {:?} in {}",
            pairs,
            out
        );
        let swap_to_swap = pairs
            .iter()
            .filter(|(a, b)| a.contains("sw_") && b.contains("sw_"))
            .count();
        assert_eq!(
            swap_to_swap, 2,
            "expected swap's 2 ports to connect swap-to-swap, not to id(1)'s port: {:?} in {}",
            pairs, out
        );
    }

    // Parses `render_swap`'s own `\draw (X) .. controls ... .. (Y);` bezier
    // lines and asserts the crossing swap(1,1) implements is genuine -- the
    // internal draw connects *opposite* physical positions (its two ends
    // resolve to different absolute y), not a no-op pass-through that would
    // happen to land both ends at the same y (indistinguishable, by y
    // alone, from a genuinely straight identity wire).
    fn assert_swap1x1_not_cancelled(tikz: &str) {
        let bezier_re = regex::Regex::new(
            r"\\draw \(([^)]+)\) \.\. controls \([^)]+\) and \([^)]+\) \.\. \(([^)]+)\);",
        )
        .unwrap();
        let abs_y = absolute_y_by_name(tikz);
        let mut draws: Vec<(f32, f32)> = Vec::new();
        for caps in bezier_re.captures_iter(tikz) {
            if let (Some(&ya), Some(&yb)) = (abs_y.get(&caps[1]), abs_y.get(&caps[2])) {
                draws.push((ya, yb));
            }
        }
        assert_eq!(
            draws.len(),
            2,
            "expected exactly 2 internal swap(1,1) bezier draws: {}",
            tikz
        );
        for (ya, yb) in &draws {
            assert!(
                (ya - yb).abs() > 1e-3,
                "expected swap(1,1)'s internal draw to cross opposite physical positions, got equal y {} -> {} (looks cancelled/no-op): {}",
                ya,
                yb,
                tikz
            );
        }
    }

    // Rework item 1's own required regression tests: `copy ; swap(1,1)`,
    // `(ge * antipode) ; swap(1,1)`, and `swap(1,1) ; (ge * antipode)` must
    // each have (a) no crossing at the composition *boundary* (vector index
    // 0 = bottom on both sides, per `render_swap`'s bottom-first fix), and
    // (b) a swap that is genuinely *not* cancelled -- its own internal
    // routing still crosses the physically-bottom wire to the
    // physically-top port, and vice versa (`assert_swap1x1_not_cancelled`).
    #[test]
    fn copy_seq_swap1x1_no_crossing_and_not_cancelled() {
        let out = crate::compile("copy ; swap(1,1)", &env()).expect("compile");
        let pairs = comp_wire_endpoints(&out);
        assert_eq!(pairs.len(), 2, "expected two composition wires: {}", out);
        let abs_y = absolute_y_by_name(&out);
        let resolved: Vec<(f32, f32)> = pairs.iter().map(|(a, b)| (abs_y[a], abs_y[b])).collect();
        assert_no_crossing(&resolved, &out);
        assert_swap1x1_not_cancelled(&out);
    }

    #[test]
    fn ge_tensor_antipode_seq_swap1x1_no_crossing_and_not_cancelled() {
        let out = crate::compile("(ge * antipode) ; swap(1,1)", &env()).expect("compile");
        let pairs = comp_wire_endpoints(&out);
        assert_eq!(pairs.len(), 2, "expected two composition wires: {}", out);
        let abs_y = absolute_y_by_name(&out);
        let resolved: Vec<(f32, f32)> = pairs.iter().map(|(a, b)| (abs_y[a], abs_y[b])).collect();
        assert_no_crossing(&resolved, &out);
        assert_swap1x1_not_cancelled(&out);
    }

    #[test]
    fn swap1x1_seq_ge_tensor_antipode_no_crossing_and_not_cancelled() {
        let out = crate::compile("swap(1,1) ; (ge * antipode)", &env()).expect("compile");
        let pairs = comp_wire_endpoints(&out);
        assert_eq!(pairs.len(), 2, "expected two composition wires: {}", out);
        let abs_y = absolute_y_by_name(&out);
        let resolved: Vec<(f32, f32)> = pairs.iter().map(|(a, b)| (abs_y[a], abs_y[b])).collect();
        assert_no_crossing(&resolved, &out);
        assert_swap1x1_not_cancelled(&out);
    }

    // Requirement 2 (part 1): `f * g` must still draw `f` physically above
    // `g` on the page -- `render_tensor`'s `top_off` placement logic is
    // untouched by this rework, only the *order ports are exposed in*
    // changed. `mult` (top) and `ge` (bottom), both single-output
    // generators, land at exactly two absolute y's (bottom-first vector
    // order: index 0 = `ge`, index 1 = `mult`), and index 1's y must be
    // strictly greater.
    #[test]
    fn tensor_still_draws_top_branch_physically_above_bottom() {
        let generators = env();
        let mut r = Renderer::new(&generators);
        let layout = r
            .render(&Expr::Tensor(
                Box::new(Expr::Gen {
                    name: "mult".into(),
                    args: vec![],
                }),
                Box::new(Expr::Gen {
                    name: "ge".into(),
                    args: vec![],
                }),
            ))
            .expect("render tensor(mult, ge)");
        assert_eq!(layout.right.len(), 2, "expected 1 (ge) + 1 (mult) port");
        let bottom_y = layout.right[0].y;
        let top_y = layout.right[1].y;
        assert!(
            top_y > bottom_y,
            "expected mult (top branch) strictly above ge (bottom branch): top_y={}, bottom_y={}",
            top_y,
            bottom_y
        );
    }

    // Maps each pic id (e.g. "g3") to the generator/pic style name it was
    // instantiated with (e.g. "ge"), by parsing the emitted
    // `\pic ... (g3) at (...) {ge};` line -- used to identify *which*
    // generator a composition wire's endpoint belongs to without relying
    // on the numeric id allocation order.
    fn pic_id_to_style(tikz: &str) -> std::collections::HashMap<String, String> {
        let re = regex::Regex::new(r"\\pic(?:\[[^\]]*\])? \(([^)]+)\) at \([^)]+\) \{([^}]+)\};")
            .unwrap();
        re.captures_iter(tikz)
            .map(|c| (c[1].to_string(), c[2].to_string()))
            .collect()
    }

    fn pic_id_of(port_name: &str) -> &str {
        port_name
            .split("-in-")
            .next()
            .unwrap()
            .split("-out-")
            .next()
            .unwrap()
    }

    // Requirement 2 (part 2): `(a * b) ; (c * d)` must connect a->c and
    // b->d (matching top-to-top, bottom-to-bottom), not crossed/swapped,
    // regardless of which vector-order convention `render_tensor` uses --
    // both sides go through the *same* reordering rule, so the pairing
    // must still respect top/bottom correspondence. Uses `ge`/`antipode`
    // (both arity1/coarity1, distinct pic styles) so each endpoint's
    // originating generator can be identified unambiguously by name.
    #[test]
    fn tensor_then_comp_connects_corresponding_branches_not_swapped() {
        let ge = || Expr::Gen {
            name: "ge".into(),
            args: vec![],
        };
        let antipode = || Expr::Gen {
            name: "antipode".into(),
            args: vec![],
        };
        // a=ge, b=antipode (top/bottom of the left tensor); c=ge, d=antipode
        // (top/bottom of the right tensor, same styles in the same
        // top/bottom roles) -- so a->c and b->d both connect *matching*
        // styles, while a crossed/swapped mapping (a->d, b->c) would
        // connect *mismatched* styles (ge->antipode), making the bug
        // detectable by a simple style-equality check.
        let expr = Expr::Comp(
            Box::new(Expr::Tensor(Box::new(ge()), Box::new(antipode()))),
            Box::new(Expr::Tensor(Box::new(ge()), Box::new(antipode()))),
        );
        let out = generate(&expr, &env()).expect("generate (a*b);(c*d)");
        assert_pic_port_counts_match_env(&out, &env());
        let styles = pic_id_to_style(&out);
        let pairs = comp_wire_endpoints(&out);
        assert_eq!(pairs.len(), 2, "expected two composition wires: {}", out);
        for (a, b) in &pairs {
            let a_style = &styles[pic_id_of(a)];
            let b_style = &styles[pic_id_of(b)];
            assert_eq!(
                a_style, b_style,
                "expected a->c (ge->ge) and b->d (antipode->antipode), not crossed, got {} ({}) -> {} ({}) in {}",
                a, a_style, b, b_style, out
            );
        }
    }

    // Acceptance criteria expressions (spec + task): parse the real emitted
    // TikZ for `mult;ge` and `((ge;ge) * ge) ; mult` against `env()` (a
    // faithful in-crate mirror of `.local/material/generators.toml`'s
    // relevant entries -- see `env()`'s own doc comment; kept in-crate so
    // these tests run in CI without reading the git-excluded `.local/`
    // tree), and confirm every composition wire is straight. Now that
    // `render_gen` gives every pic port a known y, every boundary in these
    // expressions is fully resolvable, so `checked` must equal the total
    // wire count -- not just be nonzero -- to catch a future regression
    // that silently made a pair unresolvable again.
    #[test]
    fn validator_expr_mult_seq_ge_wires_are_exactly_straight() {
        let out = crate::compile("mult;ge", &env()).expect("compile");
        assert_pic_port_counts_match_env(&out, &env());
        let pairs = comp_wire_endpoints(&out);
        assert!(
            !pairs.is_empty(),
            "expected at least one composition wire: {}",
            out
        );
        let checked = assert_comp_wires_exactly_straight(&out);
        assert_eq!(
            checked,
            pairs.len(),
            "expected every comp wire's endpoints to resolve now that generator pic ports are known: {}",
            out
        );
    }

    #[test]
    fn validator_expr_ge_ge_tensor_ge_seq_mult_wires_are_exactly_straight() {
        let out = crate::compile("((ge;ge) * ge) ; mult", &env()).expect("compile");
        assert_pic_port_counts_match_env(&out, &env());
        let pairs = comp_wire_endpoints(&out);
        assert!(
            !pairs.is_empty(),
            "expected at least one composition wire: {}",
            out
        );
        let checked = assert_comp_wires_exactly_straight(&out);
        assert_eq!(
            checked,
            pairs.len(),
            "expected every comp wire's endpoints to resolve now that generator pic ports are known: {}",
            out
        );
    }

    // This equation has 5 separate `render_comp` boundaries producing, in
    // emission order, 10 composition wires: two 1-wire boundaries
    // (`codiscard;copy`, `mult;ge`), one 2-wire boundary (`comult;(ge*id(1))`),
    // and two 3-wire boundaries (`(id(1)*(codiscard;copy));((id(1)*antipode)*id(1))`
    // and the outermost comp absorbing that into `((mult;ge)*id(1))`).
    // `render_gen` now grows a generator's box to
    // `max(config height, max(visual_arity, visual_coarity, 1) * MIN_PITCH)`
    // instead of leaving it pinned at the 1.0 default, so a multi-port
    // generator's ports get the same half-`MIN_PITCH` margin `render_id`/
    // `render_swap` already gave their own wires (`y = (i+0.5)*MIN_PITCH`).
    // That was the actual root cause of the previously-"accepted" 3-wire
    // bend in this equation's two 3-wire boundaries (an `id(1)` lane mixed
    // with a then-unmargined 2-port generator's ports): with matching
    // margins on both sides, every one of these 5 boundaries -- not just
    // the 1- and 2-wire ones -- is exactly straight. There is no longer any
    // "n=3 non-uniform pitch" limitation to document here.
    #[test]
    fn validator_full_equation_all_10_wires_are_exactly_straight() {
        let out = crate::compile(
            "comult;(ge * id(1)) = (id(1) * (codiscard;copy));(id(1) * antipode) * id(1);((mult;ge) * id(1))",
            &env(),
        )
        .expect("compile");
        assert_pic_port_counts_match_env(&out, &env());
        let pairs = comp_wire_endpoints(&out);
        assert_eq!(
            pairs.len(),
            10,
            "expected 10 composition wires across the equation: {}",
            out
        );
        let checked = assert_comp_wires_exactly_straight(&out);
        assert_eq!(
            checked,
            pairs.len(),
            "expected all 10 comp wires to resolve and be exactly straight: {}",
            out
        );
    }

    // Direct proof of the BLOCKER-1 root cause: without the margin fix,
    // `Tensor(mult, mult)` exposed its 4 combined arity-side ports at
    // [0.0, 1.0, 1.0, 2.0] -- two physically distinct wires (bottom mult's
    // top port, top mult's bottom port) landing on the identical y 1.0,
    // which is not just "not straight" but actively indistinguishable.
    // With the margin fix each generator box is exactly as tall as its own
    // `MIN_PITCH`-spaced ports need, so stacking two via `Tensor` gives 4
    // strictly increasing, evenly-spaced y's.
    #[test]
    fn mult_tensor_mult_has_distinct_port_ys() {
        let generators = env();
        let mut r = Renderer::new(&generators);
        let layout = r
            .render(&Expr::Tensor(
                Box::new(Expr::Gen {
                    name: "mult".into(),
                    args: vec![],
                }),
                Box::new(Expr::Gen {
                    name: "mult".into(),
                    args: vec![],
                }),
            ))
            .expect("render mult*mult");
        let ys: Vec<f32> = layout.left.iter().map(|a| a.y).collect();
        assert_eq!(
            ys,
            vec![0.5, 1.5, 2.5, 3.5],
            "expected 4 distinct, evenly-MIN_PITCH-spaced ports, got {:?}",
            ys
        );
        let unique: std::collections::HashSet<i64> =
            ys.iter().map(|y| (y * 1000.0).round() as i64).collect();
        assert_eq!(
            unique.len(),
            ys.len(),
            "expected all 4 ports at distinct y (no two physically different wires sharing a y), got {:?}",
            ys
        );
    }

    // Asserts that connections listed in vector-index order (as
    // `comp_wire_endpoints` returns them, which matches `render_comp`'s own
    // emission order) never invert their relative vertical order between
    // the two sides of the boundary -- i.e. a wire from a lower source port
    // never lands on a destination port higher than a higher source's,
    // which would visually cross.
    fn assert_no_crossing(pairs: &[(f32, f32)], out: &str) {
        // A tie on either side (e.g. two floating-point paths landing on
        // the same logical y up to rounding) is not a crossing -- only a
        // strict, clearly-signed inversion on *both* sides is.
        const EPS: f32 = 1e-3;
        for i in 0..pairs.len() {
            for j in (i + 1)..pairs.len() {
                let (a_i, b_i) = pairs[i];
                let (a_j, b_j) = pairs[j];
                if (a_i - a_j).abs() > EPS && (b_i - b_j).abs() > EPS {
                    assert_eq!(
                        (a_i - a_j).signum(),
                        (b_i - b_j).signum(),
                        "composition wires cross: pair {} ({},{}) vs pair {} ({},{}) in: {}",
                        i,
                        a_i,
                        b_i,
                        j,
                        a_j,
                        b_j,
                        out
                    );
                }
            }
        }
    }

    // Asserts a group of composition wires (all from the *same*
    // `render_comp` boundary) exhibits a genuine symmetric bend around
    // mean dy == 0, per the spec's own accepted "differing pitch" case --
    // not an arbitrary/asymmetric bbox-centered offset.
    fn assert_mean_dy_zero(diffs: &[f32], out: &str) {
        let mean: f32 = diffs.iter().sum::<f32>() / diffs.len() as f32;
        assert!(
            mean.abs() < 1e-3,
            "expected mean dy ~0 (symmetric bend around the mean), got diffs {:?}: {}",
            diffs,
            out
        );
    }

    // Reviewer-requested direct proof (MAJOR 1 rework, tightened by round-3
    // rework item 3): `id(2)`'s two monotonic bottom-first wires feed
    // `mult`'s two ports, whose visual-index -> vector-position mapping
    // `render_gen` reverses to also be bottom-first -- so the two
    // composition wires must not cross. With `MIN_PITCH` now `1.0` (round-3
    // item 3), `id`'s wire-to-wire pitch exactly matches a 2-port
    // generator's port pitch (`1.0/(n-1) = 1.0/1 = 1.0` for `mult`'s 2-port
    // side, from the fixed +/-0.5 unit frame, independent of box height),
    // so this boundary is not just non-crossing but now *exactly* straight.
    #[test]
    fn id2_seq_mult_wires_are_exactly_straight() {
        let out = crate::compile("id(2);mult", &env()).expect("compile");
        assert_pic_port_counts_match_env(&out, &env());
        let pairs = comp_wire_endpoints(&out);
        assert_eq!(pairs.len(), 2, "expected two composition wires: {}", out);
        let checked = assert_comp_wires_exactly_straight(&out);
        assert_eq!(checked, 2, "expected both wires to resolve: {}", out);
    }

    // Reviewer-requested direct proof (MAJOR 1 rework, tightened by round-3
    // rework item 3): the mirror case, `comult`'s two output ports feeding
    // `id(2)`'s two input wires -- now exactly straight for the same
    // matching-pitch reason as `id2_seq_mult_wires_are_exactly_straight`.
    #[test]
    fn comult_seq_id2_wires_are_exactly_straight() {
        let out = crate::compile("comult;id(2)", &env()).expect("compile");
        assert_pic_port_counts_match_env(&out, &env());
        let pairs = comp_wire_endpoints(&out);
        assert_eq!(pairs.len(), 2, "expected two composition wires: {}", out);
        let checked = assert_comp_wires_exactly_straight(&out);
        assert_eq!(checked, 2, "expected both wires to resolve: {}", out);
    }

    // Round-3 rework item 3: `comult;(ge * id(1))` -- one of the full
    // validator equation's own boundaries -- must also be exactly straight
    // in isolation (not just as part of the larger equation below).
    #[test]
    fn comult_seq_ge_tensor_id1_wires_are_exactly_straight() {
        let out = crate::compile("comult;(ge * id(1))", &env()).expect("compile");
        assert_pic_port_counts_match_env(&out, &env());
        let pairs = comp_wire_endpoints(&out);
        assert_eq!(pairs.len(), 2, "expected two composition wires: {}", out);
        let checked = assert_comp_wires_exactly_straight(&out);
        assert_eq!(checked, 2, "expected both wires to resolve: {}", out);
    }
}
