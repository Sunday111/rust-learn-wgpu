#[cfg(test)]
pub(crate) mod test_utils {
    use cgmath::Vector3;
    use num_traits::Float;

    pub fn almost_equal<T: Float>(a: T, b: T, epsilon: T) -> bool {
        (a - b).abs() < epsilon
    }

    pub fn almost_equal_vec<T: Float>(a: Vector3<T>, b: Vector3<T>, epsilon: T) -> bool {
        almost_equal(a.x, b.x, epsilon)
            && almost_equal(a.y, b.y, epsilon)
            && almost_equal(a.z, b.z, epsilon)
    }
}
