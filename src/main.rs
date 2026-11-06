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
const SYSTEM_ZOOM: f32 = 30.0;
const SYSTEM_PITCH: f32 = -0.6;

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

/// Dónde y de qué tamaño se dibuja un cuerpo.
struct Placement {
    /// Centro del cuerpo en el espacio de la escena (antes de la rotación de vista).
    center: Vec3,
    /// Multiplicador de tamaño de todas sus piezas (y de la distancia de la luna).
    size: f32,
    /// Píxeles por unidad de escena a 800 px de ancho.
    zoom: f32,
    /// Si es Some, la luz viene de ese punto (la estrella); si no, se usa la luz fija.
    star: Option<Vec3>,
}

fn render(fb: &mut Framebuffer, meshes: &Meshes, body: &Body, pl: &Placement, t: f32, yaw: f32, pitch: f32) {
    let k = fb.width as f32 / WIDTH as f32;
    let zoom = pl.zoom * k;
    let (cx, cy) = (fb.width as f32 / 2.0, fb.height as f32 / 2.0);
    let center_view = view_rot(pl.center, yaw, pitch);
    let light = match pl.star {
        Some(star) => (view_rot(star, yaw, pitch) - center_view).normalized(),
        None => shaders::default_light(),
    };

    for part in &body.parts {
        let mesh = meshes.get(part.mesh);
        let spin = part.spin * t;
        let scale = part.scale * pl.size;
        let offset = match part.orbit {
            Some((dist, speed, incl)) => Vec3::new((speed * t + 1.94).cos(), 0.0, (speed * t + 1.94).sin())
                .rot_x(incl)
                * (dist * pl.size),
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
                let w = to_view(p * scale) + off_view + center_view;
                Vtx {
                    sx: cx + w.x * zoom,
                    sy: cy - w.y * zoom,
                    z: w.z + if part.billboard { 0.5 * pl.size } else { 0.0 },
                    p,
                    n: to_view(n),
                    // Relativa al centro del planeta y en radios planetarios, como esperan los shaders.
                    w: (w - center_view) * (1.0 / pl.size),
                }
            })
            .collect();

        let shader = part.shader;
        let shade_fn = |p: Vec3, n: Vec3, w: Vec3| shaders::shade(shader, &Frag { p, n, w, t, axis, light });
        for tri in &mesh.tris {
            raster::triangle(fb, &verts[tri[0]], &verts[tri[1]], &verts[tri[2]], mesh.spherical, &shade_fn);
        }
    }
}

/// Órbita de cada cuerpo en el sistema: (índice en `bodies`, tamaño, distancia, velocidad angular, fase).
const SYSTEM: [(usize, f32, f32, f32, f32); 5] = [
    (0, 1.7, 0.0, 0.0, 0.0),   // estrella
    (3, 0.36, 3.4, 0.50, 0.7), // lava
    (1, 0.50, 5.2, 0.30, 2.4), // rocoso + luna
    (2, 1.00, 8.6, 0.15, 3.2), // gaseoso + anillos
    (4, 0.75, 11.8, 0.09, 5.6), // hielo + anillos
];

fn system_center(dist: f32, speed: f32, phase: f32, t: f32) -> Vec3 {
    let a = speed * t + phase;
    Vec3::new(a.cos() * dist, 0.0, a.sin() * dist)
}

/// Dibuja el sistema completo: estrella al centro y los demás cuerpos orbitándola (traslación).
fn render_system(
    fb: &mut Framebuffer,
    meshes: &Meshes,
    bodies: &[Body],
    t: f32,
    (yaw, pitch, zoom_mul): (f32, f32, f32),
    focus: usize,
) {
    let k = fb.width as f32 / WIDTH as f32;
    let zoom = SYSTEM_ZOOM * zoom_mul * k;
    let (cx, cy) = (fb.width as f32 / 2.0, fb.height as f32 / 2.0);

    // La cámara sigue al cuerpo enfocado: se traslada toda la escena para dejarlo en el centro.
    let (_, _, fd, fs, fp) = SYSTEM[focus];
    let shift = system_center(fd, fs, fp, t);

    // Líneas de órbita tenues, dibujadas antes que los cuerpos.
    for &(_, _, dist, _, _) in SYSTEM.iter().filter(|s| s.2 > 0.0) {
        let steps = (dist * zoom * 6.0) as usize;
        for i in 0..steps {
            let a = std::f32::consts::TAU * i as f32 / steps as f32;
            let v = view_rot(Vec3::new(a.cos() * dist, 0.0, a.sin() * dist) - shift, yaw, pitch);
            let (x, y) = (cx + v.x * zoom, cy - v.y * zoom);
            if x >= 0.0 && y >= 0.0 && (x as usize) < fb.width && (y as usize) < fb.height {
                fb.put(y as usize * fb.width + x as usize, f32::INFINITY, [0.45, 0.55, 0.8, 0.22]);
            }
        }
    }

    // De atrás hacia adelante para que los anillos translúcidos mezclen bien.
    let mut order: Vec<(f32, Placement, usize)> = SYSTEM
        .iter()
        .map(|&(i, size, dist, speed, phase)| {
            let center = system_center(dist, speed, phase, t) - shift;
            let depth = view_rot(center, yaw, pitch).z;
            (depth, Placement { center, size, zoom: SYSTEM_ZOOM * zoom_mul, star: Some(Vec3::ZERO - shift) }, i)
        })
        .collect();
    order.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    for (_, pl, i) in &order {
        render(fb, meshes, &bodies[*i], pl, t, yaw, pitch);
    }
}

