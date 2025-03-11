use cgmath::Matrix4;
use cgmath::{Deg, Rad};

pub struct Rotator {
    pub yaw: Deg<f32>,
    pub pitch: Deg<f32>,
    pub roll: Deg<f32>,
}

fn sincos(angle: Rad<f32>) -> (f32, f32) {
    let a: f32 = angle.0;
    (a.sin(), a.cos())
}

impl Rotator {
    pub fn to_matrix(&self) -> Matrix4<f32> {
        let (sa, ca) = sincos(self.roll.into());
        let (sb, cb) = sincos(self.pitch.into());
        let (sg, cg) = sincos(self.yaw.into());

        Matrix4::new(
            cb * cg,
            cb * sg,
            -sb,
            0.0,
            sa * sb * cg - ca * sg,
            sa * sb * sg + ca * cg,
            sa * cb,
            0.0,
            ca * sb * cg + sa * sg,
            ca * sb * sg - sa * cg,
            ca * cb,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_utils::*;
    use cgmath::{Transform, Vector3};

    #[test]
    fn test_zero_rotator() {
        let m = Rotator {
            yaw: Deg(0.0),
            pitch: Deg(0.0),
            roll: Deg(0.0),
        }
        .to_matrix();

        assert!(almost_equal_vec(
            m.transform_vector(Vector3::unit_x()),
            Vector3::unit_x(),
            1e-6
        ));

        assert!(almost_equal_vec(
            m.transform_vector(Vector3::unit_y()),
            Vector3::unit_y(),
            1e-6
        ));

        assert!(almost_equal_vec(
            m.transform_vector(Vector3::unit_y()),
            Vector3::unit_y(),
            1e-6
        ));
    }

    #[test]
    fn test_90_yaw() {
        let m = Rotator {
            yaw: Deg(90.0),
            pitch: Deg(0.0),
            roll: Deg(0.0),
        }
        .to_matrix();

        assert!(almost_equal_vec(
            m.transform_vector(Vector3::unit_x()),
            Vector3::unit_y(),
            1e-6
        ));

        assert!(almost_equal_vec(
            m.transform_vector(Vector3::unit_y()),
            -Vector3::unit_x(),
            1e-6
        ));

        assert!(almost_equal_vec(
            m.transform_vector(Vector3::unit_z()),
            Vector3::unit_z(),
            1e-6
        ));
    }

    #[test]
    fn test_90_pitch() {
        let m = Rotator {
            yaw: Deg(0.0),
            pitch: Deg(90.0),
            roll: Deg(0.0),
        }
        .to_matrix();

        assert!(almost_equal_vec(
            m.transform_vector(Vector3::unit_x()),
            -Vector3::unit_z(),
            1e-6
        ));

        assert!(almost_equal_vec(
            m.transform_vector(Vector3::unit_y()),
            Vector3::unit_y(),
            1e-6
        ));

        assert!(almost_equal_vec(
            m.transform_vector(Vector3::unit_z()),
            Vector3::unit_x(),
            1e-6
        ));
    }

    #[test]
    fn test_90_roll() {
        let m = Rotator {
            yaw: Deg(0.0),
            pitch: Deg(0.0),
            roll: Deg(90.0),
        }
        .to_matrix();

        assert!(almost_equal_vec(
            m.transform_vector(Vector3::unit_x()),
            Vector3::unit_x(),
            1e-6
        ));

        assert!(almost_equal_vec(
            m.transform_vector(Vector3::unit_y()),
            Vector3::unit_z(),
            1e-6
        ));

        assert!(almost_equal_vec(
            m.transform_vector(Vector3::unit_z()),
            -Vector3::unit_y(),
            1e-6
        ));
    }
}
