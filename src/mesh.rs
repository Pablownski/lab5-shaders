use crate::math::Vec3;
use std::f32::consts::PI;

pub struct Mesh {
    pub pos: Vec<Vec3>,
    pub nrm: Vec<Vec3>,
    pub tris: Vec<[usize; 3]>,
    /// Las esferas renormalizan la posición interpolada para que el shader vea puntos exactos de la esfera.
    pub spherical: bool,
}

/// Esfera unitaria (UV sphere).
pub fn sphere(lat: usize, lon: usize) -> Mesh {
    let mut pos = Vec::new();
    for i in 0..=lat {
        let theta = PI * i as f32 / lat as f32;
        for j in 0..=lon {
            let phi = 2.0 * PI * j as f32 / lon as f32;
            pos.push(Vec3::new(theta.sin() * phi.cos(), theta.cos(), theta.sin() * phi.sin()));
        }
    }
    let mut tris = Vec::new();
    for i in 0..lat {
        for j in 0..lon {
            let a = i * (lon + 1) + j;
            let b = a + lon + 1;
            tris.push([a, b, a + 1]);
            tris.push([a + 1, b, b + 1]);
        }
    }
    Mesh { nrm: pos.clone(), pos, tris, spherical: true }
}

/// Anillo plano en XZ con radio interior 0.5 y exterior 1.0.
pub fn annulus(seg: usize) -> Mesh {
    let mut pos = Vec::new();
    for i in 0..=seg {
        let a = 2.0 * PI * i as f32 / seg as f32;
        pos.push(Vec3::new(0.5 * a.cos(), 0.0, 0.5 * a.sin()));
        pos.push(Vec3::new(a.cos(), 0.0, a.sin()));
    }
    let mut tris = Vec::new();
    for i in 0..seg {
        let k = 2 * i;
        tris.push([k, k + 1, k + 2]);
        tris.push([k + 2, k + 1, k + 3]);
    }
    Mesh { nrm: vec![Vec3::new(0.0, 1.0, 0.0); pos.len()], pos, tris, spherical: false }
}

/// Cuadrado en XY de lado 2 (se usa como billboard para la corona).
pub fn quad() -> Mesh {
    let pos = vec![
        Vec3::new(-1.0, -1.0, 0.0),
        Vec3::new(1.0, -1.0, 0.0),
        Vec3::new(1.0, 1.0, 0.0),
        Vec3::new(-1.0, 1.0, 0.0),
    ];
    Mesh {
        nrm: vec![Vec3::new(0.0, 0.0, -1.0); 4],
        pos,
        tris: vec![[0, 1, 2], [0, 2, 3]],
        spherical: false,
    }
}
