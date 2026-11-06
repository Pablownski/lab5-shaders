mod font;
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
/// GIFs: resolución final, cuadros, y tiempo de escena que avanza por cuadro.
const GIF_SIZE: (usize, usize) = (400, 300);
const GIF_FRAMES: usize = 90;
const GIF_DT: f32 = 0.3;

/// Video: resolución final, cuadros por vista y tiempo de escena por cuadro (30 fps).
const VIDEO_SIZE: (usize, usize) = (1280, 720);
const VIDEO_FRAMES: usize = 300;
const VIDEO_DT: f32 = 0.1;

const SYSTEM_ZOOM: f32 = 25.0;
const SYSTEM_PITCH: f32 = -0.6;

#[derive(Clone, Copy)]
enum MeshKind {
    Sphere,
    Ring,
    Quad,
    Satellite,
    Rock,
    Streak,
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
    /// (distancia, velocidad angular, inclinación de la órbita, fase inicial) respecto al planeta.
    orbit: Option<(f32, f32, f32, f32)>,
    /// Los billboards siempre miran a la cámara (no heredan inclinación ni rotación de vista).
    billboard: bool,
    /// Inclinación del eje del planeta, para la sombra de sus anillos.
    axis_tilt: f32,
    /// Trayectoria lineal (inicio, fin, periodo, fase inicial) en el espacio de la escena; se repite.
    /// Si inicio == fin la pieza queda fija en ese punto. Un billboard con trayectoria es una estela:
    /// se orienta según el movimiento y su cabeza (extremo +x) queda sobre la posición.
    path: Option<(Vec3, Vec3, f32, f32)>,
}

struct Body {
    name: &'static str,
    file: &'static str,
    /// Píxeles por radio planetario (a 800 px de ancho).
    zoom: f32,
    parts: Vec<Part>,
}

fn part(mesh: MeshKind, shader: Shader, scale: f32) -> Part {
    Part { mesh, shader, scale, tilt: 0.0, spin: 0.0, orbit: None, billboard: false, axis_tilt: 0.0, path: None }
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
            name: "Planeta rocoso con luna, satélites, meteorito y estrella fugaz",
            file: "rocoso",
            zoom: 125.0,
            parts: earth_parts(),
        },
        Body {
            name: "Gigante gaseoso con anillos y lunas",
            file: "gaseoso",
            zoom: 100.0,
            parts: vec![
                Part { spin: 0.3, tilt: 0.4, axis_tilt: 0.4, ..part(MeshKind::Sphere, Shader::Gas, 1.0) },
                Part { tilt: 0.4, axis_tilt: 0.4, ..part(MeshKind::Ring, Shader::GasRing, 2.5) },
                Part { spin: 0.2, orbit: Some((3.1, 0.5, 0.1, 0.4)), ..part(MeshKind::Sphere, Shader::Io, 0.17) },
                Part { spin: 0.1, orbit: Some((3.9, 0.3, -0.12, 3.3)), ..part(MeshKind::Sphere, Shader::Europa, 0.15) },
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
        Body {
            name: "Agujero negro",
            file: "agujero",
            zoom: 80.0,
            parts: vec![
                Part { billboard: true, ..part(MeshKind::Quad, Shader::BhHalo, 3.0) },
                part(MeshKind::Sphere, Shader::BlackHole, 1.0),
                Part { tilt: 0.22, axis_tilt: 0.22, ..part(MeshKind::Ring, Shader::BhDisk, 3.8) },
            ],
        },
        Body {
            name: "Vista conjunta: estrella, Tierra, luna, satélites, meteorito y estrella fugaz",
            file: "conjunta",
            zoom: 80.0,
            parts: {
                let sun = Vec3::new(-4.3, 2.4, 4.0);
                let fixed = Some((sun, sun, 1.0, 0.0));
                let mut parts = vec![
                    Part { billboard: true, path: fixed, ..part(MeshKind::Quad, Shader::Corona, 2.4 * 0.9) },
                    Part { spin: 0.08, path: fixed, ..part(MeshKind::Sphere, Shader::Star, 0.9) },
                ];
                parts.extend(earth_parts());
                parts
            },
        },
    ]
}

