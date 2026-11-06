use crate::math::{lerp, smoothstep, Vec3};
use crate::noise::{fbm, noise, worley};
use std::f32::consts::PI;

#[derive(Clone, Copy)]
pub enum Shader {
    Star,
    Corona,
    Earth,
    Moon,
    Gas,
    GasRing,
    Lava,
    Ice,
    IceRing,
}

/// Datos interpolados que recibe cada shader de fragmento.
pub struct Frag {
    /// Posición local (esfera unitaria o coordenadas del anillo/quad).
    pub p: Vec3,
    /// Normal en espacio de vista.
    pub n: Vec3,
    /// Posición en espacio de vista; el planeta está en el origen con radio 1.
    pub w: Vec3,
    pub t: f32,
    /// Eje de rotación del planeta en espacio de vista (para la sombra de los anillos).
    pub axis: Vec3,
}

pub fn light() -> Vec3 {
    Vec3::new(-0.6, 0.45, -0.66).normalized()
}

pub fn shade(s: Shader, f: &Frag) -> Option<[f32; 4]> {
    match s {
        Shader::Star => star(f),
        Shader::Corona => corona(f),
        Shader::Earth => earth(f),
        Shader::Moon => moon(f),
        Shader::Gas => gas_giant(f),
        Shader::GasRing => gas_ring(f),
        Shader::Lava => lava(f),
        Shader::Ice => ice_giant(f),
        Shader::IceRing => ice_ring(f),
    }
}

// ---------- utilidades ----------

fn c3(r: f32, g: f32, b: f32) -> Vec3 {
    Vec3::new(r, g, b)
}

/// Rampa de color por tramos: stops ordenados por posición.
fn ramp(stops: &[(f32, Vec3)], t: f32) -> Vec3 {
    if t <= stops[0].0 {
        return stops[0].1;
    }
    for w in stops.windows(2) {
        if t <= w[1].0 {
            return w[0].1.mix(w[1].1, (t - w[0].0) / (w[1].0 - w[0].0));
        }
    }
    stops[stops.len() - 1].1
}

fn out(c: Vec3, a: f32) -> Option<[f32; 4]> {
    Some([c.x.clamp(0.0, 1.0), c.y.clamp(0.0, 1.0), c.z.clamp(0.0, 1.0), a])
}

/// Coseno entre la normal y la dirección hacia la cámara (1 al centro del disco, 0 en el borde).
fn facing(f: &Frag) -> f32 {
    (-f.n.z).clamp(0.0, 1.0)
}

fn diffuse(f: &Frag) -> f32 {
    f.n.dot(light()).max(0.0)
}

// ---------- estrella ----------

fn star(f: &Frag) -> Option<[f32; 4]> {
    let (p, t) = (f.p, f.t);

    // Capa 1: convección (fbm con domain warping animado) pintada con una rampa de fuego.
    let q = p * 4.5 + Vec3::new(t * 0.10, -t * 0.07, t * 0.05);
    let warp = Vec3::new(
        fbm(q, 3),
        fbm(q + Vec3::new(5.2, 1.3, 2.8), 3),
        fbm(q + Vec3::new(1.7, 9.2, 4.1), 3),
    );
    let conv = fbm(q + warp * 1.2, 5);
    let mut col = ramp(
        &[
            (0.0, c3(0.62, 0.08, 0.0)),
            (0.4, c3(1.0, 0.38, 0.02)),
            (0.65, c3(1.0, 0.70, 0.12)),
            (1.0, c3(1.0, 0.95, 0.55)),
        ],
        conv,
    );

    // Capa 2: gránulos (celdas de Worley) que aclaran el centro de cada celda.
    let (f1, _, _) = worley(p * 16.0 + Vec3::new(0.0, t * 0.25, 0.0));
    col = col * (0.82 + 0.35 * (1.0 - f1.min(1.0)));

    // Capa 3: manchas solares oscuras, solo en latitudes medias.
    let s = fbm(p * 2.4 + Vec3::new(11.0, 3.0, 7.0), 4);
    let lat_mask = smoothstep(0.75, 0.45, p.y.abs());
    let spot = smoothstep(0.58, 0.68, s) * lat_mask;
    let penumbra = smoothstep(0.52, 0.60, s) * lat_mask;
    col = col.mix(c3(0.45, 0.10, 0.0), penumbra * 0.6);
    col = col.mix(c3(0.12, 0.02, 0.0), spot);

    // Capa 4: venas brillantes de fulguraciones (ruido "ridged").
    let ridge = 1.0 - (2.0 * noise(p * 6.5 + Vec3::new(t * 0.25, 0.0, -t * 0.15)) - 1.0).abs();
    col += c3(1.0, 0.85, 0.5) * smoothstep(0.95, 1.0, ridge) * 0.3 * (1.0 - spot);

    // Capa 5: oscurecimiento del limbo + halo rojizo en el borde.
    let mu = facing(f);
    col = col * (0.40 + 0.60 * mu.powf(0.55));
    col += c3(1.0, 0.30, 0.04) * (1.0 - mu).powi(3) * 0.55;
    out(col * 1.1, 1.0)
}

