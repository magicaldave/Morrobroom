pub const BOOK_START_DEFAULT: &str =
    "<DIV ALIGN=\"LEFT\"><FONT COLOR=\"000000\" SIZE=\"3\" FACE=\"Magic Cards\"><BR>";

pub mod colors {}

pub enum NiBroomSurface {
    NoClip = 1,
    SmoothShading = 2,
}

pub enum NiBroomContent {
    InvertFaces = 1,
}
