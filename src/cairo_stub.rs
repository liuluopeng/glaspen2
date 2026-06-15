/// Minimal cairo type stubs for cross-compilation type checking.
/// Real builds use the cairo-rs crate (feature "cairo-real").

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Format {
    ARGB32,
}

pub struct ImageSurface(*mut u8);

impl ImageSurface {
    pub fn create(_format: Format, _width: i32, _height: i32) -> Result<Self, ()> {
        Ok(Self(std::ptr::null_mut()))
    }
    pub fn width(&self) -> i32 { 0 }
    pub fn height(&self) -> i32 { 0 }
    pub fn stride(&self) -> i32 { 0 }
    pub fn data(&self) -> Result<&[u8], ()> { Ok(&[]) }
}

pub struct Context;

impl Context {
    pub fn new(_surface: &ImageSurface) -> Result<Self, ()> { Ok(Self) }
    pub fn set_source_rgba(&self, _: f64, _: f64, _: f64, _: f64) {}
    pub fn set_line_width(&self, _: f64) {}
    pub fn set_line_cap(&self, _: LineCap) {}
    pub fn set_line_join(&self, _: LineJoin) {}
    pub fn set_operator(&self, _: Operator) {}
    pub fn move_to(&self, _: f64, _: f64) {}
    pub fn line_to(&self, _: f64, _: f64) {}
    pub fn arc(&self, _: f64, _: f64, _: f64, _: f64, _: f64) {}
    pub fn rectangle(&self, _: f64, _: f64, _: f64, _: f64) {}
    pub fn stroke(&self) -> Result<(), ()> { Ok(()) }
    pub fn fill(&self) -> Result<(), ()> { Ok(()) }
    pub fn paint(&self) -> Result<(), ()> { Ok(()) }
    pub fn select_font_face(&self, _: &str, _: FontSlant, _: FontWeight) {}
    pub fn set_font_size(&self, _: f64) {}
    pub fn text_extents(&self, _: &str) -> TextExtents { TextExtents }
    pub fn show_text(&self, _: &str) -> Result<(), ()> { Ok(()) }
    pub fn set_source_surface(&self, _: &ImageSurface, _: f64, _: f64) -> Result<(), ()> { Ok(()) }
}

#[derive(Clone, Copy)]
pub enum LineCap { Butt, Round, Square }

#[derive(Clone, Copy)]
pub enum LineJoin { Miter, Round, Bevel }

#[derive(Clone, Copy)]
pub enum Operator { Clear, Over }

#[derive(Clone, Copy)]
pub enum FontSlant { Normal, Italic, Oblique }

#[derive(Clone, Copy)]
pub enum FontWeight { Normal, Bold }

pub struct TextExtents;
impl TextExtents {
    pub fn x_bearing(&self) -> f64 { 0.0 }
    pub fn y_bearing(&self) -> f64 { 0.0 }
    pub fn width(&self) -> f64 { 0.0 }
    pub fn height(&self) -> f64 { 0.0 }
    pub fn x_advance(&self) -> f64 { 0.0 }
    pub fn y_advance(&self) -> f64 { 0.0 }
}
