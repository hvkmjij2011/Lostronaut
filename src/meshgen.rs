//! Procedural meshes: everything in the game is built here from boxes, cylinders and spheres
//! with vertex colours. Models stand on the origin with +Y up and face -Z.
use crate::galaxy::Species;
use crate::items::{Item, SK};
use crate::util::Rng;
use crate::voxel::{MeshData, TerrainGen};
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use std::f32::consts::{PI, TAU};

#[derive(Default)]
pub struct MB {
    pos: Vec<[f32; 3]>,
    nor: Vec<[f32; 3]>,
    col: Vec<[f32; 4]>,
    uv: Vec<[f32; 2]>,
    idx: Vec<u32>,
    /// Emissive lookup coordinate given to the vertices added next (see [`glow_uv`]).
    glow: [f32; 2],
}

pub fn lin(c: [f32; 3]) -> [f32; 4] {
    [c[0].max(0.0).powf(2.2), c[1].max(0.0).powf(2.2), c[2].max(0.0).powf(2.2), 1.0]
}

// ---------------------------------------------------------------- per-vertex glow
//
// `StandardMaterial` has no per-vertex emission, so glowing parts encode their emissive colour
// in UV0, which points into a small generated lookup texture holding every colour of a
// 32-level RGB cube (`glow_lut`). Materials that use it (`Gfx::flora`, `Gfx::shiny`) set that
// texture as `emissive_texture`; vertices that do not glow point at the black texel.

pub const GLOW_W: u32 = 256;
pub const GLOW_H: u32 = 128;

/// UV of the lookup texel for a linear emissive colour (channels 0..1).
pub fn glow_uv(c: [f32; 3]) -> [f32; 2] {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 31.0).round() as u32;
    let (r, g, b) = (q(c[0]), q(c[1]), q(c[2]));
    let x = r + 32 * (b % 8);
    let y = g + 32 * (b / 8);
    [(x as f32 + 0.5) / GLOW_W as f32, (y as f32 + 0.5) / GLOW_H as f32]
}

/// RGBA8 (linear) pixels of the emissive lookup texture, `GLOW_W` x `GLOW_H`.
pub fn glow_lut() -> Vec<u8> {
    let mut px = Vec::with_capacity((GLOW_W * GLOW_H * 4) as usize);
    for y in 0..GLOW_H {
        for x in 0..GLOW_W {
            let (r, g, b) = (x % 32, y % 32, x / 32 + 8 * (y / 32));
            let s = |v: u32| (v as f32 / 31.0 * 255.0).round() as u8;
            px.extend_from_slice(&[s(r), s(g), s(b), 255]);
        }
    }
    px
}

pub fn mesh_with_uv(pos: Vec<[f32; 3]>, nor: Vec<[f32; 3]>, col: Vec<[f32; 4]>, uv: Vec<[f32; 2]>, idx: Vec<u32>) -> Mesh {
    let mut mesh = mesh_from(pos, nor, col, idx);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
    mesh
}

pub fn at(x: f32, y: f32, z: f32) -> Mat4 {
    Mat4::from_translation(Vec3::new(x, y, z))
}

pub fn atr(x: f32, y: f32, z: f32, rot: Quat) -> Mat4 {
    Mat4::from_rotation_translation(rot, Vec3::new(x, y, z))
}

pub fn mesh_from(pos: Vec<[f32; 3]>, nor: Vec<[f32; 3]>, col: Vec<[f32; 4]>, idx: Vec<u32>) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    mesh.insert_indices(Indices::U32(idx));
    mesh
}

pub fn terrain_mesh(m: MeshData) -> Mesh {
    mesh_from(m.pos, m.nor, m.col, m.idx)
}

impl MB {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn build(self) -> Mesh {
        mesh_with_uv(self.pos, self.nor, self.col, self.uv, self.idx)
    }

    pub fn tris(&self) -> usize {
        self.idx.len() / 3
    }

    /// Uniformly scales everything built so far about the origin.
    pub fn scale(&mut self, s: f32) {
        for p in &mut self.pos {
            *p = [p[0] * s, p[1] * s, p[2] * s];
        }
    }

    fn v(&mut self, m: &Mat4, p: Vec3, n: Vec3, c: [f32; 4]) -> u32 {
        self.pos.push(m.transform_point3(p).to_array());
        self.nor.push(m.transform_vector3(n).normalize_or_zero().to_array());
        self.col.push(c);
        self.uv.push(self.glow);
        (self.pos.len() - 1) as u32
    }

    /// Emission (sRGB colour times strength 0..1) for everything added until the next call;
    /// `None` switches it off.
    pub fn set_glow(&mut self, c: Option<([f32; 3], f32)>) {
        self.glow = match c {
            Some((c, k)) => {
                let l = lin(c);
                glow_uv([l[0] * k, l[1] * k, l[2] * k])
            }
            None => glow_uv([0.0; 3]),
        };
    }

    /// One flat-shaded triangle in model space (normal from the transformed corners, so
    /// non-uniform scales stay correct).
    pub fn tri(&mut self, m: &Mat4, a: Vec3, b: Vec3, c: Vec3, col: [f32; 3]) {
        let (a, b, c) = (m.transform_point3(a), m.transform_point3(b), m.transform_point3(c));
        let n = (b - a).cross(c - a).normalize_or_zero();
        let col = lin(col);
        let i = self.pos.len() as u32;
        for p in [a, b, c] {
            self.pos.push(p.to_array());
            self.nor.push(n.to_array());
            self.col.push(col);
            self.uv.push(self.glow);
        }
        self.idx.extend_from_slice(&[i, i + 1, i + 2]);
    }