fn corona(f: &Frag) -> Option<[f32; 4]> {
    // El quad de la corona mide 2.4 radios solares de semiancho.
    let r = (f.p.x * f.p.x + f.p.y * f.p.y).sqrt();
    let edge = 1.0 / 2.4;
    let d = ((r - edge) / (1.0 - edge)).max(0.0);
    if d >= 1.0 {
        return None;
    }
    let ang = f.p.y.atan2(f.p.x);
    let rays = noise(Vec3::new(ang.cos() * 3.0, ang.sin() * 3.0, f.t * 0.15));
    let glow = (-d * 5.0).exp() * (0.65 + 0.7 * rays);
    let col = c3(1.0, 0.85, 0.45).mix(c3(1.0, 0.35, 0.05), d.sqrt());
    out(col, (glow * 0.95 * smoothstep(1.0, 0.7, d)).clamp(0.0, 1.0))
}

// ---------- planeta rocoso (tipo Tierra) ----------

fn earth(f: &Frag) -> Option<[f32; 4]> {
    let (p, t) = (f.p, f.t);
    let lat = p.y.abs();
    let h = fbm(p * 2.1 + Vec3::new(3.0, 1.0, 5.0), 6);
    let sea = 0.54;
    let diff = diffuse(f);
    let mu = facing(f);

    // Capa 1: relieve -> océano (profundidad) o continente (rampa por altura).
    let land = h > sea;
    let mut surf;
    if !land {
        let depth = smoothstep(sea - 0.22, sea, h);
        surf = c3(0.01, 0.05, 0.22).mix(c3(0.10, 0.45, 0.62), depth * depth);
    } else {
        let e = ((h - sea) / (1.0 - sea)).clamp(0.0, 1.0);
        surf = ramp(
            &[
                (0.0, c3(0.76, 0.70, 0.48)),
                (0.07, c3(0.22, 0.46, 0.16)),
                (0.30, c3(0.10, 0.30, 0.09)),
                (0.55, c3(0.40, 0.34, 0.28)),
                (0.80, c3(0.93, 0.94, 0.96)),
            ],
            e,
        );
        // Capa 2: desiertos en latitudes cálidas.
        let dry = smoothstep(0.52, 0.72, fbm(p * Vec3::new(1.6, 3.0, 1.6) + Vec3::new(7.0, 0.0, 2.0), 4))
            * smoothstep(0.65, 0.2, lat);
        surf = surf.mix(c3(0.80, 0.58, 0.32), dry * 0.85);
    }

    // Capa 3: casquetes polares con borde irregular.
    let ice = smoothstep(0.80, 0.87, lat + (fbm(p * 5.0, 4) - 0.5) * 0.16);
    surf = surf.mix(c3(0.92, 0.96, 1.0), ice);
    let is_water = !land && ice < 0.5;

    // Iluminación difusa + brillo especular solo en el agua.
    let mut col = surf * (0.03 + diff);
    if is_water {
        let hv = (light() + c3(0.0, 0.0, -1.0)).normalized();
        col += c3(1.0, 0.95, 0.85) * f.n.dot(hv).max(0.0).powf(70.0) * 0.8;
    }

    // Capa 4: nubes que se desplazan, con sombra sobre la superficie.
    let cloud_p = p * Vec3::new(3.0, 4.5, 3.0) + Vec3::new(t * 0.03, 0.0, 0.0);
    let cl = smoothstep(0.50, 0.72, fbm(cloud_p, 5));
    let shadow = smoothstep(0.50, 0.72, fbm(cloud_p + Vec3::new(0.06, -0.04, 0.0), 5));
    col = col * (1.0 - 0.35 * shadow * (1.0 - cl));
    col = col.mix(c3(1.0, 1.0, 1.0) * (0.04 + diff * 1.05), cl * 0.92);

    // Capa 5: luces de ciudades en el lado nocturno de la tierra.
    let night = smoothstep(0.12, 0.0, diff);
    if land && ice < 0.3 {
        let city = smoothstep(0.66, 0.78, noise(p * 38.0))
            * smoothstep(0.50, 0.62, fbm(p * 7.0 + Vec3::new(2.0, 2.0, 2.0), 3));
        col += c3(1.0, 0.72, 0.30) * city * night * (1.0 - cl * 0.85);
    }

    // Capa 6: atmósfera (fresnel azul), más fuerte hacia el lado iluminado.
    col += c3(0.25, 0.52, 1.0) * (1.0 - mu).powi(3) * (0.12 + diff * 1.1);
    out(col, 1.0)
}

