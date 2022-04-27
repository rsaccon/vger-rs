use std::f64::consts::FRAC_PI_2;

use euclid::default::{Box3D, Point3D, Transform3D, Translation3D};
// use nalgebra::{Point, TAffine, Transform, Translation};

// use crate::window::Window;

/// The camera abstraction
///
/// Please note that the metaphor we're using (which influences how mouse input
/// is handled, for example) is not that of a camera freely flying through a
/// static scene. Instead, the camera is static, and the model is freely
/// translated and rotated.
#[derive(Debug)]
pub struct Camera {
    /// The distance to the near plane
    near_plane: f64,

    /// The distance to the far plane
    far_plane: f64,

    /// The rotational part of the transform
    ///
    /// This is not an `nalgebra::Rotation`, as rotations happen around a center
    /// point, which means they must include a translational component.
    // pub rotation: Transform<f64, TAffine, 3>,
    pub rotation: Transform3D<f64>, // TODO

    /// The locational part of the transform
    // pub translation: Translation<f64, 3>,
    pub translation: Translation3D<f64>,
}

impl Camera {
    const SCREEN_FILL_FRAC: f64 = 0.5;
    const DEFAULT_NEAR_PLANE: f64 = 0.0001;
    const DEFAULT_FAR_PLANE: f64 = 1000.0;

    const INITIAL_FIELD_OF_VIEW_IN_X: f64 = FRAC_PI_2; // 90 degrees

    /// Returns a new camera aligned for viewing a bounding box
    pub fn new(aabb: &Box3D<f64>) -> Self {
        let initial_distance = {
            // Let's make sure we choose a distance, so that the model fills
            // most of the screen.
            //
            // To do that, first compute the model's highest point, as well as
            // the furthest point from the origin, in x and y.
            let highest_point = aabb.max.z;
            let furthest_point = *[aabb.min.x.abs(), aabb.max.x, aabb.min.y.abs(), aabb.max.y]
                .iter()
                .reduce(|accum, item| if accum >= item { accum } else { item })
                .unwrap();

            // The actual furthest point is not far enough. We don't want the
            // model to fill the whole screen.
            let furthest_point = furthest_point / Self::SCREEN_FILL_FRAC;
            // Having computed those points, figuring out how far the camera
            // needs to be from the model is just a bit of trigonometry.
            let distance_from_model =
                furthest_point / (Self::INITIAL_FIELD_OF_VIEW_IN_X / 2.).atan();

            // And finally, the distance from the origin is trivial now.
            highest_point + distance_from_model
        };

        let initial_offset = {
            let mut offset = aabb.center();
            offset.z = 0.;
            -offset
        };

        Self {
            near_plane: Self::DEFAULT_NEAR_PLANE,
            far_plane: Self::DEFAULT_FAR_PLANE,
            rotation: Transform3D::identity(),
            translation: Translation3D::new(initial_offset.x, initial_offset.y, -initial_distance),
        }
    }

    pub fn near_plane(&self) -> f64 {
        self.near_plane
    }

    pub fn far_plane(&self) -> f64 {
        self.far_plane
    }

    pub fn field_of_view_in_x(&self) -> f64 {
        Self::INITIAL_FIELD_OF_VIEW_IN_X
    }

    pub fn position(&self) -> Point3D<f64> {
        self.camera_to_model()
            .inverse()
            .unwrap()
            .transform_point3d(Point3D::origin())
            .unwrap()
    }

    // pub fn position(&self) -> Point<f64, 3> {
    //     self.camera_to_model()
    //         .inverse_transform_point(&Point::origin())
    // }

    /// Transform the position of the cursor on the near plane to model space
    // pub fn cursor_to_model_space(
    //     &self,
    //     cursor: PhysicalPosition<f64>,
    //     window: &Window,
    // ) -> Point<f64, 3> {
    //     let width = window.width() as f64;
    //     let height = window.height() as f64;
    //     let aspect_ratio = width / height;

