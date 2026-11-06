use crate::math::{lerp, Vec3};

fn hash(x: i32, y: i32, z: i32, salt: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6b343)
        ^ (y as u32).wrapping_mul(0xd8163841)
        ^ (z as u32).wrapping_mul(0xcb1ab31f)
        ^ salt.wrapping_mul(0x9e3779b9);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a2d39);
    h ^= h >> 15;
    (h >> 8) as f32 / 16_777_215.0
}

/// Ruido de valor 3D en [0, 1].
pub fn noise(p: Vec3) -> f32 {
    let (fx, fy, fz) = (p.x.floor(), p.y.floor(), p.z.floor());
    let (ix, iy, iz) = (fx as i32, fy as i32, fz as i32);
    let fade = |t: f32| t * t * (3.0 - 2.0 * t);
    let (u, v, w) = (fade(p.x - fx), fade(p.y - fy), fade(p.z - fz));
    let h = |dx, dy, dz| hash(ix + dx, iy + dy, iz + dz, 0);
    let x00 = lerp(h(0, 0, 0), h(1, 0, 0), u);
    let x10 = lerp(h(0, 1, 0), h(1, 1, 0), u);
    let x01 = lerp(h(0, 0, 1), h(1, 0, 1), u);
    let x11 = lerp(h(0, 1, 1), h(1, 1, 1), u);
    lerp(lerp(x00, x10, v), lerp(x01, x11, v), w)
}

/// Suma de octavas de ruido, normalizada a [0, 1].
pub fn fbm(mut p: Vec3, octaves: u32) -> f32 {
    let (mut sum, mut amp, mut norm) = (0.0, 0.5, 0.0);
    for _ in 0..octaves {
        sum += amp * noise(p);
        norm += amp;
        amp *= 0.5;
        p = p.rot_y(0.5).rot_x(0.3) * 2.03 + Vec3::new(17.3, 5.1, 9.7);
    }
    sum / norm
}

/// Ruido celular: (distancia al punto más cercano, al segundo más cercano, id [0,1] de la celda).
pub fn worley(p: Vec3) -> (f32, f32, f32) {
    let (ix, iy, iz) = (p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
    let (mut f1, mut f2, mut id) = (f32::MAX, f32::MAX, 0.0);
    for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                let (cx, cy, cz) = (ix + dx, iy + dy, iz + dz);
                let feature = Vec3::new(
                    cx as f32 + hash(cx, cy, cz, 1),
                    cy as f32 + hash(cx, cy, cz, 2),
                    cz as f32 + hash(cx, cy, cz, 3),
                );
                let d = (feature - p).length();
                if d < f1 {
                    f2 = f1;
                    f1 = d;
                    id = hash(cx, cy, cz, 4);
                } else if d < f2 {
                    f2 = d;
                }
            }
        }
    }
    (f1, f2, id)
}

/// Hash 2D para el campo de estrellas del fondo.
pub fn hash2(x: i32, y: i32, salt: u32) -> f32 {
    hash(x, y, 0, salt)
}
