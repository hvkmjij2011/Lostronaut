//! World-generation props built from flat-shaded triangles: the wreckage of the Öncü (large
//! wreck pieces and small scrap) and the glowing growths of the caves. Each piece has its own
//! constructor so a later pass can swap any of them for a Blender model. Models stand on their
//! origin with +Y up (the origin sits at ground level; parts below it are buried).
use crate::meshgen::mesh_from;
use crate::util::Rng;
use bevy::prelude::*;
use std::f32::consts::{PI, TAU};

/// Weathered hull white, scorch, livery orange, structure grey, bare metal.
const HULL: [f32; 3] = [0.74, 0.74, 0.72];
const SCORCH: [f32; 3] = [0.2, 0.18, 0.17];
const STRIPE: [f32; 3] = [0.86, 0.42, 0.12];
const FRAME: [f32; 3] = [0.3, 0.31, 0.33];
const METAL: [f32; 3] = [0.55, 0.57, 0.6];
const INNER: [f32; 3] = [0.16, 0.16, 0.18];

/// Wreck piece kinds, in the order `wreck_meshes` returns them.
pub const W_PLATE_A: u8 = 0;
pub const W_PLATE_B: u8 = 1;
pub const W_TANK: u8 = 2;
pub const W_BELL: u8 = 3;
pub const W_RIB: u8 = 4;
pub const W_PIECES: u8 = 5;

/// Flat-shaded triangle soup with per-triangle colour.
#[derive(Default)]
struct Fb {
    pos: Vec<[f32; 3]>,
    nor: Vec<[f32; 3]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

fn lin(c: [f32; 3]) -> [f32; 4] {
    [c[0].max(0.0).powf(2.2), c[1].max(0.0).powf(2.2), c[2].max(0.0).powf(2.2), 1.0]
}

fn shade(c: [f32; 3], s: f32) -> [f32; 3] {
    [(c[0] * s).min(1.0), (c[1] * s).min(1.0), (c[2] * s).min(1.0)]
}

impl Fb {
    fn tri(&mut self, m: &Mat4, a: Vec3, b: Vec3, c: Vec3, col: [f32; 3]) {
        let (a, b, c) = (m.transform_point3(a), m.transform_point3(b), m.transform_point3(c));
        let n = (b - a).cross(c - a);
        if n.length_squared() < 1e-12 {
            return;
        }
        let n = n.normalize();
        let i0 = self.pos.len() as u32;
        let lc = lin(col);
        for p in [a, b, c] {
            self.pos.push(p.to_array());
            self.nor.push(n.to_array());
            self.col.push(lc);
        }
        self.idx.extend_from_slice(&[i0, i0 + 1, i0 + 2]);
    }

    fn quad(&mut self, m: &Mat4, p: [Vec3; 4], col: [f32; 3]) {
        self.tri(m, p[0], p[1], p[2], col);
        self.tri(m, p[0], p[2], p[3], col);
    }

    /// Both sides of a quad, the back side in its own colour.
    fn quad2(&mut self, m: &Mat4, p: [Vec3; 4], front: [f32; 3], back: [f32; 3]) {
        self.quad(m, p, front);
        self.quad(m, [p[3], p[2], p[1], p[0]], back);
    }

    fn cuboid(&mut self, m: &Mat4, half: Vec3, col: [f32; 3]) {
        let h = half;
        let p = |x: f32, y: f32, z: f32| Vec3::new(x * h.x, y * h.y, z * h.z);
        let faces = [
            [p(-1., -1., 1.), p(1., -1., 1.), p(1., 1., 1.), p(-1., 1., 1.)],
            [p(1., -1., -1.), p(-1., -1., -1.), p(-1., 1., -1.), p(1., 1., -1.)],
            [p(1., -1., 1.), p(1., -1., -1.), p(1., 1., -1.), p(1., 1., 1.)],
            [p(-1., -1., -1.), p(-1., -1., 1.), p(-1., 1., 1.), p(-1., 1., -1.)],
            [p(-1., 1., 1.), p(1., 1., 1.), p(1., 1., -1.), p(-1., 1., -1.)],
            [p(-1., -1., -1.), p(1., -1., -1.), p(1., -1., 1.), p(-1., -1., 1.)],
        ];
        for (k, f) in faces.iter().enumerate() {
            self.quad(m, *f, shade(col, [1.0, 0.9, 0.95, 0.85, 1.08, 0.7][k]));
        }
    }