/// Tierra con su luna, dos satélites artificiales, un meteorito con estela de fuego y una estrella fugaz.
fn earth_parts() -> Vec<Part> {
    // El meteorito pasa rozando la Tierra por abajo; la estrella fugaz cruza por encima.
    let meteor = (Vec3::new(5.0, -3.0, -1.2), Vec3::new(-4.5, -0.6, -1.2), 24.0, 0.383);
    let shooting = (Vec3::new(-6.0, 3.2, -1.5), Vec3::new(6.0, 0.3, -1.5), 11.0, 0.086);
    vec![
        Part { spin: 0.25, tilt: 0.41, ..part(MeshKind::Sphere, Shader::Earth, 1.0) },
        Part { spin: 0.1, orbit: Some((2.5, 0.35, 0.18, 1.94)), ..part(MeshKind::Sphere, Shader::Moon, 0.27) },
        // Satélites artificiales: uno en órbita baja inclinada y otro casi polar y retrógrado.
        Part {
            spin: 0.9,
            orbit: Some((1.55, 1.1, 0.8, 0.5)),
            ..part(MeshKind::Satellite, Shader::Satellite, 0.13)
        },
        Part {
            spin: -0.7,
            orbit: Some((1.9, -0.8, 1.45, 3.0)),
            ..part(MeshKind::Satellite, Shader::Satellite, 0.11)
        },
        Part { billboard: true, path: Some(meteor), ..part(MeshKind::Streak, Shader::Fire, 1.1) },
        Part { spin: 1.6, path: Some(meteor), ..part(MeshKind::Rock, Shader::Meteorite, 0.13) },
        Part { billboard: true, path: Some(shooting), ..part(MeshKind::Streak, Shader::ShootingStar, 1.5) },
    ]
}

struct Meshes {
    sphere: Mesh,
    ring: Mesh,
    quad: Mesh,
    satellite: Mesh,
    rock: Mesh,
    streak: Mesh,
}

impl Meshes {
    fn get(&self, k: MeshKind) -> &Mesh {
        match k {
            MeshKind::Sphere => &self.sphere,
            MeshKind::Ring => &self.ring,
            MeshKind::Quad => &self.quad,
            MeshKind::Satellite => &self.satellite,
            MeshKind::Rock => &self.rock,
            MeshKind::Streak => &self.streak,
        }
    }
}

fn view_rot(v: Vec3, yaw: f32, pitch: f32) -> Vec3 {
    v.rot_y(yaw).rot_x(pitch)
}

