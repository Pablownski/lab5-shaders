use crate::framebuffer::Framebuffer;
use crate::math::Vec3;

/// Vértice ya proyectado: posición en pantalla + atributos a interpolar.
#[derive(Clone, Copy)]
pub struct Vtx {
    pub sx: f32,
    pub sy: f32,
    pub z: f32,
    /// Posición local (sin rotar): la usan los shaders para el ruido, así la textura gira con el cuerpo.
    pub p: Vec3,
    /// Normal en espacio de vista.
    pub n: Vec3,
    /// Posición en espacio de vista (unidades de radio planetario).
    pub w: Vec3,
}

pub type ShadeFn<'a> = &'a dyn Fn(Vec3, Vec3, Vec3) -> Option<[f32; 4]>;

/// Rasteriza un triángulo con coordenadas baricéntricas, z-buffer y shader por píxel.
pub fn triangle(fb: &mut Framebuffer, a: &Vtx, b: &Vtx, c: &Vtx, spherical: bool, shade: ShadeFn) {
    let area = (b.sx - a.sx) * (c.sy - a.sy) - (b.sy - a.sy) * (c.sx - a.sx);
    if area.abs() < 1e-6 {
        return;
    }
    let min_x = a.sx.min(b.sx).min(c.sx).floor().max(0.0) as usize;
    let max_x = (a.sx.max(b.sx).max(c.sx).ceil().max(0.0) as usize).min(fb.width - 1);
    let min_y = a.sy.min(b.sy).min(c.sy).floor().max(0.0) as usize;
    let max_y = (a.sy.max(b.sy).max(c.sy).ceil().max(0.0) as usize).min(fb.height - 1);

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let w0 = ((b.sx - px) * (c.sy - py) - (b.sy - py) * (c.sx - px)) / area;
            let w1 = ((c.sx - px) * (a.sy - py) - (c.sy - py) * (a.sx - px)) / area;
            let w2 = 1.0 - w0 - w1;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let z = w0 * a.z + w1 * b.z + w2 * c.z;
            let i = y * fb.width + x;
            if z >= fb.zbuffer[i] {
                continue;
            }
            let mut p = a.p * w0 + b.p * w1 + c.p * w2;
            if spherical {
                p = p.normalized();
            }
            let n = (a.n * w0 + b.n * w1 + c.n * w2).normalized();
            let w = a.w * w0 + b.w * w1 + c.w * w2;
            if let Some(color) = shade(p, n, w) {
                fb.put(i, z, color);
            }
        }
    }
}
