use crate::noise::hash2;

pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    pub buffer: Vec<u32>,
    pub zbuffer: Vec<f32>,
    background: Vec<u32>,
}

fn pack(c: [f32; 3]) -> u32 {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0) as u32;
    (q(c[0]) << 16) | (q(c[1]) << 8) | q(c[2])
}

impl Framebuffer {
    pub fn new(width: usize, height: usize) -> Self {
        // Fondo: degradado azul muy oscuro con estrellas de brillo variable.
        let mut background = vec![0; width * height];
        for y in 0..height {
            for x in 0..width {
                let g = y as f32 / height as f32;
                let mut c = [0.012 + 0.01 * g, 0.014 + 0.012 * g, 0.03 + 0.03 * g];
                let h = hash2(x as i32, y as i32, 7);
                if h > 0.9988 {
                    let b = 0.35 + 0.65 * hash2(x as i32, y as i32, 11);
                    let tint = hash2(x as i32, y as i32, 13);
                    c = [b * (0.8 + 0.2 * tint), b * 0.9, b * (1.0 - 0.2 * tint)];
                }
                background[y * width + x] = pack(c);
            }
        }
        Framebuffer {
            width,
            height,
            buffer: background.clone(),
            zbuffer: vec![f32::INFINITY; width * height],
            background,
        }
    }

    pub fn clear(&mut self) {
        self.buffer.copy_from_slice(&self.background);
        self.zbuffer.fill(f32::INFINITY);
    }

    /// Escribe un fragmento; con alfa < 1 mezcla con lo que ya hay y no escribe z.
    pub fn put(&mut self, i: usize, z: f32, c: [f32; 4]) {
        if c[3] >= 0.999 {
            self.zbuffer[i] = z;
            self.buffer[i] = pack([c[0], c[1], c[2]]);
        } else {
            let d = self.buffer[i];
            let dst = [
                (d >> 16 & 255) as f32 / 255.0,
                (d >> 8 & 255) as f32 / 255.0,
                (d & 255) as f32 / 255.0,
            ];
            let a = c[3];
            self.buffer[i] = pack([
                dst[0] * (1.0 - a) + c[0] * a,
                dst[1] * (1.0 - a) + c[1] * a,
                dst[2] * (1.0 - a) + c[2] * a,
            ]);
        }
    }

    /// Guarda un PNG promediando bloques de `ss` x `ss` píxeles (supermuestreo).
    pub fn save_png(&self, path: &str, ss: usize) -> image::ImageResult<()> {
        let (w, h) = (self.width / ss, self.height / ss);
        let mut img = image::RgbImage::new(w as u32, h as u32);
        for y in 0..h {
            for x in 0..w {
                let mut acc = [0u32; 3];
                for sy in 0..ss {
                    for sx in 0..ss {
                        let px = self.buffer[(y * ss + sy) * self.width + x * ss + sx];
                        acc[0] += px >> 16 & 255;
                        acc[1] += px >> 8 & 255;
                        acc[2] += px & 255;
                    }
                }
                let n = (ss * ss) as u32;
                img.put_pixel(
                    x as u32,
                    y as u32,
                    image::Rgb([(acc[0] / n) as u8, (acc[1] / n) as u8, (acc[2] / n) as u8]),
                );
            }
        }
        img.save(path)
    }
}
