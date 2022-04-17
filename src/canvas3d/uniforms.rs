use bytemuck::{Pod, Zeroable};

use super::transform::Transform;

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct ScreenSize {
    pub width: f32,
    pub height: f32,
}

impl Default for ScreenSize {
    fn default() -> Self {
        Self {
            width: 512.0,
            height: 512.0,
        }
    }
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct Rect {
    pub x_min: f32,
    pub y_min: f32,
    pub width: f32,
    pub height: f32,
}

impl Default for Rect {
    fn default() -> Self {
        Self {
            x_min: 0.0,
            y_min: 0.0,
            width: 512.0,
            height: 512.0,
        }
    }
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct Transforms {
    pub transform: Transform,
    pub transform_normals: Transform,
}

impl Default for Transforms {
    fn default() -> Self {
        Self {
            transform: Transform::identity(),
            transform_normals: Transform::identity(),
        }
    }
}
