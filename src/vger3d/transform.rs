use bytemuck::{Pod, Zeroable};
use euclid::default::Transform3D;
// use nalgebra::{Matrix4, Perspective3};

use super::camera::Camera;

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(transparent)]
pub struct Transform(pub [f32; 16]);

impl Transform {
    pub fn identity() -> Self {
        Self::from(&Transform3D::identity())
    }

    /// Compute transform used for vertices
    ///
    /// The returned transform is used for transforming vertices on the GPU.
    pub fn for_vertices(camera: &Camera, aspect_ratio: f64) -> Self {
        let field_of_view_in_y = camera.field_of_view_in_x() / aspect_ratio;

        let projection = perspective(
            aspect_ratio,
            field_of_view_in_y,
            camera.near_plane(),
            camera.far_plane(),
        );

        let transform = projection.then(&camera.camera_to_model());

        Self::from(&transform)
    }

    /// Compute transform used for normals
    ///
    /// This method is only relevant for the graphics code. The returned
    /// transform is used for transforming normals on the GPU.
    pub fn for_normals(camera: &Camera) -> Self {
        let array = camera.camera_to_model().inverse().unwrap().to_array();
        // .transpose();

        // Self::from(&transform)

        Self(array.map(|val| val as f32))
    }
}

impl From<&Transform3D<f64>> for Transform {
    fn from(matrix: &Transform3D<f64>) -> Self {
        let mut native = [0.0; 16];
        native.copy_from_slice(&matrix.to_array());

        Self(native.map(|val| val as f32))
    }
}

// pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
//     let f = 1.0 / f32::tan(fov_y * TORAD / 2.0);
//     let nf = 1.0 / (near - far);
//     return Mat4 {v: [
//         f / aspect,
//         0.0,
//         0.0,
//         0.0,
//         0.0,
//         f,
//         0.0,
//         0.0,
//         0.0,
//         0.0,
//         (far + near) * nf,
//         -1.0,
//         0.0,
//         0.0,
//         (2.0 * far * near) * nf,
//         0.0
//     ]}
// }

pub fn perspective(fov_y: f64, aspect: f64, near: f64, far: f64) -> Transform3D<f64> {
    let f = 1.0 / f64::tan(fov_y / 2.0);
    let nf = 1.0 / (near - far);
    Transform3D::new(
        f / aspect,
        0.0,
        0.0,
        0.0,
        0.0,
        f,
        0.0,
        0.0,
        0.0,
        0.0,
        (far + near) * nf,
        -1.0,
        0.0,
        0.0,
        (2.0 * far * near) * nf,
        0.0,
    )
}