    /// Flat quad a-b-c-d (counter-clockwise seen from the front).
    pub fn quad_flat(&mut self, m: &Mat4, a: Vec3, b: Vec3, c: Vec3, d: Vec3, col: [f32; 3]) {
        self.tri(m, a, b, c, col);
        self.tri(m, a, c, d, col);
    }

    /// Faceted lumpy ball (an icosphere with `detail` subdivisions, every corner pushed in or
    /// out by up to `lump`). `paint(facet direction, random 0..1)` colours each facet.
    pub fn blob(&mut self, m: Mat4, r: Vec3, detail: u32, lump: f32, rng: &mut Rng, paint: &mut dyn FnMut(Vec3, f32) -> [f32; 3]) {
        let (verts, faces) = icosphere(detail);
        let pts: Vec<Vec3> = verts.iter().map(|v| *v * r * (1.0 + lump * rng.range(-1.0, 1.0))).collect();
        for f in faces {
            let (a, b, c) = (pts[f[0] as usize], pts[f[1] as usize], pts[f[2] as usize]);
            let dir = (verts[f[0] as usize] + verts[f[1] as usize] + verts[f[2] as usize]).normalize();
            let col = paint(dir, rng.f32());
            self.tri(&m, a, b, c, col);
        }
    }

    /// Surface of revolution around +Y through `profile` points (radius, height), flat shaded.
    /// Odd corners of every ring are pulled in by `star` (ribs, jagged rims). A profile that
    /// ends at a non-zero radius gets a flat cap. `paint(segment, side)` colours each quad.
    pub fn lathe(&mut self, m: Mat4, profile: &[(f32, f32)], sides: u32, star: f32, paint: &mut dyn FnMut(usize, u32) -> [f32; 3]) {
        let ring = |k: usize, i: u32| {
            let (r, y) = profile[k];
            let a = i as f32 / sides as f32 * TAU;
            let r = if i % 2 == 1 { r * (1.0 - star) } else { r };
            Vec3::new(a.cos() * r, y, a.sin() * r)
        };
        for k in 0..profile.len() - 1 {
            for i in 0..sides {
                let col = paint(k, i);
                let (a, b, c, d) = (ring(k, i), ring(k + 1, i), ring(k + 1, i + 1), ring(k, i + 1));
                if profile[k + 1].0 < 1e-4 {
                    self.tri(&m, a, b, d, col);
                } else if profile[k].0 < 1e-4 {
                    self.tri(&m, a, b, c, col);
                } else {
                    self.quad_flat(&m, a, b, c, d, col);
                }
            }
        }
        let last = profile.len() - 1;
        if profile[last].0 > 1e-4 {
            let top = Vec3::new(0.0, profile[last].1, 0.0);
            let col = paint(last, 0);
            for i in 0..sides {
                self.tri(&m, top, ring(last, i + 1), ring(last, i), col);
            }
        }
    }

    /// Tube along a polyline with a radius per point (0 = pointed end), flat shaded.
    /// `paint(segment, side)` colours each quad. A blunt end gets a cap.
    pub fn tube(&mut self, m: Mat4, pts: &[Vec3], radii: &[f32], sides: u32, paint: &mut dyn FnMut(usize, u32) -> [f32; 3]) {
        let n = pts.len();
        if n < 2 {
            return;
        }
        let dirs: Vec<Vec3> = (0..n)
            .map(|k| {
                let a = pts[k.saturating_sub(1)];
                let b = pts[(k + 1).min(n - 1)];
                (b - a).normalize_or(Vec3::Y)
            })
            .collect();
        let mut frames = Vec::with_capacity(n);
        let mut u = crate::util::any_perp(dirs[0]);
        for d in &dirs {
            u = (u - *d * u.dot(*d)).normalize_or(crate::util::any_perp(*d));
            frames.push((u, u.cross(*d)));
        }
        let ring = |k: usize, i: u32| {
            let a = i as f32 / sides as f32 * TAU;
            let (u, v) = frames[k];
            pts[k] + (u * a.cos() + v * a.sin()) * radii[k]
        };
        for k in 0..n - 1 {
            for i in 0..sides {
                let col = paint(k, i);
                let (a, b, c, d) = (ring(k, i), ring(k + 1, i), ring(k + 1, i + 1), ring(k, i + 1));
                if radii[k + 1] < 1e-4 {
                    self.tri(&m, a, b, d, col);
                } else if radii[k] < 1e-4 {
                    self.tri(&m, a, b, c, col);
                } else {
                    self.quad_flat(&m, a, b, c, d, col);
                }
            }
        }
        if radii[n - 1] > 1e-4 {
            let col = paint(n - 1, 0);
            for i in 0..sides {
                self.tri(&m, pts[n - 1], ring(n - 1, i + 1), ring(n - 1, i), col);
            }
        }
    }

    /// Double-sided leaf / frond: a strip from `base` along `dir`, bending by `droop` (towards
    /// -Y) along its length, widest in the middle; `serrate` > 0 notches the edge.
    #[allow(clippy::too_many_arguments)]
    pub fn leaf(&mut self, m: &Mat4, base: Vec3, dir: Vec3, len: f32, width: f32, droop: f32, segs: u32, serrate: f32, paint: &mut dyn FnMut(u32) -> [f32; 3]) {
        let dir = dir.normalize_or(Vec3::Y);
        let side = dir.cross(Vec3::Y).normalize_or(Vec3::X);
        let mut pts = Vec::with_capacity(segs as usize + 1);
        let mut p = base;
        let mut d = dir;
        for k in 0..=segs {
            pts.push(p);
            let t = k as f32 / segs as f32;
            d = (d - Vec3::Y * droop * (0.6 + t) / segs as f32).normalize();
            p += d * len / segs as f32;
        }
        for k in 0..segs as usize {
            let t0 = k as f32 / segs as f32;
            let t1 = (k + 1) as f32 / segs as f32;
            let w = |t: f32, k: usize| (t * PI).sin().max(0.0) * width * if k % 2 == 1 { 1.0 - serrate } else { 1.0 };
            let (w0, w1) = (w(t0, k), w(t1, k + 1));
            let col = paint(k as u32);
            let (a, b) = (pts[k], pts[k + 1]);
            let (al, ar, bl, br) = (a - side * w0, a + side * w0, b - side * w1, b + side * w1);
            // the strip is slightly folded along its midrib so it catches light from both sides
            let fold = Vec3::Y * (w0 + w1) * 0.15;
            let (am, bm) = (a + fold, b + fold);
            for (p0, p1, p2, p3) in [(al, am, bm, bl), (am, ar, br, bm)] {
                self.quad_flat(m, p0, p1, p2, p3, col);
                self.quad_flat(m, p3, p2, p1, p0, scale3(col, 0.8));
            }
        }
    }