fn moon(f: &Frag) -> Option<[f32; 4]> {
    let p = f.p;
    // Capa 1: regolito gris con "mares" oscuros.
    let mare = smoothstep(0.46, 0.60, fbm(p * 1.7 + Vec3::new(5.0, 2.0, 9.0), 4));
    let mut g = lerp(0.62, 0.36, mare) + (fbm(p * 22.0, 3) - 0.5) * 0.14;

    // Capa 2: cráteres de dos escalas (celdas de Worley: cuenco oscuro + borde claro).
    for (scale, density) in [(5.0_f32, 0.55_f32), (13.0, 0.5)] {
        let (d, _, id) = worley(p * scale);
        if id > density {
            let r = 0.14 + 0.2 * (id - density) / (1.0 - density);
            if d < r {
                g -= 0.22 * (1.0 - d / r);
            }
            g += 0.16 * smoothstep(0.045, 0.0, (d - r).abs());
        }
    }
    let diff = diffuse(f);
    out(c3(g, g * 0.98, g * 0.95) * (0.02 + diff), 1.0)
}

// ---------- gigante gaseoso con anillos ----------

/// Densidad de los anillos en función de u in [0,1] (0 = borde interior).
fn ring_density(u: f32) -> f32 {
    let coarse = noise(Vec3::new(u * 40.0, 1.5, 0.0));
    let fine = noise(Vec3::new(u * 230.0, 7.5, 0.0));
    let mut d = 0.30 + 0.55 * coarse + 0.25 * (fine - 0.5);
    d *= 1.0 - 0.95 * smoothstep(0.50, 0.52, u) * smoothstep(0.58, 0.56, u); // división de Cassini
    d *= smoothstep(0.0, 0.04, u) * smoothstep(1.0, 0.96, u);
    d.clamp(0.0, 1.0)
}

