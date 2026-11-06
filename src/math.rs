use std::ops::{Add, AddAssign, Mul, Neg, Sub};

#[derive(Clone, Copy, Debug)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 0.0 };

    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Vec3 { x, y, z }
    }

    pub fn dot(self, o: Vec3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    pub fn normalized(self) -> Vec3 {
        let l = self.length();
        if l == 0.0 {
            self
        } else {
            self * (1.0 / l)
        }
    }

    pub fn rot_x(self, a: f32) -> Vec3 {
        let (s, c) = a.sin_cos();
        Vec3::new(self.x, self.y * c - self.z * s, self.y * s + self.z * c)
    }

    pub fn rot_y(self, a: f32) -> Vec3 {
        let (s, c) = a.sin_cos();
        Vec3::new(self.x * c + self.z * s, self.y, -self.x * s + self.z * c)
    }

    pub fn rot_z(self, a: f32) -> Vec3 {
        let (s, c) = a.sin_cos();
        Vec3::new(self.x * c - self.y * s, self.x * s + self.y * c, self.z)
    }

    pub fn mix(self, o: Vec3, t: f32) -> Vec3 {
        self * (1.0 - t) + o * t
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl AddAssign for Vec3 {
    fn add_assign(&mut self, o: Vec3) {
        *self = *self + o;
    }
}

impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Mul<f32> for Vec3 {
    type Output = Vec3;
    fn mul(self, k: f32) -> Vec3 {
        Vec3::new(self.x * k, self.y * k, self.z * k)
    }
}

/// Producto componente a componente.
impl Mul<Vec3> for Vec3 {
    type Output = Vec3;
    fn mul(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x * o.x, self.y * o.y, self.z * o.z)
    }
}

impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Hermite suave; funciona también con e0 > e1 (curva invertida).
pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
