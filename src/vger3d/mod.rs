pub mod vertices;

pub mod uniforms;

pub mod transform;

pub mod camera;

pub mod geometries;

pub struct Translate {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

pub struct Rotate {
    pub axis_x_angle: f64,
    pub axis_y_angle: f64,
}