/// Vistas: 0..bodies.len() = un cuerpo; la última = sistema completo.
fn draw(fb: &mut Framebuffer, meshes: &Meshes, bodies: &[Body], view: usize, t: f32, cam: (f32, f32, f32), focus: usize) {
    let (yaw, pitch, zoom_mul) = cam;
    fb.clear();
    if view == bodies.len() {
        render_system(fb, meshes, bodies, t, cam, focus);
    } else {
        let b = &bodies[view];
        let pl = Placement { center: Vec3::ZERO, size: 1.0, zoom: b.zoom * zoom_mul, star: None };
        render(fb, meshes, b, &pl, t, yaw, pitch);
    }
}

/// Zoom que deja al cuerpo enfocado con ~110 px de radio (el sol = vista completa).
fn focus_zoom(focus: usize) -> f32 {
    if focus == 0 {
        1.0
    } else {
        110.0 / (SYSTEM_ZOOM * SYSTEM[focus].1)
    }
}

fn focus_name(bodies: &[Body], focus: usize) -> &'static str {
    if focus == 0 {
        "Estrella"
    } else {
        bodies[SYSTEM[focus].0].name
    }
}

fn title(bodies: &[Body], view: usize, focus: usize) -> String {
    if view == bodies.len() {
        format!("Shaders - Sistema completo (foco: {})", focus_name(bodies, focus))
    } else {
        format!("Shaders - {}", view_name(bodies, view).0)
    }
}

fn view_name(bodies: &[Body], view: usize) -> (&'static str, &'static str) {
    bodies.get(view).map_or(("Sistema completo", "sistema"), |b| (b.name, b.file))
}

fn default_pitch(bodies: &[Body], view: usize) -> f32 {
    if view == bodies.len() {
        SYSTEM_PITCH
    } else {
        -0.35
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let meshes = Meshes { sphere: mesh::sphere(64, 128), ring: mesh::annulus(180), quad: mesh::quad() };
    let bodies = bodies();
    let views = bodies.len() + 1;

    // Modo sin ventana: genera un PNG por vista en docs/ (con supermuestreo).
    if args.iter().any(|a| a == "--save-all") {
        let mut fb = Framebuffer::new(WIDTH * SUPERSAMPLE, HEIGHT * SUPERSAMPLE);
        for v in 0..views {
            draw(&mut fb, &meshes, &bodies, v, 4.0, (0.0, default_pitch(&bodies, v), 1.0), 0);
            let path = format!("docs/{}.png", view_name(&bodies, v).1);
            fb.save_png(&path, SUPERSAMPLE).expect("no se pudo guardar la imagen");
            println!("guardado {path}");
        }
        return;
    }

    let mut window =
        Window::new("Shaders", WIDTH, HEIGHT, WindowOptions::default()).expect("no se pudo crear la ventana");
    window.set_target_fps(60);
    let mut fb = Framebuffer::new(WIDTH, HEIGHT);

    let mut current = bodies.len();
    let (mut yaw, mut pitch) = (0.0_f32, default_pitch(&bodies, current));
    let (mut t, mut paused, mut zoom, mut focus) = (0.0_f32, false, 1.0_f32, 0usize);
    let keys = [Key::Key1, Key::Key2, Key::Key3, Key::Key4, Key::Key5, Key::Key6];
    window.set_title(&title(&bodies, current, focus));

    while window.is_open() && !window.is_key_down(Key::Escape) {
        for (i, key) in keys.iter().enumerate() {
            if i < views && window.is_key_pressed(*key, KeyRepeat::No) {
                current = i;
                yaw = 0.0;
                zoom = if current == bodies.len() { focus_zoom(focus) } else { 1.0 };
                pitch = default_pitch(&bodies, current);
                window.set_title(&title(&bodies, current, focus));
            }
        }
        if window.is_key_pressed(Key::Space, KeyRepeat::No) {
            paused = !paused;
        }
        if window.is_key_pressed(Key::R, KeyRepeat::No) {
            yaw = 0.0;
            zoom = if current == bodies.len() { focus_zoom(focus) } else { 1.0 };
            pitch = default_pitch(&bodies, current);
        }
        // Tab: en el sistema, enfoca el siguiente cuerpo (sigue su órbita) y ajusta el zoom.
        if current == bodies.len() && window.is_key_pressed(Key::Tab, KeyRepeat::No) {
            focus = (focus + 1) % SYSTEM.len();
            zoom = focus_zoom(focus);
            window.set_title(&title(&bodies, current, focus));
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
        // Zoom: rueda del ratón o teclas +/-.
        if let Some((_, dy)) = window.get_scroll_wheel() {
            zoom *= 1.1_f32.powf(dy);
        }
        if window.is_key_down(Key::Equal) || window.is_key_down(Key::NumPadPlus) {
            zoom *= 1.03;
        }
        if window.is_key_down(Key::Minus) || window.is_key_down(Key::NumPadMinus) {
            zoom /= 1.03;
        }
        zoom = zoom.clamp(0.3, 14.0);
        if !paused {
            t += 1.0 / 30.0;
        }

        draw(&mut fb, &meshes, &bodies, current, t, (yaw, pitch, zoom), focus);

        if window.is_key_pressed(Key::S, KeyRepeat::No) {
            let path = format!("docs/{}.png", view_name(&bodies, current).1);
            match fb.save_png(&path, 1) {
                Ok(()) => println!("guardado {path}"),
                Err(e) => eprintln!("error al guardar: {e}"),
            }
        }
        window.update_with_buffer(&fb.buffer, WIDTH, HEIGHT).unwrap();
    }
}