    /// Open tube along +Y with the given radius profile, `sides` segments, seen from outside
    /// in `out` and from inside in `inn`.
    fn tube(&mut self, m: &Mat4, rings: &[(f32, f32)], sides: u32, out: [f32; 3], inn: [f32; 3]) {
        for w in rings.windows(2) {
            let ((y0, r0), (y1, r1)) = (w[0], w[1]);
            for i in 0..sides {
                let a0 = i as f32 / sides as f32 * TAU;
                let a1 = (i + 1) as f32 / sides as f32 * TAU;
                let p = |a: f32, r: f32, y: f32| Vec3::new(a.cos() * r, y, a.sin() * r);
                let q = [p(a0, r0, y0), p(a0, r1, y1), p(a1, r1, y1), p(a1, r0, y0)];
                let s = 0.9 + 0.1 * ((i * 7) % 3) as f32;
                self.quad2(m, q, shade(out, s), shade(inn, s));
            }
        }
    }

    /// Solid cylinder along +Y.
    fn cyl(&mut self, m: &Mat4, r: f32, h: f32, sides: u32, col: [f32; 3]) {
        let p = |i: u32, y: f32| {
            let a = i as f32 / sides as f32 * TAU;
            Vec3::new(a.cos() * r, y, a.sin() * r)
        };
        for i in 0..sides {
            self.quad(m, [p(i + 1, 0.0), p(i, 0.0), p(i, h), p(i + 1, h)], shade(col, 0.88 + 0.12 * (i % 2) as f32));
            self.tri(m, Vec3::new(0.0, h, 0.0), p(i + 1, h), p(i, h), shade(col, 1.05));
            self.tri(m, Vec3::ZERO, p(i, 0.0), p(i + 1, 0.0), shade(col, 0.7));
        }
    }