fn gas_giant(f: &Frag) -> Option<[f32; 4]> {
    let (p, t) = (f.p, f.t);
    let y = p.y;
    let mu = facing(f);
    let diff = diffuse(f);

    // Capa 1: bandas de latitud perturbadas por turbulencia.
    let turb = fbm(Vec3::new(p.x * 2.5 + t * 0.02, y * 3.5, p.z * 2.5), 4);
    let lat = y + (turb - 0.5) * 0.14;
    let band = 0.5 + 0.5 * (lat * 21.0).sin();
    let v = 0.62 * band + 0.38 * (0.5 + 0.5 * (lat * 47.0 + 1.3).sin());

    // Capa 2: vetas finas estiradas en longitud (corrientes en chorro con rotación diferencial).
    let shift = (band - 0.5) * t * 0.012;
    let streak = fbm(Vec3::new((p.x + shift) * 5.0, y * 42.0, p.z * 5.0), 4);
    let mut col = ramp(
        &[
            (0.0, c3(0.40, 0.22, 0.16)),
            (0.3, c3(0.70, 0.43, 0.27)),
            (0.55, c3(0.90, 0.77, 0.58)),
            (0.8, c3(0.97, 0.91, 0.79)),
            (1.0, c3(0.78, 0.60, 0.44)),
        ],
        (v + (streak - 0.5) * 0.35).clamp(0.0, 1.0),
    );

    // Capa 3: la gran tormenta (óvalo con remolino y halo claro).
    let lon = p.z.atan2(p.x);
    let latr = y.clamp(-1.0, 1.0).asin();
    let mut dl = lon - 0.9;
    if dl > PI {
        dl -= 2.0 * PI;
    }
    if dl < -PI {
        dl += 2.0 * PI;
    }
    let ex = dl * latr.cos() / 0.42;
    let ey = (latr + 0.32) / 0.16;
    let d = (ex * ex + ey * ey).sqrt();
    if d < 1.5 {
        let a = (1.5 - d) * 2.2;
        let (sa, ca) = a.sin_cos();
        let (rx, ry) = (ex * ca - ey * sa, ex * sa + ey * ca);
        let sw = fbm(Vec3::new(rx * 2.2, ry * 2.2, 3.0 + t * 0.05), 4);
        let spot = c3(0.50, 0.12, 0.07).mix(c3(0.86, 0.40, 0.24), sw);
        col = col.mix(c3(0.96, 0.88, 0.76), smoothstep(1.5, 1.15, d) * 0.55);
        col = col.mix(spot, smoothstep(1.05, 0.8, d));
    }

    // Sombra de los anillos proyectada sobre el planeta.
    let la = light().dot(f.axis);
    let mut shadow = 1.0;
    if la.abs() > 1e-3 {
        let s = -f.w.dot(f.axis) / la;
        if s > 0.0 {
            let r = (f.w + light() * s).length();
            if (1.25..2.5).contains(&r) {
                shadow = 1.0 - 0.75 * ring_density((r - 1.25) / 1.25);
            }
        }
    }

    // Capa 4: iluminación suave + bruma atmosférica en el borde.
    let mut lit = col * (0.025 + diff.powf(0.85) * shadow);
    lit += c3(0.95, 0.75, 0.5) * (1.0 - mu).powi(3) * 0.28 * (diff + 0.15);
    out(lit, 1.0)
}

fn gas_ring(f: &Frag) -> Option<[f32; 4]> {
    let r = (f.p.x * f.p.x + f.p.z * f.p.z).sqrt();
    let u = (r - 0.5) * 2.0;
    let d = ring_density(u);
    if d < 0.02 {
        return None;
    }
    let tone = noise(Vec3::new(u * 25.0, 3.0, 1.0));
    let col = c3(0.52, 0.42, 0.33).mix(c3(0.88, 0.80, 0.66), tone);
    // Sombra del planeta sobre el anillo (rayo hacia la luz contra la esfera unitaria).
    let b = f.w.dot(light());
    let disc = b * b - (f.w.dot(f.w) - 1.0);
    let shadow = if disc > 0.0 && -b + disc.sqrt() > 0.0 { 0.10 } else { 1.0 };
    let bright = (0.45 + 0.55 * f.n.dot(light()).abs()) * shadow;
    out(col * bright, d * 0.92)
}

// ---------- planeta de lava ----------

fn lava(f: &Frag) -> Option<[f32; 4]> {
    let (p, t) = (f.p, f.t);
    let mu = facing(f);
    let diff = diffuse(f);

    // Capa 1: corteza de basalto con rugosidad.
    let rough = fbm(p * 11.0, 4);
    let crust = c3(0.10, 0.08, 0.08).mix(c3(0.26, 0.22, 0.20), rough);

    // Capa 2: grietas = distancia entre los dos puntos celulares más cercanos (con warp), pulsan en el tiempo.
    let w = Vec3::new(
        noise(p * 3.0),
        noise(p * 3.0 + Vec3::new(5.0, 1.0, 2.0)),
        noise(p * 3.0 + Vec3::new(9.0, 3.0, 7.0)),
    );
    let (f1, f2, id) = worley(p * 4.2 + w * 0.9);
    let pulse = 0.08 + 0.025 * (t * 1.6 + id * 6.28).sin();
    let crack = 1.0 - smoothstep(0.0, pulse, f2 - f1);

    // Capa 3: mares de magma en las zonas bajas.
    let sea = smoothstep(0.60, 0.68, fbm(p * 1.9 + Vec3::new(9.0, 4.0, 1.0), 4));
    let mask = crack.max(sea * 0.9);
    let magma = ramp(
        &[(0.0, c3(0.45, 0.03, 0.0)), (0.5, c3(1.0, 0.32, 0.0)), (1.0, c3(1.0, 0.88, 0.35))],
        (mask * (0.6 + 0.6 * fbm(p * 14.0 + Vec3::new(0.0, t * 0.4, 0.0), 3))).clamp(0.0, 1.0),
    );

    // La lava es emisiva (se ve también en la cara nocturna); la corteza sí recibe luz.
    let mut col = crust * (0.04 + diff) * (1.0 - mask);
    col += magma * mask * 1.15;

    // Capa 4: humo/ceniza que oscurece y se tiñe de rojo por la lava de abajo.
    let smoke = smoothstep(0.55, 0.80, fbm(p * Vec3::new(2.0, 4.0, 2.0) + Vec3::new(t * 0.03, 3.0, 0.0), 5));
    col = col.mix(c3(0.12, 0.09, 0.09) * (0.35 + diff) + c3(0.25, 0.04, 0.0) * mask, smoke * 0.6);

    // Capa 5: atmósfera incandescente.
    col += c3(1.0, 0.28, 0.04) * (1.0 - mu).powf(2.5) * 0.6;
    out(col, 1.0)
}

