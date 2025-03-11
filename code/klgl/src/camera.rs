use cgmath::{Matrix4, Point3, Transform, Vector3};
use std::cell::{Ref, RefCell};

use crate::rotator::Rotator;

struct CameraCache {
    forward: Vector3<f32>,
    up: Vector3<f32>,
    right: Vector3<f32>,
    rotator_matrix: Matrix4<f32>,
    view_matrix: Matrix4<f32>,
}

pub struct Camera {
    eye: cgmath::Point3<f32>,
    rotator: Rotator,

    aspect: f32,
    fovy: f32,
    znear: f32,
    zfar: f32,

    cache: RefCell<Option<CameraCache>>,
}

impl Camera {
    pub fn new(
        eye: Point3<f32>,
        rot: Rotator,
        aspect: f32,
        fov: f32,
        znear: f32,
        zfar: f32,
    ) -> Self {
        Self {
            eye,
            rotator: rot,
            aspect,
            fovy: fov,
            znear,
            zfar,
            cache: RefCell::new(None),
        }
    }

    fn build_view_projection_matrix(&self) -> Matrix4<f32> {
        let cache = self.get_cache();
        let proj = cgmath::perspective(cgmath::Deg(self.fovy), self.aspect, self.znear, self.zfar);
        // OPENGL_TO_WGPU_MATRIX * proj * cache.view_matrix
        proj * cache.view_matrix
    }

    fn compute_cache(&self) -> CameraCache {
        let r = self.rotator.to_matrix();
        let forward = r.transform_vector(Vector3::unit_x());
        let right = r.transform_vector(Vector3::unit_y());
        let up = r.transform_vector(Vector3::unit_z());
        let view = Matrix4::look_to_rh(self.eye, forward, up);

        // view.x = -view.x;

        CameraCache {
            forward,
            up,
            right,
            rotator_matrix: r,
            view_matrix: view,
        }
    }

    fn get_cache(&self) -> Ref<CameraCache> {
        if self.cache.borrow().is_none() {
            *self.cache.borrow_mut() = Some(self.compute_cache());
        }

        Ref::map(self.cache.borrow(), |opt| opt.as_ref().unwrap())
    }

    pub fn get_eye(&self) -> &Point3<f32> {
        &self.eye
    }

    pub fn set_eye(&mut self, eye: Point3<f32>) {
        if self.eye != eye {
            self.eye = eye;
            self.clear_cache();
        }
    }

    pub fn get_rotator(&self) -> &Rotator {
        &self.rotator
    }

    pub fn set_rotator(&mut self, rotator: Rotator) {
        self.rotator = rotator;
        self.clear_cache();
    }

    pub fn set_aspect(&mut self, aspect: f32) {
        if aspect != self.aspect {
            self.aspect = aspect;
            self.clear_cache();
        }
    }

    pub fn forward(&self) -> Vector3<f32> {
        self.get_cache().forward
    }

    pub fn right(&self) -> Vector3<f32> {
        self.get_cache().right
    }

    pub fn up(&self) -> Vector3<f32> {
        self.get_cache().up
    }

    pub fn clear_cache(&mut self) {
        self.cache = RefCell::new(None);
    }
}

// We need this for Rust to store our data correctly for the shaders
#[repr(C)]
// This is so we can store this in a buffer
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    // We can't use cgmath with bytemuck directly, so we'll have
    // to convert the Matrix4 into a 4x4 f32 array
    pub view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    pub fn new() -> Self {
        use cgmath::SquareMatrix;
        Self {
            view_proj: Matrix4::identity().into(),
        }
    }

    pub fn update_view_proj(&mut self, camera: &Camera) {
        self.view_proj = camera.build_view_projection_matrix().into();
    }
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;
    use cgmath::Deg;

    #[test]
    fn test_add() {
        let c = Camera::new(
            (0.0, 0.0, 0.0).into(),
            Rotator {
                yaw: Deg(0.0),
                pitch: Deg(0.0),
                roll: Deg(0.0),
            },
            1.0,
            // which way is "up"
            90.0,
            0.1,
            100.0,
        );
        let v = c.get_cache().view_matrix;
        let a: Point3<f32> = Point3::new(1.0, 0.0, 0.0);
        let b: Point3<f32> = Point3::new(0.0, 1.0, 0.0);
        let c: Point3<f32> = Point3::new(0.0, 0.0, 1.0);

        println!("with view matrix:");
        println!("  {:?} -> {:?}", a, v.transform_point(a));
        println!("  {:?} -> {:?}", b, v.transform_point(b));
        println!("  {:?} -> {:?}", c, v.transform_point(c));
    }
}