/// Escala respecto a la resolución de referencia (800x600); usa el lado limitante para no recortar en 16:9.
fn pixel_scale(fb: &Framebuffer) -> f32 {
    (fb.width as f32 / WIDTH as f32).min(fb.height as f32 / HEIGHT as f32)
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
    let k = pixel_scale(fb);
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
        let orbit_offset = match part.orbit {
            Some((dist, speed, incl, phase)) => {
                Vec3::new((speed * t + phase).cos(), 0.0, (speed * t + phase).sin()).rot_x(incl) * dist
            }
            None => Vec3::ZERO,
        };
        let path_offset = match part.path {
            Some((a, b, period, phase0)) => a + (b - a) * (t / period + phase0).fract(),
            None => Vec3::ZERO,
        };
        let offset = (orbit_offset + path_offset) * pl.size;
        // Estela: billboard que se mueve; apunta en la dirección del movimiento proyectada en pantalla.
        let streak_dir = match part.path {
            Some((a, b, _, _)) if part.billboard && (b - a).length() > 1e-6 => {
                let d = view_rot(b - a, yaw, pitch);
                Some(Vec3::new(d.x, d.y, 0.0).normalized())
            }
            _ => None,
        };
        let tilt = streak_dir.map_or(part.tilt, |d| d.y.atan2(d.x));
        let to_view = |v: Vec3| -> Vec3 {
            if part.billboard {
                return v.rot_z(tilt);
            }
            view_rot(v.rot_y(spin).rot_z(part.tilt), yaw, pitch)
        };
        let axis = if part.billboard {
            Vec3::new(0.0, 1.0, 0.0)
        } else {
            view_rot(Vec3::new(0.0, 1.0, 0.0).rot_z(part.axis_tilt), yaw, pitch)
        };
        // La cabeza de la estela (+x) queda sobre la posición de la trayectoria.
        let head_shift = streak_dir.map_or(Vec3::ZERO, |d| d * -scale * shaders::STREAK_HEAD);
        let off_view = view_rot(offset, yaw, pitch) + head_shift;

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
const SYSTEM: [(usize, f32, f32, f32, f32); 6] = [
    (0, 1.7, 0.0, 0.0, 0.0),   // estrella
    (3, 0.36, 3.4, 0.50, 0.7), // lava
    (1, 0.50, 5.2, 0.30, 2.4), // rocoso + luna
    (2, 1.00, 8.6, 0.15, 3.2), // gaseoso + anillos
    (4, 0.75, 11.8, 0.09, 5.6), // hielo + anillos
    (5, 0.30, 14.6, 0.05, 1.2), // agujero negro
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
    let k = pixel_scale(fb);
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
        // El agujero negro tiene un disco enorme respecto a su horizonte: se encuadra más lejos.
        let radius_px = if SYSTEM[focus].0 == 5 { 45.0 } else { 110.0 };
        radius_px / (SYSTEM_ZOOM * SYSTEM[focus].1)
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

/// Ayuda de controles en la esquina superior izquierda (solo en la ventana; no sale en capturas ni videos).
fn draw_help(buf: &mut [u32], width: usize) {
    const LINES: [&str; 4] = [
        "TAB: CAMBIAR DE VISTA",
        "F: ENFOCAR CUERPO (EN EL SISTEMA)",
        "+/-: ZOOM   ESPACIO: PAUSA",
        "H: OCULTAR AYUDA",
    ];
    let scale = 2;
    let box_w = LINES.iter().map(|l| font::text_width(l, scale)).max().unwrap() + 20;
    font::dim_rect(buf, width, 0, 0, box_w, LINES.len() * 20 + 12);
    for (i, line) in LINES.iter().enumerate() {
        let color = if i == 0 { 0x00FF_E27A } else { 0x00D0_D6E8 };
        font::draw_text(buf, width, 10, 10 + i * 20, line, scale, color);
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
    let meshes = Meshes {
        sphere: mesh::sphere(64, 128),
        ring: mesh::annulus(180),
        quad: mesh::quad(),
        satellite: mesh::satellite(),
        rock: mesh::rock(24, 48),
        streak: mesh::streak(),
    };
    let bodies = bodies();
    let views = bodies.len() + 1;

    // Modo sin ventana: cargo run --release -- --video <vista> <carpeta> vuelca los cuadros PNG de una vista.
    if let Some(i) = args.iter().position(|a| a == "--video") {
        let v: usize = args[i + 1].parse().expect("vista inválida");
        let dir = &args[i + 2];
        std::fs::create_dir_all(dir).expect("no se pudo crear la carpeta");
        let ss = 2;
        let mut fb = Framebuffer::new(VIDEO_SIZE.0 * ss, VIDEO_SIZE.1 * ss);
        for n in 0..VIDEO_FRAMES {
            draw(&mut fb, &meshes, &bodies, v, n as f32 * VIDEO_DT, (0.0, default_pitch(&bodies, v), 1.0), 0);
            fb.to_image(ss).save(format!("{dir}/{n:04}.png")).expect("no se pudo guardar el cuadro");
        }
        return;
    }

    // Modo sin ventana: un GIF animado por vista (el sistema y la vista conjunta incluidos) en docs/.
    if args.iter().any(|a| a == "--gif") {
        use image::codecs::gif::{GifEncoder, Repeat};
        use image::{Delay, Frame};
        let ss = 2;
        let mut fb = Framebuffer::new(GIF_SIZE.0 * ss, GIF_SIZE.1 * ss);
        // La escena se dibuja a 800 px de ancho de referencia; el zoom se reescala con el tamaño del framebuffer.
        for v in 0..views {
            let path = format!("docs/{}.gif", view_name(&bodies, v).1);
            let mut enc = GifEncoder::new_with_speed(std::fs::File::create(&path).expect("no se pudo crear el gif"), 10);
            enc.set_repeat(Repeat::Infinite).unwrap();
            for i in 0..GIF_FRAMES {
                draw(&mut fb, &meshes, &bodies, v, i as f32 * GIF_DT, (0.0, default_pitch(&bodies, v), 1.0), 0);
                let rgba = image::DynamicImage::ImageRgb8(fb.to_image(ss)).into_rgba8();
                enc.encode_frame(Frame::from_parts(rgba, 0, 0, Delay::from_numer_denom_ms(67, 1))).unwrap();
            }
            println!("guardado {path}");
        }
        return;
    }

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
    let mut show_help = true;
    let keys = [Key::Key1, Key::Key2, Key::Key3, Key::Key4, Key::Key5, Key::Key6, Key::Key7, Key::Key8];
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
        // Tab: pasa a la siguiente vista (cíclico).
        if window.is_key_pressed(Key::Tab, KeyRepeat::No) {
            current = (current + 1) % views;
            yaw = 0.0;
            zoom = if current == bodies.len() { focus_zoom(focus) } else { 1.0 };
            pitch = default_pitch(&bodies, current);
            window.set_title(&title(&bodies, current, focus));
        }
        if window.is_key_pressed(Key::H, KeyRepeat::No) {
            show_help = !show_help;
        }
        // F: en el sistema, enfoca el siguiente cuerpo (sigue su órbita) y ajusta el zoom.
        if current == bodies.len() && window.is_key_pressed(Key::F, KeyRepeat::No) {
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
        if show_help {
            draw_help(&mut fb.buffer, WIDTH);
        }
        window.update_with_buffer(&fb.buffer, WIDTH, HEIGHT).unwrap();
    }
}
