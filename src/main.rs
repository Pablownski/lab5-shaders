mod framebuffer;
mod math;
mod mesh;
mod noise;
mod raster;
mod shaders;

use framebuffer::Framebuffer;
use math::Vec3;
use mesh::Mesh;
use minifb::{Key, KeyRepeat, Window, WindowOptions};
use raster::Vtx;
use shaders::{Frag, Shader};

const WIDTH: usize = 800;
const HEIGHT: usize = 600;
const SUPERSAMPLE: usize = 2;

#[derive(Clone, Copy)]
enum MeshKind {
    Sphere,
    Ring,
    Quad,
}

/// Una pieza renderizable: malla + shader + su transformación (escala, inclinación, giro, órbita).
struct Part {
    mesh: MeshKind,
    shader: Shader,
    scale: f32,
    /// Inclinación del eje (rad, alrededor de Z).
    tilt: f32,
    /// Velocidad de giro propio (rad/s).
    spin: f32,
    /// (distancia, velocidad angular, inclinación de la órbita) respecto al planeta.
    orbit: Option<(f32, f32, f32)>,
    /// Los billboards siempre miran a la cámara (no heredan inclinación ni rotación de vista).
    billboard: bool,
    /// Inclinación del eje del planeta, para la sombra de sus anillos.
    axis_tilt: f32,
}

struct Body {
    name: &'static str,
    file: &'static str,
    /// Píxeles por radio planetario (a 800 px de ancho).
    zoom: f32,
    parts: Vec<Part>,
}

fn part(mesh: MeshKind, shader: Shader, scale: f32) -> Part {
    Part { mesh, shader, scale, tilt: 0.0, spin: 0.0, orbit: None, billboard: false, axis_tilt: 0.0 }
}

fn bodies() -> Vec<Body> {
    vec![
        Body {
            name: "Estrella",
            file: "estrella",
            zoom: 150.0,
            parts: vec![
                Part { billboard: true, ..part(MeshKind::Quad, Shader::Corona, 2.4) },
                Part { spin: 0.08, tilt: 0.2, ..part(MeshKind::Sphere, Shader::Star, 1.0) },
            ],
        },
        Body {
            name: "Planeta rocoso con luna",
            file: "rocoso",
            zoom: 125.0,
            parts: vec![
                Part { spin: 0.25, tilt: 0.41, ..part(MeshKind::Sphere, Shader::Earth, 1.0) },
                Part {
                    spin: 0.1,
                    orbit: Some((2.5, 0.35, 0.18)),
                    ..part(MeshKind::Sphere, Shader::Moon, 0.27)
                },
            ],
        },
        Body {
            name: "Gigante gaseoso con anillos",
            file: "gaseoso",
            zoom: 125.0,
            parts: vec![
                Part { spin: 0.3, tilt: 0.4, axis_tilt: 0.4, ..part(MeshKind::Sphere, Shader::Gas, 1.0) },
                Part { tilt: 0.4, axis_tilt: 0.4, ..part(MeshKind::Ring, Shader::GasRing, 2.5) },
            ],
        },
        Body {
            name: "Planeta de lava",
            file: "lava",
            zoom: 170.0,
            parts: vec![Part { spin: 0.2, tilt: 0.25, ..part(MeshKind::Sphere, Shader::Lava, 1.0) }],
        },
        Body {
            name: "Gigante de hielo",
            file: "hielo",
            zoom: 150.0,
            parts: vec![
                Part { spin: 0.25, tilt: 1.35, axis_tilt: 1.35, ..part(MeshKind::Sphere, Shader::Ice, 1.0) },
                Part { tilt: 1.35, axis_tilt: 1.35, ..part(MeshKind::Ring, Shader::IceRing, 2.2) },
            ],
        },
    ]
}

struct Meshes {
    sphere: Mesh,
    ring: Mesh,
    quad: Mesh,
}

impl Meshes {
    fn get(&self, k: MeshKind) -> &Mesh {
        match k {
            MeshKind::Sphere => &self.sphere,
            MeshKind::Ring => &self.ring,
            MeshKind::Quad => &self.quad,
        }
    }
}

fn view_rot(v: Vec3, yaw: f32, pitch: f32) -> Vec3 {
    v.rot_y(yaw).rot_x(pitch)
}