    //     // Cursor position in normalized coordinates (-1 to +1) with
    //     // aspect ratio taken into account.
    //     let x = cursor.x / width * 2. - 1.;
    //     let y = -(cursor.y / height * 2. - 1.) / aspect_ratio;

    //     // Cursor position in camera space.
    //     let f = (self.field_of_view_in_x() / 2.).tan() * self.near_plane();
    //     let cursor = Point::origin() + Vector::from([x * f, y * f, -self.near_plane()]);

    //     self.camera_to_model().inverse_transform_point(&cursor)
    // }

    /// Compute the point on the model, that the cursor currently points to
    // pub fn focus_point(
    //     &self,
    //     window: &Window,
    //     cursor: Option<PhysicalPosition<f64>>,
    //     mesh: &Mesh<fj_math::Point<3>>,
    // ) -> FocusPoint {
    //     let cursor = match cursor {
    //         Some(cursor) => cursor,
    //         None => return FocusPoint::none(),
    //     };

    //     // Transform camera and cursor positions to model space.
    //     let origin = self.position();
    //     let cursor = self.cursor_to_model_space(cursor, window);
    //     let dir = (cursor - origin).normalize();

    //     let ray = Ray { origin, dir };

    //     let mut min_t = None;

    //     for triangle in mesh.triangles() {
    //         let t = Triangle::from_points(triangle.points)
    //             .to_parry()
    //             .cast_local_ray(&ray, f64::INFINITY, true);

    //         if let Some(t) = t {
    //             if t <= min_t.unwrap_or(t) {
    //                 min_t = Some(t);
    //             }
    //         }
    //     }

    //     FocusPoint(min_t.map(|t| ray.point_at(t)))
    // }

    /// Access the transform from camera to model space
    pub fn camera_to_model(&self) -> Transform3D<f64> {
        //
        // euclid: self.then(other) is equivalent to: self * other
        //

        Transform3D::identity()
            .then(&self.translation.to_transform())
            .then(&self.rotation)
    }

    // pub fn update_planes(&mut self, aabb: &Aabb<3>) {
    //     let view_transform = self.camera_to_model();
    //     let view_direction = Vector::from([0., 0., -1.]);

    //     let mut dist_min = f64::INFINITY;
    //     let mut dist_max = f64::NEG_INFINITY;

    //     for vertex in aabb.vertices() {
    //         let point = view_transform.transform_point(&vertex.to_na());

    //         // Project `point` onto `view_direction`. See this Wikipedia page:
    //         // https://en.wikipedia.org/wiki/Vector_projection
    //         //
    //         // Let's rename the variables first, so they fit the names in that
    //         // page.
    //         let (a, b) = (point.coords, view_direction);
    //         let a1 = a.dot(&b) / b.dot(&b) * b;

    //         let dist = a1.magnitude();

    //         if dist < dist_min {
    //             dist_min = dist;
    //         }
    //         if dist > dist_max {
    //             dist_max = dist;
    //         }
    //     }

    //     self.near_plane = if dist_min > 0. {
    //         // Setting `self.near_plane` to `dist_min` should theoretically
    //         // work, but results in the front of the model being clipped. I
    //         // wasn't able to figure out why, and for the time being, this
    //         // factor seems to work well enough.
    //         dist_min * 0.5
    //     } else {
    //         Self::DEFAULT_NEAR_PLANE
    //     };
    //     self.far_plane = if dist_max > 0. {
    //         dist_max
    //     } else {
    //         Self::DEFAULT_FAR_PLANE
    //     };
    // }
}

/// The point on the model that the cursor is currently pointing at
///
/// Such a point might or might not exist, depending on whether the cursor is
/// pointing at the model or not.
pub struct FocusPoint(pub Option<Point3D<f64>>);

impl FocusPoint {
    /// Construct the "none" instance of `FocusPoint`
    ///
    /// This instance represents the case that no focus point exists.
    pub fn none() -> Self {
        Self(None)
    }
}