    fn quad(&mut self, m: &Mat4, p: [Vec3; 4], c: [f32; 4]) {
        let n = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero();
        let i: Vec<u32> = p.iter().map(|q| self.v(m, *q, n, c)).collect();
        self.idx.extend_from_slice(&[i[0], i[1], i[2], i[0], i[2], i[3]]);
    }

    /// Box centred on the transform origin.
    pub fn cuboid(&mut self, m: Mat4, half: Vec3, c: [f32; 3]) {
        let c = lin(c);
        let h = half;
        let p = |x: f32, y: f32, z: f32| Vec3::new(x * h.x, y * h.y, z * h.z);
        self.quad(&m, [p(-1., -1., 1.), p(1., -1., 1.), p(1., 1., 1.), p(-1., 1., 1.)], c);
        self.quad(&m, [p(1., -1., -1.), p(-1., -1., -1.), p(-1., 1., -1.), p(1., 1., -1.)], c);
        self.quad(&m, [p(1., -1., 1.), p(1., -1., -1.), p(1., 1., -1.), p(1., 1., 1.)], c);
        self.quad(&m, [p(-1., -1., -1.), p(-1., -1., 1.), p(-1., 1., 1.), p(-1., 1., -1.)], c);
        self.quad(&m, [p(-1., 1., 1.), p(1., 1., 1.), p(1., 1., -1.), p(-1., 1., -1.)], c);
        self.quad(&m, [p(-1., -1., -1.), p(1., -1., -1.), p(1., -1., 1.), p(-1., -1., 1.)], c);
    }

    /// Cylinder / cone along +Y from y=0 to y=h. Flat shaded when it has few sides.
    pub fn cyl(&mut self, m: Mat4, r0: f32, r1: f32, h: f32, seg: u32, c: [f32; 3]) {
        let c = lin(c);
        let ring = |i: u32, r: f32, y: f32| {
            let a = i as f32 / seg as f32 * TAU;
            Vec3::new(a.cos() * r, y, a.sin() * r)
        };
        if seg <= 6 {
            for i in 0..seg {
                self.quad(&m, [ring(i + 1, r0, 0.0), ring(i, r0, 0.0), ring(i, r1.max(1e-4), h), ring(i + 1, r1.max(1e-4), h)], c);
            }
        } else {
            let slope = (r0 - r1) / h.max(1e-4);
            let start = self.pos.len() as u32;
            for i in 0..=seg {
                let a = i as f32 / seg as f32 * TAU;
                let n = Vec3::new(a.cos(), slope, a.sin());
                self.v(&m, ring(i, r0, 0.0), n, c);
                self.v(&m, ring(i, r1, h), n, c);
            }
            for i in 0..seg {
                let b = start + i * 2;
                self.idx.extend_from_slice(&[b, b + 1, b + 3, b, b + 3, b + 2]);
            }
        }
        for (r, y, ny) in [(r0, 0.0, -1.0f32), (r1, h, 1.0)] {
            if r < 1e-3 {
                continue;
            }
            let n = Vec3::Y * ny;
            let centre = self.v(&m, Vec3::new(0.0, y, 0.0), n, c);
            let start = self.pos.len() as u32;
            for i in 0..=seg {
                self.v(&m, ring(i, r, y), n, c);
            }
            for i in 0..seg {
                if ny > 0.0 {
                    self.idx.extend_from_slice(&[centre, start + i + 1, start + i]);
                } else {
                    self.idx.extend_from_slice(&[centre, start + i, start + i + 1]);
                }
            }
        }
    }

    /// Ellipsoid centred on the transform origin.
    pub fn sphere(&mut self, m: Mat4, r: Vec3, seg: u32, c: [f32; 3]) {
        let c = lin(c);
        let rings = (seg / 2).max(3);
        let start = self.pos.len() as u32;
        for j in 0..=rings {
            let th = j as f32 / rings as f32 * PI;
            for i in 0..=seg {
                let ph = i as f32 / seg as f32 * TAU;
                let n = Vec3::new(th.sin() * ph.cos(), th.cos(), th.sin() * ph.sin());
                self.v(&m, n * r, (n / r).normalize_or_zero(), c);
            }
        }
        let w = seg + 1;
        for j in 0..rings {
            for i in 0..seg {
                let a = start + j * w + i;
                let b = a + w;
                self.idx.extend_from_slice(&[a, a + 1, b + 1, a, b + 1, b]);
            }
        }
    }

    pub fn ball(&mut self, x: f32, y: f32, z: f32, r: f32, c: [f32; 3]) {
        self.sphere(at(x, y, z), Vec3::splat(r), 8, c);
    }
}

fn shade(c: [f32; 3], s: f32) -> [f32; 3] {
    [(c[0] * s).min(1.0), (c[1] * s).min(1.0), (c[2] * s).min(1.0)]
}

pub fn scale3(c: [f32; 3], s: f32) -> [f32; 3] {
    shade(c, s)
}

