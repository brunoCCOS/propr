// Layout geometry shared by the renderer: per-wire anchors and the minimum
// wire pitch that makes picture height grow with wire count instead of
// staying fixed at 1.0.

pub(crate) const MIN_PITCH: f32 = 0.5;

/// A named TikZ anchor with an optional known y position.
///
/// `y` is `Some` for anchors the layout engine places itself (id/swap,
/// exposed coordinates from reanchoring) and `None` for anchors that live
/// inside a user-drawn `\pic` -- propr never sees inside a pic, so their true
/// vertical position is unknown at layout time.
#[derive(Debug, Clone)]
pub(crate) struct Anchor {
    pub name: String,
    pub y: Option<f32>,
}

impl Anchor {
    pub fn known(name: String, y: f32) -> Self {
        Self { name, y: Some(y) }
    }

    pub fn unknown(name: String) -> Self {
        Self { name, y: None }
    }

    pub fn shifted(&self, dy: f32) -> Self {
        Self {
            name: self.name.clone(),
            y: self.y.map(|y| y + dy),
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
