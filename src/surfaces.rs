pub const MAP_SCALE: f32 = 2.;

pub mod colors {
    pub const SKY: (f32, f32, f32) = (1., 0., 1.);
    pub const WATER: (f32, f32, f32) = (0., 0., 1.);
    pub const LAVA: (f32, f32, f32) = (1., 0., 0.);
    pub const SLIME: (f32, f32, f32) = (0., 1., 0.);
}

pub enum NiBroomSurface {
    NoClip = 1,
    Phong = 2,
    Invert = 4,
}