/// Unit icosphere: corners and outward-wound faces, `detail` times subdivided (0..=2).
pub fn icosphere(detail: u32) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let t = (1.0 + 5.0f32.sqrt()) / 2.0;
    let mut v: Vec<Vec3> = [
        (-1.0, t, 0.0), (1.0, t, 0.0), (-1.0, -t, 0.0), (1.0, -t, 0.0),
        (0.0, -1.0, t), (0.0, 1.0, t), (0.0, -1.0, -t), (0.0, 1.0, -t),
        (t, 0.0, -1.0), (t, 0.0, 1.0), (-t, 0.0, -1.0), (-t, 0.0, 1.0),
    ]
    .iter()
    .map(|&(x, y, z)| Vec3::new(x, y, z).normalize())
    .collect();
    let mut f: Vec<[u32; 3]> = vec![
        [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11], [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
        [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9], [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
    ];
    for _ in 0..detail.min(2) {
        let mut mid = std::collections::HashMap::new();
        let mut half = |a: u32, b: u32, v: &mut Vec<Vec3>| -> u32 {
            *mid.entry((a.min(b), a.max(b))).or_insert_with(|| {
                v.push(((v[a as usize] + v[b as usize]) * 0.5).normalize());
                (v.len() - 1) as u32
            })
        };
        let mut nf = Vec::with_capacity(f.len() * 4);
        for [a, b, c] in f {
            let (ab, bc, ca) = (half(a, b, &mut v), half(b, c, &mut v), half(c, a, &mut v));
            nf.extend_from_slice(&[[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]);
        }
        f = nf;
    }
    // make every face wind outwards whatever the table above says
    for face in f.iter_mut() {
        let (a, b, c) = (v[face[0] as usize], v[face[1] as usize], v[face[2] as usize]);
        if (b - a).cross(c - a).dot(a + b + c) < 0.0 {
            face.swap(1, 2);
        }
    }
    (v, f)
}

/// A plant of one species (three variants per species). Built in `nature.rs`.
pub fn plant_mesh(sp: &Species, variant: u32) -> Mesh {
    crate::nature::plant(sp, variant, crate::world::mobile()).build()
}

/// Surface deposit of one ore type (three variants per ore). Built in `nature.rs`.
pub fn crystal_mesh(item: Item, variant: u32) -> Mesh {
    crate::nature::crystal(item, variant).build()
}

pub fn sample_mesh() -> Mesh {
    crate::nature::specimen().build()
}

pub fn fragment_mesh() -> Mesh {
    crate::nature::fragment().build()
}

const WHITE: [f32; 3] = [0.9, 0.9, 0.92];
const GREY: [f32; 3] = [0.45, 0.47, 0.5];
const DARK: [f32; 3] = [0.17, 0.18, 0.21];
const ORANGE: [f32; 3] = [0.95, 0.5, 0.12];
const GLASS: [f32; 3] = [0.3, 0.62, 0.92];
const YELLOW: [f32; 3] = [0.95, 0.8, 0.2];

fn rocket(b: &mut MB, r: f32, body_h: f32, base_y: f32, accent: [f32; 3]) {
    b.cyl(at(0.0, base_y, 0.0), r, r, body_h, 12, WHITE);
    b.cyl(at(0.0, base_y + body_h * 0.55, 0.0), r * 1.02, r * 1.02, body_h * 0.08, 12, accent);
    b.cyl(at(0.0, base_y + body_h, 0.0), r, 0.0, r * 1.8, 12, accent);
    b.cyl(at(0.0, base_y - r * 0.6, 0.0), r * 0.75, r * 0.5, r * 0.6, 10, DARK);
    b.sphere(at(0.0, base_y + body_h * 0.75, -r * 0.9), Vec3::new(r * 0.35, r * 0.35, r * 0.15), 8, GLASS);
    for i in 0..3 {
        let a = i as f32 / 3.0 * TAU + 0.5;
        let rot = Quat::from_rotation_y(a);
        let o = rot * Vec3::new(r * 1.25, 0.0, 0.0);
        b.cuboid(atr(o.x, base_y + r * 0.5, o.z, rot * Quat::from_rotation_z(0.25)), Vec3::new(r * 0.45, r * 0.9, 0.06), accent);
        let foot = rot * Vec3::new(r * 1.7, 0.0, 0.0);
        b.cyl(atr(foot.x, 0.0, foot.z, Quat::IDENTITY), 0.22, 0.12, (base_y - r * 0.3).max(0.2), 6, GREY);
    }
}

pub fn structure_mesh(kind: SK) -> Mesh {
    let mut b = MB::new();
    match kind {
        SK::Pod => {
            let tilt = Quat::from_rotation_z(0.12) * Quat::from_rotation_x(-0.06);
            b.cyl(atr(0.0, 0.25, 0.0, tilt), 1.7, 1.7, 1.7, 14, WHITE);
            b.cyl(atr(-0.2, 1.93, -0.1, tilt), 1.7, 0.8, 1.0, 14, [0.75, 0.75, 0.78]);
            b.cyl(atr(-0.33, 2.9, -0.16, tilt), 0.8, 0.5, 0.25, 10, DARK);
            b.cyl(atr(0.0, 0.25, 0.0, tilt), 1.74, 1.74, 0.25, 14, ORANGE);
            b.cuboid(atr(0.0, 1.0, -1.66, tilt), Vec3::new(0.45, 0.7, 0.08), DARK);
            b.sphere(atr(1.2, 1.4, -1.15, tilt), Vec3::new(0.3, 0.3, 0.12), 8, GLASS);
            b.sphere(atr(-1.2, 1.4, -1.15, tilt), Vec3::new(0.3, 0.3, 0.12), 8, GLASS);
            for i in 0..3 {
                let a = i as f32 / 3.0 * TAU;
                b.cyl(atr(a.cos() * 1.6, 0.0, a.sin() * 1.6, Quat::from_rotation_z(-a.cos() * 0.4) * Quat::from_rotation_x(a.sin() * 0.4)), 0.14, 0.1, 0.6, 6, GREY);
            }
            b.cyl(at(0.9, 2.2, 0.6), 0.03, 0.03, 1.8, 4, GREY);
            b.ball(0.9, 4.0, 0.6, 0.09, [1.0, 0.25, 0.2]);
            b.cuboid(atr(1.2, 0.6, 1.3, Quat::from_rotation_y(0.7) * Quat::from_rotation_z(0.5)), Vec3::new(0.7, 0.04, 0.5), [0.2, 0.2, 0.22]);
        }
        SK::Smelter => {
            b.cuboid(at(0.0, 0.3, 0.0), Vec3::new(1.0, 0.3, 1.0), DARK);
            b.cyl(at(0.0, 0.6, 0.0), 0.8, 0.65, 1.3, 10, GREY);
            b.cyl(at(0.3, 1.9, 0.3), 0.22, 0.18, 1.1, 8, DARK);
            b.cuboid(at(0.0, 1.0, -0.72), Vec3::new(0.35, 0.28, 0.1), [1.0, 0.55, 0.1]);
            b.cuboid(at(0.0, 1.0, -0.76), Vec3::new(0.2, 0.15, 0.1), [1.0, 0.9, 0.3]);
            b.cuboid(at(-0.9, 0.75, 0.0), Vec3::new(0.2, 0.2, 0.4), ORANGE);
        }
        SK::Fabricator => {
            b.cuboid(at(0.0, 0.45, 0.0), Vec3::new(1.1, 0.45, 0.8), WHITE);
            b.cuboid(at(0.0, 0.93, 0.0), Vec3::new(1.15, 0.04, 0.85), GREY);
            b.cyl(at(-0.9, 0.95, 0.5), 0.09, 0.09, 1.3, 6, DARK);
            b.cuboid(at(-0.3, 2.2, 0.5), Vec3::new(0.7, 0.07, 0.09), ORANGE);
            b.cyl(at(0.3, 1.75, 0.5), 0.1, 0.03, 0.4, 6, DARK);
            b.cuboid(atr(0.75, 1.3, 0.5, Quat::from_rotation_x(-0.3)), Vec3::new(0.3, 0.25, 0.04), GLASS);
            b.cuboid(at(0.2, 1.05, 0.0), Vec3::new(0.25, 0.08, 0.25), YELLOW);
        }
        SK::ChemLab => {
            b.cuboid(at(0.0, 0.3, 0.0), Vec3::new(1.2, 0.3, 0.9), WHITE);
            b.cyl(at(-0.55, 0.6, 0.0), 0.42, 0.42, 1.4, 10, [0.35, 0.8, 0.6]);
            b.cyl(at(-0.55, 2.0, 0.0), 0.45, 0.2, 0.25, 10, GREY);
            b.cyl(at(0.55, 0.6, 0.1), 0.34, 0.34, 1.0, 10, GLASS);
            b.sphere(at(0.55, 1.75, 0.1), Vec3::splat(0.36), 8, [0.9, 0.5, 0.25]);
            b.cyl(atr(-0.5, 1.5, 0.0, Quat::from_rotation_z(-PI / 2.0)), 0.06, 0.06, 1.0, 5, DARK);
            b.cuboid(at(0.0, 0.8, -0.8), Vec3::new(0.3, 0.2, 0.06), DARK);
        }
        SK::Research => {
            b.cyl(Mat4::IDENTITY, 1.2, 1.1, 0.5, 12, WHITE);
            b.sphere(at(0.0, 0.5, 0.0), Vec3::new(0.85, 0.8, 0.85), 10, GLASS);
            b.cyl(at(0.0, 0.45, 0.0), 1.15, 1.15, 0.1, 12, ORANGE);
            b.cyl(at(0.9, 0.5, 0.6), 0.05, 0.05, 1.4, 5, GREY);
            b.cyl(atr(0.9, 1.9, 0.6, Quat::from_rotation_z(0.6)), 0.05, 0.45, 0.25, 10, WHITE);
            b.ball(0.0, 0.9, 0.0, 0.25, [0.5, 0.9, 1.0]);
        }
        SK::Solar => {
            b.cyl(Mat4::IDENTITY, 0.3, 0.2, 0.2, 8, GREY);
            b.cyl(at(0.0, 0.2, 0.0), 0.08, 0.08, 1.2, 6, GREY);
            let rot = Quat::from_rotation_x(0.45);
            b.cuboid(atr(0.0, 1.5, 0.0, rot), Vec3::new(1.3, 0.04, 0.8), [0.12, 0.2, 0.55]);
            b.cuboid(atr(0.0, 1.47, 0.0, rot), Vec3::new(1.36, 0.03, 0.86), WHITE);
            for i in -1..=1 {
                b.cuboid(atr(i as f32 * 0.86, 1.52, 0.0, rot), Vec3::new(0.02, 0.04, 0.8), WHITE);
            }
        }
        SK::BioGen => {
            b.cuboid(at(0.0, 0.2, 0.0), Vec3::new(0.9, 0.2, 0.6), DARK);
            b.cyl(atr(-0.8, 0.85, 0.0, Quat::from_rotation_z(-PI / 2.0)), 0.5, 0.5, 1.6, 10, [0.4, 0.62, 0.3]);
            b.cyl(atr(-0.82, 0.85, 0.0, Quat::from_rotation_z(-PI / 2.0)), 0.53, 0.53, 0.15, 10, YELLOW);
            b.cyl(at(0.5, 1.3, 0.0), 0.12, 0.1, 0.9, 6, GREY);
            b.cuboid(at(0.0, 1.45, -0.3), Vec3::new(0.3, 0.12, 0.2), ORANGE);
        }
        SK::Battery => {
            b.cuboid(at(0.0, 0.55, 0.0), Vec3::new(0.55, 0.55, 0.4), DARK);
            b.cuboid(at(0.0, 0.55, 0.0), Vec3::new(0.57, 0.12, 0.42), YELLOW);
            b.cyl(at(-0.25, 1.1, 0.0), 0.1, 0.1, 0.18, 6, [0.9, 0.3, 0.2]);
            b.cyl(at(0.25, 1.1, 0.0), 0.1, 0.1, 0.18, 6, [0.3, 0.5, 0.9]);
            b.cuboid(at(0.0, 0.85, -0.41), Vec3::new(0.3, 0.08, 0.02), [0.3, 1.0, 0.5]);
        }
        SK::Storage => {
            b.cuboid(at(0.0, 0.5, 0.0), Vec3::new(0.8, 0.5, 0.6), WHITE);
            b.cuboid(at(0.0, 1.02, 0.0), Vec3::new(0.84, 0.06, 0.64), ORANGE);
            b.cuboid(at(0.0, 0.5, -0.61), Vec3::new(0.15, 0.1, 0.03), DARK);
            for x in [-0.82, 0.82] {
                b.cuboid(at(x, 0.5, 0.0), Vec3::new(0.03, 0.5, 0.1), GREY);
            }
        }
        SK::Beacon => {
            b.cyl(Mat4::IDENTITY, 0.38, 0.3, 0.18, 8, GREY);
            b.cyl(at(0.0, 0.18, 0.0), 0.07, 0.06, 2.1, 6, WHITE);
            b.ball(0.0, 2.45, 0.0, 0.26, [0.4, 0.95, 1.0]);
            b.cyl(at(0.0, 2.2, 0.0), 0.2, 0.2, 0.06, 8, ORANGE);
        }
        SK::Shuttle => {
            rocket(&mut b, 0.95, 3.2, 1.3, ORANGE);
        }
        SK::Rtg => {
            b.cyl(Mat4::IDENTITY, 0.6, 0.55, 0.25, 10, GREY);
            b.cyl(at(0.0, 0.25, 0.0), 0.4, 0.4, 1.2, 10, DARK);
            for i in 0..6 {
                let rot = Quat::from_rotation_y(i as f32 / 6.0 * TAU);
                let o = rot * Vec3::new(0.55, 0.0, 0.0);
                b.cuboid(atr(o.x, 0.85, o.z, rot), Vec3::new(0.18, 0.5, 0.03), GREY);
            }
            b.ball(0.0, 1.5, 0.0, 0.25, [0.45, 1.0, 0.35]);
        }
        SK::Tether => {
            b.cyl(Mat4::IDENTITY, 0.16, 0.12, 0.1, 6, GREY);
            b.cyl(at(0.0, 0.1, 0.0), 0.04, 0.035, 1.3, 5, WHITE);
            b.ball(0.0, 1.48, 0.0, 0.1, [0.4, 0.95, 1.0]);
        }
        SK::Pylon => {
            b.cyl(Mat4::IDENTITY, 0.25, 0.2, 0.15, 6, GREY);
            b.cyl(at(0.0, 0.15, 0.0), 0.06, 0.05, 2.2, 6, DARK);
            b.cuboid(at(0.0, 2.3, 0.0), Vec3::new(0.35, 0.04, 0.04), YELLOW);
        }
        SK::Shipyard => {
            b.cyl(Mat4::IDENTITY, 4.2, 4.4, 0.4, 20, GREY);
            b.cyl(at(0.0, 0.4, 0.0), 2.6, 2.6, 0.12, 20, DARK);
            b.cyl(at(0.0, 0.4, 0.0), 4.0, 4.0, 0.05, 20, YELLOW);
            b.cuboid(at(3.5, 6.4, 0.0), Vec3::new(0.3, 6.0, 0.3), ORANGE);
            b.cuboid(at(3.5, 6.4, 0.0), Vec3::new(0.34, 0.1, 0.34), DARK);
            for y in [4.0, 8.0, 11.5] {
                b.cuboid(at(2.6, y, 0.0), Vec3::new(0.9, 0.08, 0.12), GREY);
            }
            b.cuboid(at(-3.3, 0.9, 1.5), Vec3::new(0.5, 0.5, 0.5), WHITE);
            b.cuboid(atr(-3.3, 1.6, 1.5, Quat::from_rotation_x(-0.4)), Vec3::new(0.4, 0.3, 0.04), GLASS);
        }
    }
    b.build()
}

/// The interstellar ship as it grows on the shipyard.
pub fn starship_mesh(hull: u32, core: bool, nav: bool, cryo: bool) -> Option<Mesh> {
    if hull == 0 && !core {
        return None;
    }
    let mut b = MB::new();
    let r = 1.7;
    let mut y = 1.6;
    if core {
        b.cyl(at(0.0, 0.5, 0.0), 1.5, 0.9, 1.1, 12, DARK);
        for i in 0..3 {
            let a = i as f32 / 3.0 * TAU;
            b.cyl(at(a.cos() * 1.1, 0.45, a.sin() * 1.1), 0.5, 0.3, 0.9, 8, [0.3, 0.3, 0.34]);
        }
        b.cyl(at(0.0, 1.5, 0.0), r * 1.03, r * 1.03, 0.2, 14, [1.0, 0.55, 0.9]);
    }
    for i in 0..hull {
        b.cyl(at(0.0, y, 0.0), r, r, 2.3, 14, WHITE);
        b.cyl(at(0.0, y + 2.2, 0.0), r * 1.02, r * 1.02, 0.12, 14, if i % 2 == 0 { ORANGE } else { GREY });
        if cryo && i == 2 {
            for k in 0..6 {
                let a = k as f32 / 6.0 * TAU;
                b.sphere(at(a.cos() * r * 0.97, y + 1.1, a.sin() * r * 0.97), Vec3::splat(0.3), 6, [0.6, 0.95, 1.0]);
            }
        }
        y += 2.3;
    }
    if hull >= 1 {
        for i in 0..4 {
            let rot = Quat::from_rotation_y(i as f32 / 4.0 * TAU + 0.4);
            let o = rot * Vec3::new(r * 1.3, 0.0, 0.0);
            b.cuboid(atr(o.x, 2.4, o.z, rot * Quat::from_rotation_z(0.3)), Vec3::new(0.7, 1.3, 0.08), ORANGE);
        }
    }
    if nav && hull >= 4 {
        b.cyl(at(0.0, y, 0.0), r, 0.0, 3.2, 14, [0.3, 0.5, 1.0]);
        b.cyl(at(0.0, y + 3.0, 0.0), 0.04, 0.02, 1.4, 4, GREY);
    } else if hull > 0 {
        b.cyl(at(0.0, y, 0.0), r * 0.9, r * 0.9, 0.1, 14, DARK);
    }
    Some(b.build())
}

pub fn flame_mesh() -> Mesh {
    let mut b = MB::new();
    b.cyl(atr(0.0, 0.0, 0.0, Quat::from_rotation_x(PI)), 0.55, 0.0, 3.0, 8, [1.0, 0.6, 0.15]);
    b.cyl(atr(0.0, 0.0, 0.0, Quat::from_rotation_x(PI)), 0.3, 0.0, 1.8, 8, [1.0, 0.95, 0.6]);
    b.build()
}

pub struct AstroMeshes {
    pub torso: Mesh,
    pub head: Mesh,
    pub arm: Mesh,
    pub leg: Mesh,
    pub tool: Mesh,
}

pub fn astronaut() -> AstroMeshes {
    let suit = [0.93, 0.93, 0.95];
    let mut t = MB::new();
    t.sphere(at(0.0, 0.0, 0.0), Vec3::new(0.3, 0.36, 0.22), 10, suit);
    t.cuboid(at(0.0, 0.05, 0.27), Vec3::new(0.24, 0.3, 0.12), GREY);
    t.cuboid(at(0.0, 0.2, 0.3), Vec3::new(0.25, 0.04, 0.13), ORANGE);
    t.cuboid(at(0.0, 0.02, -0.2), Vec3::new(0.12, 0.08, 0.04), DARK);
    t.ball(0.07, 0.03, -0.24, 0.025, [0.3, 1.0, 0.4]);
    t.cyl(at(0.0, -0.36, 0.0), 0.24, 0.27, 0.1, 10, ORANGE);
    let mut h = MB::new();
    h.sphere(Mat4::IDENTITY, Vec3::splat(0.27), 12, suit);
    h.sphere(at(0.0, 0.0, -0.1), Vec3::new(0.21, 0.19, 0.2), 10, [0.95, 0.65, 0.15]);
    h.cyl(at(0.0, -0.3, 0.0), 0.2, 0.2, 0.08, 10, GREY);
    let mut a = MB::new();
    a.sphere(at(0.0, 0.0, 0.0), Vec3::splat(0.11), 8, ORANGE);
    a.cyl(at(0.0, -0.5, 0.0), 0.075, 0.09, 0.5, 7, suit);
    a.ball(0.0, -0.54, 0.0, 0.095, DARK);
    let mut l = MB::new();
    l.cyl(at(0.0, -0.55, 0.0), 0.09, 0.115, 0.55, 7, suit);
    l.cuboid(at(0.0, -0.6, -0.04), Vec3::new(0.11, 0.07, 0.17), DARK);
    let mut g = MB::new();
    g.cuboid(at(0.0, 0.0, -0.15), Vec3::new(0.06, 0.08, 0.2), GREY);
    g.cyl(atr(0.0, 0.0, -0.3, Quat::from_rotation_x(-PI / 2.0)), 0.07, 0.11, 0.25, 8, ORANGE);
    g.ball(0.0, 0.0, -0.55, 0.06, [0.4, 0.95, 1.0]);
    AstroMeshes { torso: t.build(), head: h.build(), arm: a.build(), leg: l.build(), tool: g.build() }
}

/// Unit sphere coloured like the planet's surface, for the sky and the menu.
pub fn planet_ball(gen: &TerrainGen, seg: u32) -> Mesh {
    let rings = seg / 2;
    let (mut pos, mut nor, mut col, mut idx) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for j in 0..=rings {
        let th = j as f32 / rings as f32 * PI;
        for i in 0..=seg {
            let ph = i as f32 / seg as f32 * TAU;
            let n = Vec3::new(th.sin() * ph.cos(), th.cos(), th.sin() * ph.sin());
            let h = gen.height(n);
            let c = gen.color(n * (h - 0.2), n);
            let k = 0.8 + 0.4 * ((h - gen.radius) / gen.amp.max(1.0)).clamp(-0.5, 1.0);
            pos.push((n * (1.0 + (h - gen.radius) / gen.radius)).to_array());
            nor.push(n.to_array());
            col.push(lin([c[0] * k, c[1] * k, c[2] * k]));
        }
    }
    let w = seg + 1;
    for j in 0..rings {
        for i in 0..seg {
            let a = j * w + i;
            let b = a + w;
            idx.extend_from_slice(&[a, a + 1, b + 1, a, b + 1, b]);
        }
    }
    mesh_from(pos, nor, col, idx)
}

/// Star dome on a unit sphere: many faint stars and a few bright ones in star colours, a
/// denser milky band along a tilted great circle with soft nebula glows in it. Vertex alpha
/// carries the softness (the material blends).
pub fn star_dome(seed: u64, count: u32) -> Mesh {
    let mut rng = Rng::new(seed);
    let (mut pos, mut nor, mut col, mut idx) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let band_n = Vec3::new(0.3, 0.85, 0.42).normalize();
    let tints: [[f32; 3]; 5] = [[1.0, 1.0, 1.0], [0.75, 0.85, 1.0], [1.0, 0.9, 0.7], [1.0, 0.75, 0.6], [0.85, 0.92, 1.0]];
    let quad = |d: Vec3, s: f32, c: [f32; 4], pos: &mut Vec<[f32; 3]>, nor: &mut Vec<[f32; 3]>, col: &mut Vec<[f32; 4]>, idx: &mut Vec<u32>| {
        let u = crate::util::any_perp(d);
        let v = d.cross(u);
        let base = pos.len() as u32;
        for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            pos.push((d + u * a * s + v * b * s).to_array());
            nor.push((-d).to_array());
            col.push(c);
        }
        idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3, base, base + 2, base + 1, base, base + 3, base + 2]);
    };
    for k in 0..count + count / 2 {
        let mut d = rng.dir();
        if k >= count {
            // milky band: pull towards the great circle
            d = (d - band_n * d.dot(band_n) * rng.range(0.75, 1.0)).normalize();
        }
        let mag = rng.f32().powi(3); // most stars faint, a few bright
        let s = 0.0006 + 0.0016 * mag;
        let br = 0.35 + 0.65 * mag.sqrt();
        let t = tints[rng.int(5) as usize];
        let lc = lin([t[0] * br, t[1] * br, t[2] * br]);
        quad(d, s, [lc[0], lc[1], lc[2], 1.0], &mut pos, &mut nor, &mut col, &mut idx);
    }
    // soft glows: fans with an opaque-ish centre fading out to the rim
    for _ in 0..14 {
        let mut d = rng.dir();
        d = (d - band_n * d.dot(band_n) * 0.9).normalize();
        let u = crate::util::any_perp(d);
        let v = d.cross(u);
        let r = rng.range(0.05, 0.13);
        let c = *rng.pick(&[[0.35, 0.3, 0.6], [0.25, 0.4, 0.6], [0.55, 0.3, 0.45], [0.4, 0.42, 0.5]]);
        let a = rng.range(0.05, 0.11);
        let base = pos.len() as u32;
        pos.push(d.to_array());
        nor.push((-d).to_array());
        col.push([c[0], c[1], c[2], a]);
        for i in 0..=10 {
            let t = i as f32 / 10.0 * TAU;
            let q = d + (u * t.cos() * rng.range(0.8, 1.2) + v * t.sin()) * r;
            pos.push(q.to_array());
            nor.push((-d).to_array());
            col.push([c[0], c[1], c[2], 0.0]);
        }
        for i in 0..10 {
            idx.extend_from_slice(&[base, base + 1 + i, base + 2 + i, base, base + 2 + i, base + 1 + i]);
        }
    }
    mesh_from(pos, nor, col, idx)
}