// ---------- gigante de hielo ----------

fn ice_giant(f: &Frag) -> Option<[f32; 4]> {
    let (p, t) = (f.p, f.t);
    let y = p.y;
    let mu = facing(f);
    let diff = diffuse(f);

    // Capa 1: bandas suaves de metano turquesa.
    let turb = fbm(Vec3::new(p.x * 2.0, y * 2.0, p.z * 2.0), 3);
    let b = 0.5 + 0.5 * (y * 9.0 + turb * 3.0).sin();
    let mut col = ramp(
        &[(0.0, c3(0.20, 0.52, 0.74)), (0.5, c3(0.38, 0.76, 0.86)), (1.0, c3(0.62, 0.90, 0.92))],
        b,
    );

    // Capa 2: gran mancha oscura con nube blanca acompañante.
    let lon = p.z.atan2(p.x);
    let lat = y.clamp(-1.0, 1.0).asin();
    let mut dl = lon + 0.6;
    if dl > PI {
        dl -= 2.0 * PI;
    }
    if dl < -PI {
        dl += 2.0 * PI;
    }
    let (ex, ey) = (dl * lat.cos() / 0.38, (lat - 0.25) / 0.2);
    let d = (ex * ex + ey * ey).sqrt();
    col = col.mix(c3(0.06, 0.18, 0.42), smoothstep(1.0, 0.55, d) * 0.9);
    let comp = ((dl + 0.12) * lat.cos() / 0.2).powi(2) + ((lat - 0.05) / 0.045).powi(2);
    col = col.mix(c3(1.0, 1.0, 1.0), smoothstep(1.0, 0.2, comp.sqrt()) * 0.8);

    // Capa 3: cirros brillantes de metano alargados en longitud.
    let cirrus = smoothstep(0.62, 0.78, fbm(Vec3::new(p.x * 3.0 + t * 0.02, y * 16.0, p.z * 3.0), 4));
    col = col.mix(c3(0.95, 1.0, 1.0), cirrus * 0.75);

    // Capa 4: casquete polar claro + iluminación + bruma cian.
    col = col.mix(c3(0.8, 0.95, 0.95), smoothstep(0.72, 1.0, y.abs()) * 0.35);
    let mut lit = col * (0.03 + diff.powf(0.9));
    lit += c3(0.45, 0.85, 0.95) * (1.0 - mu).powi(3) * 0.35 * (diff + 0.2);
    out(lit, 1.0)
}

fn ice_ring(f: &Frag) -> Option<[f32; 4]> {
    let r = (f.p.x * f.p.x + f.p.z * f.p.z).sqrt();
    let u = (r - 0.5) * 2.0;
    // Tres anillos finos y débiles.
    let band = |c: f32, w: f32| smoothstep(w, w * 0.4, (u - c).abs());
    let d = band(0.30, 0.05).max(band(0.56, 0.035)).max(band(0.88, 0.06) * 0.8);
    if d < 0.02 {
        return None;
    }
    let grain = 0.7 + 0.3 * noise(Vec3::new(u * 150.0, 2.0, 0.0));
    let bright = 0.35 + 0.65 * f.n.dot(light()).abs();
    out(c3(0.70, 0.86, 0.90) * bright, d * 0.5 * grain)
}
