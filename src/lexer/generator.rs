pub struct Generator {
    pub value: String,
    pub params: Vec<String>,
    pub arity: u32,
    pub coarity: u32,
    pub visual_arity: Option<u32>,
    pub visual_coarity: Option<u32>,
    pub symbol: String,
    pub pic: String,
    pub width: f32,
    pub height: f32,
}