fn render(fb: &mut Framebuffer, meshes: &Meshes, body: &Body, t: f32, yaw: f32, pitch: f32) {
    let k = fb.width as f32 / WIDTH as f32;
    let zoom = body.zoom * k;
    let (cx, cy) = (fb.width as f32 / 2.0, fb.height as f32 / 2.0);

    for part in &body.parts {
        let mesh = meshes.get(part.mesh);
        let spin = part.spin * t;
        let offset = match part.orbit {
            Some((dist, speed, incl)) => Vec3::new((speed * t + 1.94).cos(), 0.0, (speed * t + 1.94).sin())
                .rot_x(incl)
                * dist,
            None => Vec3::ZERO,
        };
        let to_view = |v: Vec3| -> Vec3 {
            if part.billboard {
                return v;
            }
            view_rot(v.rot_y(spin).rot_z(part.tilt), yaw, pitch)
        };
        let axis = if part.billboard {
            Vec3::new(0.0, 1.0, 0.0)
        } else {
            view_rot(Vec3::new(0.0, 1.0, 0.0).rot_z(part.axis_tilt), yaw, pitch)
        };
        let off_view = if part.billboard { Vec3::ZERO } else { view_rot(offset, yaw, pitch) };

        let verts: Vec<Vtx> = mesh
            .pos
            .iter()
            .zip(&mesh.nrm)
            .map(|(&p, &n)| {
                let w = to_view(p * part.scale) + off_view;
                Vtx {
                    sx: cx + w.x * zoom,
                    sy: cy - w.y * zoom,
                    z: w.z + if part.billboard { 0.5 } else { 0.0 },
                    p,
                    n: to_view(n),
                    w,
                }
            })
            .collect();

        let shader = part.shader;
        let shade_fn = |p: Vec3, n: Vec3, w: Vec3| shaders::shade(shader, &Frag { p, n, w, t, axis });
        for tri in &mesh.tris {
            raster::triangle(fb, &verts[tri[0]], &verts[tri[1]], &verts[tri[2]], mesh.spherical, &shade_fn);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let meshes = Meshes { sphere: mesh::sphere(64, 128), ring: mesh::annulus(180), quad: mesh::quad() };
    let bodies = bodies();
    let (def_yaw, def_pitch) = (0.0_f32, -0.35_f32);

    // Modo sin ventana: genera un PNG por cuerpo en docs/ (con supermuestreo).
    if args.iter().any(|a| a == "--save-all") {
        let mut fb = Framebuffer::new(WIDTH * SUPERSAMPLE, HEIGHT * SUPERSAMPLE);
        for b in &bodies {
            fb.clear();
            render(&mut fb, &meshes, b, 4.0, def_yaw, def_pitch);
            let path = format!("docs/{}.png", b.file);
            fb.save_png(&path, SUPERSAMPLE).expect("no se pudo guardar la imagen");
            println!("guardado {path}");
        }
        return;
    }

    let mut window =
        Window::new("Shaders", WIDTH, HEIGHT, WindowOptions::default()).expect("no se pudo crear la ventana");
    window.set_target_fps(60);
    let mut fb = Framebuffer::new(WIDTH, HEIGHT);

    let (mut yaw, mut pitch) = (def_yaw, def_pitch);
    let (mut current, mut t, mut paused) = (0usize, 0.0_f32, false);
    let keys = [Key::Key1, Key::Key2, Key::Key3, Key::Key4, Key::Key5];
    window.set_title(&format!("Shaders - {}", bodies[current].name));

    while window.is_open() && !window.is_key_down(Key::Escape) {
        for (i, key) in keys.iter().enumerate() {
            if i < bodies.len() && window.is_key_pressed(*key, KeyRepeat::No) {
                current = i;
                window.set_title(&format!("Shaders - {}", bodies[current].name));
            }
        }
        if window.is_key_pressed(Key::Space, KeyRepeat::No) {
            paused = !paused;
        }
        if window.is_key_pressed(Key::R, KeyRepeat::No) {
            yaw = def_yaw;
            pitch = def_pitch;
        }
        if window.is_key_down(Key::Left) {
            yaw -= 0.03;
        }
        if window.is_key_down(Key::Right) {
            yaw += 0.03;
        }
        if window.is_key_down(Key::Up) {
            pitch -= 0.03;
        }
        if window.is_key_down(Key::Down) {
            pitch += 0.03;
        }
        if !paused {
            t += 1.0 / 30.0;
        }

        fb.clear();
        render(&mut fb, &meshes, &bodies[current], t, yaw, pitch);

        if window.is_key_pressed(Key::S, KeyRepeat::No) {
            let path = format!("docs/{}.png", bodies[current].file);
            match fb.save_png(&path, 1) {
                Ok(()) => println!("guardado {path}"),
                Err(e) => eprintln!("error al guardar: {e}"),
            }
        }
        window.update_with_buffer(&fb.buffer, WIDTH, HEIGHT).unwrap();
    }
}