    fn build(self) -> Mesh {
        mesh_from(self.pos, self.nor, self.col, self.idx)
    }
}

fn mat(t: Vec3, r: Quat) -> Mat4 {
    Mat4::from_rotation_translation(r, t)
}

/// A curved hull panel lying on the ground like a shallow arch, one end torn into jagged
/// teeth, a scorched patch and the orange livery band; frames on the inside.
pub fn hull_plate(variant: u32) -> Mesh {
    let mut rng = Rng::new(0x0C0_u64 + variant as u64 * 97);
    let mut b = Fb::default();
    let (rad, span, len): (f32, f32, f32) = if variant == 0 { (5.0, 1.25, 6.5) } else { (4.0, 1.0, 4.5) };
    let (nu, nv) = (8usize, 6usize);
    let h0 = rad * (1.0 - (span * 0.5).cos());
    // jagged far end: each column ends at its own length
    let ends: Vec<f32> = (0..=nu).map(|_| rng.range(0.7, 1.0)).collect();
    let pt = |u: f32, v: f32, ends: &[f32]| {
        let th = (u - 0.5) * span;
        let col = ((u * nu as f32).round() as usize).min(nu);
        let v = v * (1.0 + (ends[col] - 1.0) * crate::util::smoothstep(0.5, 1.0, v));
        Vec3::new(rad * th.sin(), rad * th.cos() - rad + h0, (v - 0.5) * len)
    };
    let holes = [(rng.int(nu as u32) as usize, nv - 2), (rng.int(nu as u32) as usize, rng.int(nv as u32 - 2) as usize)];
    let scorch_c = rng.range(0.2, 0.8);
    for j in 0..nv {
        for i in 0..nu {
            if holes.contains(&(i, j)) {
                continue;
            }
            let (u0, u1) = (i as f32 / nu as f32, (i + 1) as f32 / nu as f32);
            let (v0, v1) = (j as f32 / nv as f32, (j + 1) as f32 / nv as f32);
            let q = [pt(u0, v0, &ends), pt(u1, v0, &ends), pt(u1, v1, &ends), pt(u0, v1, &ends)];
            let mut c = if (2..=3).contains(&j) && variant == 0 || j == 1 && variant == 1 { STRIPE } else { HULL };
            let du = (u0 - scorch_c).abs();
            if du < 0.22 && j >= nv / 2 {
                c = SCORCH;
            }
            let c = shade(c, 0.9 + 0.12 * rng.f32());
            // front face looks up/out (the arch is convex upwards)
            b.quad2(&Mat4::IDENTITY, [q[0], q[3], q[2], q[1]], c, INNER);
        }
    }
    // frames on the underside
    for k in 0..3 {
        let v = 0.15 + k as f32 * 0.3;
        for i in 0..nu {
            let (u0, u1) = (i as f32 / nu as f32, (i + 1) as f32 / nu as f32);
            let (a, c) = (pt(u0, v, &ends), pt(u1, v, &ends));
            let mid = (a + c) * 0.5 - Vec3::Y * 0.12;
            let dir = c - a;
            let r = Quat::from_rotation_arc(Vec3::X, dir.normalize());
            b.cuboid(&mat(mid, r), Vec3::new(dir.length() * 0.5, 0.09, 0.07), FRAME);
        }
    }
    b.build()
}

/// A propellant tank on its side, half sunk, one end blown open into curled petals.
pub fn torn_tank() -> Mesh {
    let mut rng = Rng::new(0x7A4C);
    let mut b = Fb::default();
    let (r, len, sides) = (1.5f32, 5.5f32, 14u32);
    // lie along +Z: the tube is built along +Y and turned
    let m = mat(Vec3::new(0.0, r * 0.75, -len * 0.5), Quat::from_rotation_x(PI * 0.5));
    let mut rings = vec![(0.0, r * 0.35), (0.35, r * 0.8), (0.9, r)];
    for k in 1..=5 {
        rings.push((0.9 + k as f32 * (len - 1.6) / 5.0, r));
    }
    b.tube(&m, &rings, sides, METAL, INNER);
    // end cap
    for i in 0..sides {
        let a0 = i as f32 / sides as f32 * TAU;
        let a1 = (i + 1) as f32 / sides as f32 * TAU;
        let p = |a: f32| Vec3::new(a.cos() * r * 0.35, 0.0, a.sin() * r * 0.35);
        b.tri(&m, Vec3::ZERO, p(a0), p(a1), shade(METAL, 0.8));
    }
    // torn end: petals bent outwards, each its own length
    let y0 = len - 0.7;
    for i in 0..sides {
        let a0 = i as f32 / sides as f32 * TAU;
        let a1 = (i + 1) as f32 / sides as f32 * TAU;
        let l = rng.range(0.2, 1.4);
        let flare = rng.range(0.1, 0.8);
        let p = |a: f32, rr: f32, y: f32| Vec3::new(a.cos() * rr, y, a.sin() * rr);
        let tip = (a0 + a1) * 0.5;
        b.tri(&m, p(a0, r, y0), p(tip, r + flare, y0 + l), p(a1, r, y0), shade(SCORCH, 1.2));
        b.tri(&m, p(a1, r, y0), p(tip, r + flare, y0 + l), p(a0, r, y0), INNER);
    }
    // straps and a stencil band
    for y in [1.2f32, len - 1.6] {
        b.tube(&m, &[(y, r * 1.03), (y + 0.25, r * 1.03)], sides, FRAME, FRAME);
    }
    b.tube(&m, &[(2.2, r * 1.01), (2.9, r * 1.01)], sides, STRIPE, INNER);
    // a broken feed pipe
    b.cyl(&mat(Vec3::new(0.0, r * 1.6, -len * 0.5 + 0.4), Quat::from_rotation_z(0.9)), 0.12, 2.2, 6, FRAME);
    b.build()
}

/// The main engine bell, fallen on its side: wide flared nozzle, burnt inside, with the
/// combustion chamber and broken pipework at the throat.
pub fn engine_bell() -> Mesh {
    let mut b = Fb::default();
    let sides = 18;
    // on its side: the axis is raised by the cone's half-angle so that the flank lies along
    // the ground; the chamber behind the throat is dug in
    let lie = Quat::from_rotation_z(-PI * 0.5 + 0.43);
    let m = mat(Vec3::new(-1.7, 0.5, 0.0), lie);
    let mut rings = Vec::new();
    for k in 0..=7 {
        let t = k as f32 / 7.0;
        rings.push((t * 3.8, 0.65 + 1.75 * t.powf(1.7)));
    }
    b.tube(&m, &rings, sides, [0.42, 0.33, 0.28], [0.1, 0.09, 0.1]);
    // heat-blued lip
    b.tube(&m, &[(3.8, 2.4), (3.95, 2.48)], sides, [0.32, 0.36, 0.5], [0.32, 0.36, 0.5]);
    // throat ring and chamber
    b.tube(&m, &[(-0.25, 0.8), (0.1, 0.8)], 12, FRAME, INNER);
    b.tube(&m, &[(-1.6, 0.95), (-0.25, 0.95)], 12, METAL, INNER);
    b.tube(&m, &[(-1.75, 0.6), (-1.6, 0.95)], 12, FRAME, INNER);
    // pipes and a bent strut
    for (k, a) in [0.4f32, 2.3, 4.1].iter().enumerate() {
        let o = Vec3::new(a.cos() * 1.05, -1.2 + k as f32 * 0.3, a.sin() * 1.05);
        let pm = m * mat(o, Quat::from_rotation_x(0.3 * k as f32 - 0.3));
        b.cyl(&pm, 0.1, 1.6, 6, if k == 1 { STRIPE } else { METAL });
    }
    b.cuboid(&(m * mat(Vec3::new(1.2, -1.9, 0.3), Quat::from_rotation_z(0.6))), Vec3::new(0.12, 0.9, 0.12), FRAME);
    b.build()
}

/// A bent structural rib sticking out of the ground at an angle, with cross bracing.
pub fn rib_beam() -> Mesh {
    let mut b = Fb::default();
    let segs = 7;
    let pt = |t: f32| {
        let a = t * 1.3;
        Vec3::new(4.5 * (1.0 - a.cos()), 6.0 * a.sin() / 1.3 * 0.9 - 0.8, 0.0)
    };
    for k in 0..segs {
        let (a, c) = (pt(k as f32 / segs as f32), pt((k + 1) as f32 / segs as f32));
        let mid = (a + c) * 0.5;
        let d = c - a;
        let r = Quat::from_rotation_arc(Vec3::X, d.normalize());
        for z in [-0.45f32, 0.45] {
            b.cuboid(&mat(mid + Vec3::Z * z, r), Vec3::new(d.length() * 0.52, 0.16, 0.09), if k == 3 { STRIPE } else { FRAME });
        }
        b.cuboid(&mat(a, r), Vec3::new(0.08, 0.1, 0.5), METAL);
        if k % 2 == 0 {
            let rr = r * Quat::from_rotation_y(0.8);
            b.cuboid(&mat(mid, rr), Vec3::new(0.05, 0.05, 0.62), METAL);
        }
    }
    // a scrap of skin still riveted to the top
    let top = pt(0.85);
    b.quad2(&Mat4::IDENTITY, [top + Vec3::new(-0.6, 0.1, -0.9), top + Vec3::new(-0.6, 0.1, 0.9), top + Vec3::new(0.7, 0.6, 1.1), top + Vec3::new(0.5, 0.5, -0.8)], HULL, INNER);
    b.build()
}

/// Meshes of every wreck piece kind, indexed by the `W_*` constants.
pub fn wreck_meshes() -> Vec<Mesh> {
    vec![hull_plate(0), hull_plate(1), torn_tank(), engine_bell(), rib_beam()]
}

/// A small pickup of wreckage: a buckled plate, a pipe stub, a bracket.
pub fn scrap_mesh(variant: u32) -> Mesh {
    let mut rng = Rng::new(0x5C4A_u64 + variant as u64 * 131);
    let mut b = Fb::default();
    let c = if variant == 1 { STRIPE } else { HULL };
    // buckled plate in two facets
    let w = rng.range(0.45, 0.65);
    let l = rng.range(0.5, 0.8);
    let bend = rng.range(0.15, 0.35);
    let pa = [Vec3::new(-w, 0.05, -l), Vec3::new(w, 0.05, -l), Vec3::new(w, 0.12 + bend, 0.0), Vec3::new(-w, 0.12 + bend, 0.0)];
    let pb = [pa[3], pa[2], Vec3::new(w * 0.8, 0.08, l), Vec3::new(-w, 0.04, l * 0.9)];
    b.quad2(&Mat4::IDENTITY, [pa[0], pa[3], pa[2], pa[1]], shade(c, 1.0), INNER);
    b.quad2(&Mat4::IDENTITY, [pb[0], pb[3], pb[2], pb[1]], shade(c, 0.9), INNER);
    b.cyl(&mat(Vec3::new(rng.range(-0.3, 0.3), 0.05, 0.2), Quat::from_rotation_z(1.2) * Quat::from_rotation_y(rng.range(0.0, TAU))), 0.07, 0.7, 6, METAL);
    b.cuboid(&mat(Vec3::new(-0.25, 0.18, -0.25), Quat::from_rotation_y(0.5)), Vec3::new(0.18, 0.12, 0.05), FRAME);
    b.build()
}

/// Cave growth that gives off light: a clump of thin stalks with glowing bulbs and caps.
/// Drawn with an unlit material, so the stalk colours are kept dim.
pub fn glow_mesh(color: [f32; 3], variant: u32) -> Mesh {
    let mut rng = Rng::new(0x6107_u64 + variant as u64 * 53);
    let mut b = Fb::default();
    let stalk = shade(color, 0.22);
    let n = 5 + rng.int(4);
    for _ in 0..n {
        let a = rng.range(0.0, TAU);
        let o = Vec3::new(a.cos(), 0.0, a.sin()) * rng.range(0.0, 0.45);
        let h = rng.range(0.25, 0.9) * if variant == 2 { 0.6 } else { 1.0 };
        let tilt = Quat::from_rotation_y(a) * Quat::from_rotation_z(rng.range(-0.35, 0.35));
        let m = mat(o, tilt);
        b.cyl(&m, 0.03, h, 4, stalk);
        let top = m.transform_point3(Vec3::Y * h);
        let s = rng.range(0.07, 0.15) * if variant == 2 { 1.6 } else { 1.0 };
        let c = shade(color, rng.range(0.85, 1.15));
        // octahedral bulb
        let ax = [Vec3::X * s, Vec3::Z * s, -Vec3::X * s, -Vec3::Z * s];
        for k in 0..4 {
            let (p0, p1) = (top + ax[k], top + ax[(k + 1) % 4]);
            b.tri(&Mat4::IDENTITY, top + Vec3::Y * s * 1.6, p1, p0, c);
            b.tri(&Mat4::IDENTITY, top - Vec3::Y * s, p0, p1, shade(c, 0.75));
        }
    }
    // a glowing film on the rock around the clump
    for k in 0..6 {
        let a0 = k as f32 / 6.0 * TAU;
        let a1 = (k + 1) as f32 / 6.0 * TAU;
        let r = 0.6;
        b.tri(&Mat4::IDENTITY, Vec3::Y * 0.02, Vec3::new(a1.cos() * r, 0.02, a1.sin() * r), Vec3::new(a0.cos() * r, 0.02, a0.sin() * r), shade(color, 0.45));
    }
    b.build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::render::mesh::VertexAttributeValues;

    #[test]
    fn props_are_sane_meshes() {
        let mut all = wreck_meshes();
        all.push(scrap_mesh(0));
        all.push(scrap_mesh(1));
        all.push(glow_mesh([0.3, 1.0, 0.8], 0));
        assert_eq!(all.len() as u8, W_PIECES + 3);
        for m in &all {
            let Some(VertexAttributeValues::Float32x3(p)) = m.attribute(Mesh::ATTRIBUTE_POSITION) else { panic!("no positions") };
            assert!(p.len() >= 30);
            for v in p {
                assert!(v.iter().all(|x| x.is_finite() && x.abs() < 12.0), "vertex out of range: {v:?}");
            }
        }
    }
}
