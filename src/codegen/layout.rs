// Layout geometry shared by the renderer: per-wire anchors and the minimum
// wire pitch that makes picture height grow with wire count instead of
// staying fixed at 1.0.

pub(crate) const MIN_PITCH: f32 = 1.0;

/// A named TikZ anchor with a known y position.
///
/// Every anchor in this renderer is placed by the layout engine itself:
/// id/swap, exposed coordinates from reanchoring, and (per
/// `generator.tikz`'s documented top-first, evenly-spaced port convention,
/// which every pic in `generator.tikz` now conforms to) every generator
/// `\pic`'s in/out ports -- see `render_gen`. `y` is a plain `f32`: every
/// code path that builds an `Anchor` (`render_id`/`render_swap`/
/// `render_gen`/`reanchor_to`) always knows the position it is placing, so
/// there is no "unknown y" case left to represent.
#[derive(Debug, Clone)]
pub(crate) struct Anchor {
    pub name: String,
    pub y: f32,
}

impl Anchor {
    pub fn known(name: String, y: f32) -> Self {
        Self { name, y }
    }

    pub fn shifted(&self, dy: f32) -> Self {
        Self {
            name: self.name.clone(),
            y: self.y + dy,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Layout {
    pub width: f32,
    pub height: f32,
    pub left: Vec<Anchor>,
    pub right: Vec<Anchor>,
    pub body: String,
}