/// Sky dome: unit sphere seen from inside, rings packed near the horizon so the gradient
/// there stays smooth. Returns the mesh (colours filled in later) and its vertex directions.
pub fn sky_dome(seg: u32) -> (Mesh, Vec<Vec3>) {
    let elev: [f32; 15] = [-90.0, -40.0, -15.0, -6.0, -2.0, 0.0, 2.0, 5.0, 9.0, 15.0, 24.0, 36.0, 52.0, 70.0, 90.0];
    let (mut pos, mut nor, mut col, mut idx) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for e in elev {
        let e = e.to_radians();
        for i in 0..=seg {
            let a = i as f32 / seg as f32 * TAU;
            let d = Vec3::new(e.cos() * a.cos(), e.sin(), e.cos() * a.sin());
            pos.push(d.to_array());
            nor.push((-d).to_array());
            col.push([0.0, 0.0, 0.0, 1.0]);
        }
    }
    let w = seg + 1;
    for j in 0..elev.len() as u32 - 1 {
        for i in 0..seg {
            let a = j * w + i;
            let b = a + w;
            idx.extend_from_slice(&[a, b + 1, a + 1, a, b, b + 1]);
        }
    }
    let dirs = pos.iter().map(|p| Vec3::from_array(*p)).collect();
    let mut mesh = mesh_from(pos, nor, col, idx);
    // kept in the main world too: `update_sky` rewrites its colours
    mesh.asset_usage = RenderAssetUsages::all();
    (mesh, dirs)
}

/// Flat disc facing +Z: bright opaque centre fading to a transparent rim (sun halo).
pub fn halo_disc(seg: u32) -> Mesh {
    let (mut pos, mut nor, mut col, mut idx) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let rings: [(f32, f32); 5] = [(0.0, 1.0), (0.12, 0.55), (0.3, 0.2), (0.6, 0.05), (1.0, 0.0)];
    for (r, a) in rings {
        for i in 0..seg {
            let t = i as f32 / seg as f32 * TAU;
            pos.push([t.cos() * r, t.sin() * r, 0.0]);
            nor.push([0.0, 0.0, 1.0]);
            col.push([1.0, 1.0, 1.0, a]);
        }
    }
    for j in 0..rings.len() as u32 - 1 {
        for i in 0..seg {
            let (a, b) = (j * seg + i, j * seg + (i + 1) % seg);
            let (c, d) = (a + seg, b + seg);
            idx.extend_from_slice(&[a, c, d, a, d, b]);
        }
    }
    mesh_from(pos, nor, col, idx)
}

pub fn simple_sphere(seg: u32) -> Mesh {
    let mut b = MB::new();
    b.sphere(Mat4::IDENTITY, Vec3::ONE, seg, [1.0, 1.0, 1.0]);
    b.build()
}
