//! Natural decor meshes: ore deposits (one shape language per material), alien flora per
//! archetype, specimens and black box fragments. Flat shaded, vertex coloured, with baked
//! "sky light" (facets facing up are lighter) so they read even without shadows (phone).
//! Glowing parts use the per-vertex glow lookup (`MB::set_glow`).
use crate::galaxy::{Arch, Species};
use crate::items::Item;
use crate::meshgen::{at, atr, MB};
use crate::util::{mixc, Rng};
use bevy::prelude::*;
use std::f32::consts::{PI, TAU};

type C3 = [f32; 3];

fn sc(c: C3, s: f32) -> C3 {
    [(c[0] * s).clamp(0.0, 1.0), (c[1] * s).clamp(0.0, 1.0), (c[2] * s).clamp(0.0, 1.0)]
}

/// Baked sky light: facets facing up are lighter, facets facing down darker.
fn lit(c: C3, dir: Vec3) -> C3 {
    sc(c, 0.74 + 0.3 * dir.y)
}

fn h2(a: u32, b: u32) -> f32 {
    let mut h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA6B).rotate_left(13);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xffff) as f32 / 65535.0
}

fn up_to(dir: Vec3) -> Quat {
    Quat::from_rotation_arc(Vec3::Y, dir.normalize_or(Vec3::Y))
}

fn tilted(rng: &mut Rng, min: f32, max: f32) -> (Vec3, f32) {
    let az = rng.range(0.0, TAU);
    let tilt = rng.range(min, max);
    (Vec3::new(az.cos() * tilt.sin(), tilt.cos(), az.sin() * tilt.sin()), az)
}

/// Low rocky mound the deposit sits on (half buried).
fn mound(b: &mut MB, rng: &mut Rng, r: f32, h: f32, a: C3, c: C3) {
    let m = at(0.0, 0.0, 0.0) * Mat4::from_rotation_y(rng.range(0.0, TAU));
    b.blob(m, Vec3::new(r, h, r * rng.range(0.8, 1.0)), 1, 0.16, rng, &mut |d, x| lit(mixc(a, c, x), d));
}

/// Crystal column: `sides`-gon of radius `r` (a little narrower at the root), body `h`,
/// pyramid tip `tip`. Faces alternate between `fa` and `fb`, the tip faces use `ft`.
#[allow(clippy::too_many_arguments)]
fn column(b: &mut MB, m: Mat4, sides: u32, r: f32, h: f32, tip: f32, fa: C3, fb: C3, ft: C3) {
    let prof = [(r * 0.8, -0.15), (r, h * 0.2), (r, h), (0.0, h + tip)];
    b.lathe(m, &prof, sides, 0.0, &mut |seg, i| {
        let base = if seg == 2 { ft } else if i % 2 == 0 { fa } else { fb };
        sc(base, if seg == 2 { 1.0 - 0.12 * (i % 3) as f32 } else { 0.92 + 0.1 * h2(i, seg as u32) })
    });
}

/// Double-pointed crystal (bipyramid) centred on its waist.
fn bipyramid(b: &mut MB, m: Mat4, sides: u32, r: f32, h: f32, tip: f32, fa: C3, fb: C3, ft: C3) {
    let prof = [(0.0, -tip), (r, 0.0), (r, h), (0.0, h + tip)];
    b.lathe(m, &prof, sides, 0.0, &mut |seg, i| {
        let base = if seg != 1 { ft } else if i % 2 == 0 { fa } else { fb };
        sc(base, 0.9 + 0.12 * h2(i, seg as u32 + 7))
    });
}

/// Surface deposit of one ore type. Every ore has its own silhouette and colour logic so it
/// can be told apart at 5-25 m: iron rusty nuggets, copper green crusts on dark rock, quartz
/// clear hexagonal prisms, carbon black layered slabs, ice cyan shards, titanium steel-blue
/// blades, lithium pink mica rosettes, uranite black ore with glowing green crystals,
/// heliosite glowing amber bipyramids.
pub fn crystal(item: Item, variant: u32) -> MB {
    let mut rng = Rng::new(0xC0FFEE ^ (item as u64 + 1).wrapping_mul(0x9E37_79B9) ^ (variant as u64 * 7919));
    let rng = &mut rng;
    let mut b = MB::new();
    match item {
        Item::IronOre => {
            mound(&mut b, rng, 0.75, 0.24, [0.3, 0.2, 0.17], [0.38, 0.25, 0.2]);
            let pal: [C3; 4] = [[0.68, 0.31, 0.16], [0.32, 0.22, 0.21], [0.5, 0.47, 0.48], [0.88, 0.48, 0.2]];
            let n = 4 + rng.int(3);
            for k in 0..n {
                let a = rng.range(0.0, TAU);
                let d = if k == 0 { 0.0 } else { rng.range(0.2, 0.5) };
                let r = if k == 0 { rng.range(0.32, 0.42) } else { rng.range(0.16, 0.3) };
                let s = Vec3::new(r * rng.range(0.8, 1.3), r * rng.range(0.7, 1.1), r * rng.range(0.8, 1.3));
                let rot = Quat::from_euler(EulerRot::XYZ, rng.range(-0.6, 0.6), rng.range(0.0, TAU), rng.range(-0.6, 0.6));
                let m = atr(a.cos() * d, s.y * 0.75 + 0.05, a.sin() * d, rot);
                b.blob(m, s, 0, 0.3, rng, &mut |dir, x| {
                    let c = if x < 0.45 { pal[0] } else if x < 0.7 { pal[1] } else if x < 0.86 { pal[2] } else { pal[3] };
                    lit(c, dir)
                });
            }
            for _ in 0..5 {
                let a = rng.range(0.0, TAU);
                let d = rng.range(0.6, 0.95);
                let r = rng.range(0.05, 0.1);
                b.blob(at(a.cos() * d, r * 0.4, a.sin() * d), Vec3::splat(r), 0, 0.3, rng, &mut |dir, x| lit(if x < 0.6 { pal[0] } else { pal[1] }, dir));
            }
        }
        Item::CopperOre => {
            let br = Vec3::new(0.55, 0.42, 0.5) * rng.range(0.9, 1.1);
            let centre = Vec3::new(0.0, 0.22, 0.0);
            let yaw = Mat4::from_rotation_y(rng.range(0.0, TAU));
            b.blob(Mat4::from_translation(centre) * yaw, br, 1, 0.14, rng, &mut |d, x| lit(mixc([0.2, 0.22, 0.23], [0.3, 0.29, 0.3], x), d));
            let pal: [C3; 4] = [[0.1, 0.56, 0.44], [0.22, 0.74, 0.56], [0.07, 0.4, 0.35], [0.36, 0.84, 0.66]];
            let n = 16 + rng.int(8);
            for _ in 0..n {
                let mut d = rng.dir();
                d.y = d.y.abs() * 0.9 + 0.1;
                let d = d.normalize();
                let p = centre + d * br * 0.92;
                let r = rng.range(0.08, 0.16);
                let m = Mat4::from_rotation_translation(up_to(d), p);
                let blue = rng.chance(0.1);
                let band = rng.int(4) as usize;
                b.blob(m, Vec3::new(r, r * 0.55, r), 0, 0.2, rng, &mut |dir, x| {
                    let c = if blue { [0.16, 0.34, 0.78] } else { pal[(band + (x * 1.6) as usize) % 4] };
                    lit(c, dir)
                });
            }
            for _ in 0..2 {
                let mut d = rng.dir();
                d.y = d.y.abs() * 0.5 + 0.2;
                let p = centre + d.normalize() * br;
                b.blob(at(p.x, p.y, p.z), Vec3::splat(rng.range(0.06, 0.09)), 0, 0.3, rng, &mut |dir, x| lit(mixc([0.9, 0.5, 0.28], [0.68, 0.33, 0.18], x), dir));
            }
        }
        Item::Quartz => {
            mound(&mut b, rng, 0.55, 0.2, [0.6, 0.58, 0.58], [0.48, 0.47, 0.5]);
            let n = 5 + rng.int(5);
            for k in 0..n {
                let (dir, _) = if k == 0 { tilted(rng, 0.0, 0.2) } else { tilted(rng, 0.2, 0.75) };
                let len = if k == 0 { rng.range(0.85, 1.2) } else { rng.range(0.3, 0.8) };
                let r = len * rng.range(0.11, 0.15);
                let o = Vec3::new(dir.x, 0.0, dir.z) * 0.18;
                let m = Mat4::from_rotation_translation(up_to(dir), o + Vec3::Y * 0.12) * Mat4::from_rotation_y(rng.range(0.0, 1.0));
                let smoky = rng.chance(0.25);
                let (fa, fb) = if smoky { ([0.84, 0.82, 0.9], [0.6, 0.6, 0.7]) } else { ([0.97, 0.97, 1.0], [0.72, 0.75, 0.88]) };
                column(&mut b, m, 6, r, len, r * 1.6, fa, fb, [1.0, 1.0, 1.0]);
            }
        }
        Item::Carbon => {
            mound(&mut b, rng, 0.7, 0.2, [0.17, 0.15, 0.14], [0.24, 0.21, 0.19]);
            let n = 3 + rng.int(3);
            let mut y = 0.12;
            for _ in 0..n {
                let s = Vec3::new(rng.range(0.32, 0.55), rng.range(0.09, 0.15), rng.range(0.28, 0.45));
                let rot = Quat::from_euler(EulerRot::YXZ, rng.range(0.0, TAU), rng.range(-0.15, 0.15), rng.range(-0.15, 0.15));
                let m = atr(rng.range(-0.12, 0.12), y + s.y, rng.range(-0.12, 0.12), rot);
                b.blob(m, s, 0, 0.18, rng, &mut |dir, x| {
                    let c = if x < 0.5 { [0.09, 0.09, 0.1] } else if x < 0.85 { [0.16, 0.16, 0.18] } else { [0.36, 0.4, 0.5] };
                    lit(c, dir)
                });
                y += s.y * 1.5;
            }
            for _ in 0..4 {
                let a = rng.range(0.0, TAU);
                let d = rng.range(0.5, 0.85);
                let r = rng.range(0.07, 0.13);
                b.blob(at(a.cos() * d, r * 0.5, a.sin() * d), Vec3::new(r, r * 0.7, r), 0, 0.25, rng, &mut |dir, x| lit(if x < 0.7 { [0.1, 0.1, 0.11] } else { [0.3, 0.33, 0.42] }, dir));
            }
        }
        Item::Ice => {
            mound(&mut b, rng, 0.7, 0.18, [0.9, 0.95, 1.0], [0.74, 0.84, 0.95]);
            b.set_glow(Some(([0.5, 0.9, 1.0], 0.12)));
            let n = 5 + rng.int(4);
            for k in 0..n {
                let (dir, _) = if k == 0 { tilted(rng, 0.0, 0.15) } else { tilted(rng, 0.15, 0.7) };
                let len = if k == 0 { rng.range(0.9, 1.3) } else { rng.range(0.35, 0.85) };
                let r = len * rng.range(0.09, 0.13);
                let o = Vec3::new(dir.x, 0.0, dir.z) * 0.2;
                let m = Mat4::from_rotation_translation(up_to(dir), o + Vec3::Y * 0.08) * Mat4::from_rotation_y(rng.range(0.0, 1.5));
                column(&mut b, m, 4 + rng.int(2), r, len * 0.6, len * 0.45, [0.78, 0.94, 1.0], [0.46, 0.74, 0.94], [0.95, 0.99, 1.0]);
            }
            b.set_glow(None);
        }
        Item::TitaniumOre => {
            mound(&mut b, rng, 0.62, 0.24, [0.24, 0.25, 0.29], [0.32, 0.33, 0.37]);
            let n = 4 + rng.int(4);
            let fan = rng.range(0.0, TAU);
            for k in 0..n {
                let az = fan + (k as f32 - n as f32 * 0.5) * 0.45 + rng.range(-0.15, 0.15);
                let tilt = if k == n / 2 { 0.05 } else { rng.range(0.15, 0.6) };
                let dir = Vec3::new(az.cos() * tilt.sin(), tilt.cos(), az.sin() * tilt.sin());
                let len = rng.range(0.45, 1.05) * if k == n / 2 { 1.2 } else { 1.0 };
                let w = rng.range(0.2, 0.3);
                // a flat blade: a square column turned 45 degrees, squashed in one direction
                let m = Mat4::from_rotation_translation(up_to(dir) * Quat::from_rotation_y(az), Vec3::new(dir.x, 0.0, dir.z) * 0.15 + Vec3::Y * 0.12)
                    * Mat4::from_scale(Vec3::new(1.0, 1.0, 0.2))
                    * Mat4::from_rotation_y(PI / 4.0);
                // dark gunmetal with an anodised blue-violet sheen and bright cut edges
                let steel = if rng.chance(0.5) { [0.2, 0.25, 0.48] } else { [0.36, 0.28, 0.56] };
                column(&mut b, m, 4, w, len, w * 0.8, steel, [0.8, 0.84, 0.92], [0.62, 0.66, 0.8]);
            }
        }
        Item::Lithium => {
            mound(&mut b, rng, 0.62, 0.22, [0.8, 0.76, 0.7], [0.66, 0.62, 0.6]);
            let books = 3 + rng.int(3);
            for k in 0..books {
                let az = k as f32 / books as f32 * TAU + rng.range(-0.3, 0.3);
                let tilt = rng.range(0.85, 1.3);
                let naz = az + rng.range(0.4, 1.1);
                let n = Vec3::new(naz.cos() * tilt.sin(), tilt.cos(), naz.sin() * tilt.sin());
                let base = Vec3::new(az.cos(), 0.0, az.sin()) * rng.range(0.1, 0.28) + Vec3::Y * rng.range(0.3, 0.5);
                let r = rng.range(0.27, 0.37);
                let layers = 1 + rng.int(2);
                for l in 0..layers {
                    let rr = r * (1.0 - 0.18 * l as f32);
                    let t = rng.range(0.035, 0.055);
                    let m = Mat4::from_rotation_translation(up_to(n), base + n * (l as f32 * 0.055)) * Mat4::from_rotation_y(rng.range(0.0, 1.0));
                    let top = if (k + l) % 2 == 0 { [0.95, 0.48, 0.76] } else { [0.78, 0.38, 0.7] };
                    b.lathe(m, &[(0.0, 0.0), (rr, 0.0), (rr, t)], 6, 0.0, &mut |seg, i| match seg {
                        0 => sc(top, 0.55),
                        1 => sc([0.98, 0.82, 0.92], 0.9 + 0.1 * (i % 2) as f32),
                        _ => top,
                    });
                }
            }
        }
        Item::Uranite => {
            let br = Vec3::new(0.52, 0.38, 0.46) * rng.range(0.9, 1.1);
            let centre = Vec3::new(0.0, 0.2, 0.0);
            b.blob(Mat4::from_translation(centre) * Mat4::from_rotation_y(rng.range(0.0, TAU)), br, 1, 0.16, rng, &mut |d, x| lit(mixc([0.09, 0.1, 0.09], [0.17, 0.18, 0.15], x), d));
            b.set_glow(Some(([0.6, 1.0, 0.3], 0.22)));
            for _ in 0..9 {
                let mut d = rng.dir();
                d.y = d.y.abs() * 0.8 + 0.15;
                let d = d.normalize();
                let r = rng.range(0.05, 0.09);
                b.blob(Mat4::from_rotation_translation(up_to(d), centre + d * br * 0.95), Vec3::new(r, r * 0.4, r), 0, 0.2, rng, &mut |dir, _| lit([0.78, 0.86, 0.26], dir));
            }
            b.set_glow(Some(([0.4, 1.0, 0.25], 0.6)));
            let n = 4 + rng.int(4);
            for _ in 0..n {
                let mut d = rng.dir();
                d.y = d.y.abs() * 0.7 + 0.3;
                let d = d.normalize();
                let r = rng.range(0.07, 0.11);
                let m = Mat4::from_rotation_translation(up_to(d), centre + d * br * 0.85) * Mat4::from_rotation_y(rng.range(0.0, 1.5));
                column(&mut b, m, 4, r, rng.range(0.2, 0.42), r * 0.7, [0.46, 0.96, 0.3], [0.3, 0.75, 0.2], [0.7, 1.0, 0.5]);
            }
            b.set_glow(None);
        }
        Item::Heliosite => {
            mound(&mut b, rng, 0.62, 0.2, [0.12, 0.1, 0.12], [0.2, 0.16, 0.16]);
            let gold: C3 = [1.0, 0.62, 0.12];
            let deep: C3 = [0.85, 0.36, 0.05];
            let tip: C3 = [1.0, 0.82, 0.4];
            b.set_glow(Some(([1.0, 0.5, 0.08], 0.42)));
            let (dir, _) = tilted(rng, 0.05, 0.3);
            let r = rng.range(0.17, 0.22);
            let m = Mat4::from_rotation_translation(up_to(dir), Vec3::Y * 0.12) * Mat4::from_rotation_y(rng.range(0.0, 1.0));
            bipyramid(&mut b, m, 6, r, rng.range(0.55, 0.75), 0.3, gold, deep, tip);
            b.set_glow(Some(([1.0, 0.45, 0.06], 0.32)));
            for _ in 0..2 + rng.int(3) {
                let (dir, az) = tilted(rng, 0.4, 0.9);
                let r = rng.range(0.07, 0.11);
                let o = Vec3::new(az.cos(), 0.0, az.sin()) * rng.range(0.25, 0.45);
                let m = Mat4::from_rotation_translation(up_to(dir), o + Vec3::Y * 0.08);
                bipyramid(&mut b, m, 6, r, r * 2.5, r * 1.6, gold, deep, tip);
            }
            // a few shards hover above the deposit
            for k in 0..3 {
                let a = k as f32 / 3.0 * TAU + rng.range(0.0, 1.0);
                let r = rng.range(0.035, 0.055);
                let m = atr(a.cos() * 0.45, rng.range(0.9, 1.35), a.sin() * 0.45, Quat::from_euler(EulerRot::XYZ, rng.range(-0.5, 0.5), a, rng.range(-0.5, 0.5)));
                bipyramid(&mut b, m, 4, r, r * 1.5, r * 1.8, gold, deep, tip);
            }
            b.set_glow(None);
        }
        _ => {
            mound(&mut b, rng, 0.6, 0.25, [0.4, 0.4, 0.42], [0.5, 0.5, 0.52]);
        }
    }
    b.scale(1.3);
    b
}

/// Specimen (scannable alien sample): a rock pedestal with a glowing cyan crystal growth and
/// a hovering ringed orb.
pub fn specimen() -> MB {
    let mut rng = Rng::new(0x5EC1);
    let rng = &mut rng;
    let mut b = MB::new();
    b.blob(at(0.0, 0.08, 0.0), Vec3::new(0.45, 0.24, 0.4), 1, 0.15, rng, &mut |d, x| lit(mixc([0.3, 0.29, 0.34], [0.4, 0.38, 0.44], x), d));
    b.set_glow(Some(([0.35, 0.85, 1.0], 0.5)));
    for k in 0..5 {
        let (dir, az) = tilted(rng, 0.1, 0.7);
        let o = Vec3::new(az.cos(), 0.0, az.sin()) * 0.12;
        let len = if k == 0 { 0.4 } else { rng.range(0.15, 0.3) };
        column(&mut b, Mat4::from_rotation_translation(up_to(dir), o + Vec3::Y * 0.22), 5, len * 0.18, len, len * 0.3, [0.4, 0.85, 1.0], [0.25, 0.6, 0.9], [0.8, 0.97, 1.0]);
    }
    b.set_glow(Some(([0.4, 0.9, 1.0], 0.9)));
    b.blob(at(0.0, 0.95, 0.0), Vec3::splat(0.14), 1, 0.08, rng, &mut |_, x| mixc([0.55, 0.92, 1.0], [0.8, 1.0, 1.0], x));
    b.set_glow(Some(([0.5, 0.9, 1.0], 0.4)));
    let ring = Mat4::from_rotation_translation(Quat::from_rotation_x(0.35), Vec3::new(0.0, 0.95, 0.0));
    b.tube(ring, &(0..=16).map(|i| { let a = i as f32 / 16.0 * TAU; Vec3::new(a.cos() * 0.3, 0.0, a.sin() * 0.3) }).collect::<Vec<_>>(), &[0.018; 17], 4, &mut |_, _| [0.85, 0.95, 1.0]);
    b.set_glow(None);
    b
}

/// Black box fragment: a torn hull panel with the orange recorder, a loose cable and a red
/// blinking light.
pub fn fragment() -> MB {
    let mut b = MB::new();
    let white: C3 = [0.86, 0.86, 0.88];
    let orange: C3 = [0.95, 0.4, 0.1];
    // bent hull panel in two pieces with an orange stripe
    let p1 = Mat4::from_rotation_translation(Quat::from_rotation_z(-0.25) * Quat::from_rotation_y(0.3), Vec3::new(0.55, 0.12, 0.25));
    b.cuboid(p1, Vec3::new(0.6, 0.03, 0.45), white);
    b.cuboid(p1 * at(0.0, 0.035, 0.2), Vec3::new(0.6, 0.01, 0.07), orange);
    let p2 = Mat4::from_rotation_translation(Quat::from_rotation_x(0.9) * Quat::from_rotation_y(-0.5), Vec3::new(-0.6, 0.22, -0.3));
    b.cuboid(p2, Vec3::new(0.35, 0.03, 0.5), [0.32, 0.32, 0.36]);
    // ribs sticking out of the torn edge
    for k in 0..3 {
        b.cuboid(atr(0.05 + k as f32 * 0.2, 0.15, -0.35, Quat::from_rotation_x(-0.6 - k as f32 * 0.2)), Vec3::new(0.025, 0.025, 0.3), [0.55, 0.56, 0.6]);
    }
    // the recorder: orange box with dark bands and a handle
    let rec = Mat4::from_rotation_translation(Quat::from_rotation_z(0.4) * Quat::from_rotation_x(0.25), Vec3::new(0.0, 0.3, 0.0));
    b.cuboid(rec, Vec3::new(0.3, 0.22, 0.4), orange);
    b.cuboid(rec, Vec3::new(0.31, 0.05, 0.41), [0.95, 0.95, 0.9]);
    for z in [-0.28, 0.28] {
        b.cuboid(rec * at(0.0, 0.0, z), Vec3::new(0.315, 0.225, 0.03), [0.15, 0.15, 0.17]);
    }
    b.cuboid(rec * at(0.0, 0.25, 0.0), Vec3::new(0.04, 0.03, 0.22), [0.2, 0.2, 0.22]);
    // cable
    let pts: Vec<Vec3> = (0..6).map(|i| Vec3::new(0.3 + i as f32 * 0.16, 0.04 + (i as f32 * 1.7).sin().abs() * 0.04, -0.2 - i as f32 * 0.1)).collect();
    b.tube(Mat4::IDENTITY, &pts, &[0.03; 6], 4, &mut |_, _| [0.12, 0.12, 0.14]);
    // red light
    b.set_glow(Some(([1.0, 0.15, 0.1], 1.0)));
    b.blob(rec * at(0.0, 0.27, -0.25), Vec3::splat(0.06), 0, 0.0, &mut Rng::new(1), &mut |_, _| [1.0, 0.25, 0.2]);
    b.set_glow(None);
    b
}

// ---------------------------------------------------------------- flora

struct Pal {
    leaf: C3,
    hi: C3,
    lo: C3,
    alt: C3,
    stem: C3,
    glow: Option<C3>,
}

fn palette(sp: &Species) -> Pal {
    let mut rng = Rng::new(sp.seed ^ 0x9A1E77E);
    let leaf = sp.leaf;
    let hi = sc(mixc(leaf, [1.0, 0.95, 0.6], 0.22), 1.18);
    let lo = sc(mixc(leaf, [0.05, 0.1, 0.22], 0.35), 0.72);
    // a second leaf hue for colour variation inside a crown: shifted around the colour wheel
    let alt = if rng.chance(0.5) { mixc(leaf, [leaf[1], leaf[2], leaf[0]], 0.3) } else { mixc(leaf, [0.9, 0.75, 0.2], 0.3) };
    let glows = match sp.arch {
        Arch::Bulb => true,
        Arch::Mushroom | Arch::Coral => rng.chance(0.6),
        Arch::Bush => rng.chance(0.4),
        Arch::Cactus | Arch::Fern | Arch::Tree => rng.chance(0.3),
        Arch::Pine => rng.chance(0.15),
    };
    let glow_pal: [C3; 6] = [[0.35, 0.9, 1.0], [1.0, 0.78, 0.3], [0.85, 0.4, 1.0], [0.6, 1.0, 0.35], [1.0, 0.45, 0.6], [0.4, 1.0, 0.8]];
    let glow = glows.then(|| *rng.pick(&glow_pal));
    Pal { leaf, hi, lo, alt, stem: sp.stem, glow }
}

/// Crown facet colour: sky-lit tops, dark undersides, some facets in the second hue.
fn crown(p: &Pal, dir: Vec3, x: f32) -> C3 {
    let t = dir.y * 0.5 + 0.5;
    let c = if x > 0.86 { mixc(p.alt, p.hi, t * 0.5) } else { mixc(p.lo, if t > 0.5 { mixc(p.leaf, p.hi, (t - 0.5) * 2.0) } else { p.leaf }, (t * 1.6).min(1.0)) };
    sc(c, 0.92 + 0.16 * x)
}

fn bark(p: &Pal, seg: usize, side: u32) -> C3 {
    sc(p.stem, (0.78 + 0.07 * seg as f32).min(1.05) * if side % 2 == 0 { 1.0 } else { 0.86 })
}

/// A plant of one species. `lo` = phone quality (fewer facets).
pub fn plant(sp: &Species, variant: u32, lo: bool) -> MB {
    let mut rng = Rng::new(sp.seed ^ (variant as u64 * 7919));
    let rng = &mut rng;
    let style = (sp.seed % 3) as u32;
    let p = palette(sp);
    let mut b = MB::new();
    let s = sp.size * rng.range(0.8, 1.2);
    let det = if lo { 0 } else { 1 };
    match sp.arch {
        Arch::Tree => tree(&mut b, rng, &p, s, style, det),
        Arch::Pine => pine(&mut b, rng, &p, s, style),
        Arch::Bush => bush(&mut b, rng, &p, s, det),
        Arch::Fern => fern(&mut b, rng, &p, s, style, lo),
        Arch::Cactus => cactus(&mut b, rng, &p, s, style),
        Arch::Mushroom => mushroom(&mut b, rng, &p, s, style),
        Arch::Bulb => lantern(&mut b, rng, &p, s, det),
        Arch::Coral => coral(&mut b, rng, &p, s, lo),
    }
    // the small archetypes are drawn larger than their species size: alien worlds read
    // through giant mushrooms, lanterns and corals (harvest radius is unchanged)
    b.scale(match sp.arch {
        Arch::Mushroom => 1.5,
        Arch::Bulb => 1.4,
        Arch::Coral => 1.35,
        Arch::Fern => 1.2,
        Arch::Bush | Arch::Cactus => 1.1,
        _ => 1.0,
    });
    b
}

/// Polyline from `from` along `dir`, bending by `bend` per step.
fn bent(from: Vec3, dir: Vec3, len: f32, steps: usize, bend: Vec3) -> Vec<Vec3> {
    let mut pts = vec![from];
    let mut p = from;
    let mut d = dir.normalize();
    for _ in 0..steps {
        p += d * len / steps as f32;
        pts.push(p);
        d = (d + bend).normalize();
    }
    pts
}

fn tree(b: &mut MB, rng: &mut Rng, p: &Pal, s: f32, style: u32, det: u32) {
    let h = s * if style == 1 { 2.1 } else { 1.7 };
    let lean = Vec3::new(rng.range(-0.06, 0.06), 0.0, rng.range(-0.06, 0.06));
    let trunk = bent(Vec3::ZERO, Vec3::Y + lean, h, 4, lean * 0.5);
    let radii = [0.26 * s, 0.17 * s, 0.14 * s, 0.11 * s, 0.08 * s];
    b.tube(Mat4::IDENTITY, &trunk, &radii, 6, &mut |k, i| bark(p, k, i));
    let top = trunk[4];
    let mut ends = vec![top];
    let nb = 2 + rng.int(3);
    for k in 0..nb {
        let t = rng.range(0.5, 0.85);
        let seg = ((t * 4.0) as usize).min(3);
        let from = trunk[seg].lerp(trunk[seg + 1], t * 4.0 - seg as f32);
        let az = k as f32 / nb as f32 * TAU + rng.range(-0.4, 0.4);
        let out = Vec3::new(az.cos(), if style == 1 { 0.35 } else { 0.75 }, az.sin());
        let len = rng.range(0.55, 0.9) * s;
        let br = bent(from, out, len, 2, Vec3::Y * 0.25);
        b.tube(Mat4::IDENTITY, &br, &[0.08 * s, 0.06 * s, 0.04 * s], 4, &mut |k, i| bark(p, k + 1, i));
        ends.push(br[2]);
    }
    for (k, e) in ends.iter().enumerate() {
        let r = if k == 0 { rng.range(0.6, 0.85) } else { rng.range(0.45, 0.7) } * s;
        let shape = match style {
            1 => Vec3::new(r * 1.35, r * 0.38, r * 1.35),
            _ => Vec3::new(r, r * 0.82, r),
        };
        let m = Mat4::from_rotation_translation(Quat::from_rotation_y(rng.range(0.0, TAU)), *e + Vec3::Y * shape.y * 0.35);
        b.blob(m, shape, det, 0.2, rng, &mut |d, x| crown(p, d, x));
        if style == 2 {
            // weeping crown: strands of leaves hang from its rim
            for j in 0..5 {
                let a = j as f32 / 5.0 * TAU + rng.range(0.0, 0.8);
                let rim = *e + Vec3::new(a.cos() * shape.x * 0.85, 0.0, a.sin() * shape.z * 0.85);
                let len = rng.range(0.5, 0.9) * s;
                b.leaf(&Mat4::IDENTITY, rim, Vec3::new(a.cos() * 0.3, -1.0, a.sin() * 0.3), len, 0.07 * s, 0.0, 3, 0.3, &mut |k| mixc(p.leaf, p.lo, k as f32 * 0.25));
            }
        }
    }
    if let Some(g) = p.glow {
        // glowing fruit under the crown
        b.set_glow(Some((g, 0.7)));
        for _ in 0..4 + rng.int(3) {
            let e = ends[rng.int(ends.len() as u32) as usize];
            let a = rng.range(0.0, TAU);
            let q = e + Vec3::new(a.cos() * 0.45 * s, -0.15 * s, a.sin() * 0.45 * s);
            b.blob(at(q.x, q.y, q.z), Vec3::splat(0.07 * s), 0, 0.1, rng, &mut |_, x| mixc(g, [1.0, 1.0, 1.0], 0.2 * x));
        }
        b.set_glow(None);
    }
}

fn pine(b: &mut MB, rng: &mut Rng, p: &Pal, s: f32, style: u32) {
    let h = 0.7 * s;
    b.tube(Mat4::IDENTITY, &[Vec3::ZERO, Vec3::Y * h, Vec3::Y * h * 2.2], &[0.17 * s, 0.12 * s, 0.05 * s], 6, &mut |k, i| bark(p, k, i));
    let tiers = 3 + rng.int(2) + if style == 1 { 1 } else { 0 };
    let mut y = h * 0.75;
    let mut r = if style == 2 { 0.75 } else { 0.95 } * s;
    for t in 0..tiers {
        let th = r * if style == 2 { 1.6 } else { 1.2 };
        let droop = r * 0.22;
        let prof = [(r * 0.25, y + droop * 1.4), (r, y), (r * 0.18, y + th * 0.88), (0.0, y + th)];
        let m = Mat4::from_rotation_y(rng.range(0.0, TAU));
        let shade = 0.82 + 0.1 * t as f32;
        b.lathe(m, &prof, 9, 0.28, &mut |seg, i| match seg {
            0 => sc(p.lo, 0.8),
            1 => sc(if i % 2 == 0 { p.leaf } else { mixc(p.leaf, p.lo, 0.45) }, shade),
            _ => sc(mixc(p.leaf, p.hi, 0.6), shade),
        });
        if let (Some(g), true) = (p.glow, t + 1 == tiers) {
            b.set_glow(Some((g, 0.6)));
            b.blob(at(0.0, y + th + 0.06 * s, 0.0), Vec3::splat(0.08 * s), 0, 0.0, rng, &mut |_, _| g);
            b.set_glow(None);
        }
        y += th * 0.55;
        r *= 0.74;
    }
}

fn bush(b: &mut MB, rng: &mut Rng, p: &Pal, s: f32, det: u32) {
    let n = 3 + rng.int(3);
    let mut tops = Vec::new();
    for _ in 0..n {
        let r = rng.range(0.3, 0.52) * s;
        let a = rng.range(0.0, TAU);
        let d = rng.range(0.0, 0.42) * s;
        let c = Vec3::new(a.cos() * d, r * 0.7, a.sin() * d);
        b.blob(Mat4::from_rotation_translation(Quat::from_rotation_y(rng.range(0.0, TAU)), c), Vec3::new(r, r * 0.8, r), det, 0.22, rng, &mut |dir, x| crown(p, dir, x));
        tops.push((c, r));
    }
    // a few stiff leaves poking out
    for _ in 0..4 {
        let (c, r) = tops[rng.int(tops.len() as u32) as usize];
        let a = rng.range(0.0, TAU);
        let dir = Vec3::new(a.cos() * 0.6, 1.0, a.sin() * 0.6);
        b.leaf(&Mat4::IDENTITY, c + Vec3::Y * r * 0.4, dir, 0.45 * s, 0.07 * s, 0.4, 3, 0.0, &mut |k| mixc(p.leaf, p.hi, k as f32 * 0.3));
    }
    let berry = p.glow.unwrap_or(sc(p.stem, 2.2));
    if p.glow.is_some() || rng.chance(0.5) {
        b.set_glow(p.glow.map(|g| (g, 0.55)));
        for _ in 0..6 {
            let (c, r) = tops[rng.int(tops.len() as u32) as usize];
            let mut d = rng.dir();
            d.y = d.y.abs();
            let q = c + d * r * 0.95;
            b.blob(at(q.x, q.y, q.z), Vec3::splat(0.055 * s), 0, 0.1, rng, &mut |dir, _| lit(berry, dir));
        }
        b.set_glow(None);
    }
}

fn fern(b: &mut MB, rng: &mut Rng, p: &Pal, s: f32, style: u32, lo: bool) {
    let n = if lo { 6 } else { 7 + rng.int(4) };
    for i in 0..n {
        let a = i as f32 / n as f32 * TAU + rng.range(-0.25, 0.25);
        let up = if style == 1 { rng.range(1.6, 2.4) } else { rng.range(0.7, 1.3) };
        let dir = Vec3::new(a.cos(), up, a.sin());
        let len = rng.range(0.75, 1.2) * s;
        let w = if style == 2 { 0.18 } else { 0.12 } * s;
        let shade = rng.range(0.85, 1.1);
        b.leaf(&Mat4::IDENTITY, Vec3::Y * 0.03, dir, len, w, rng.range(0.9, 1.6), 6, 0.5, &mut |k| sc(mixc(p.lo, mixc(p.leaf, p.hi, k as f32 / 6.0), (k as f32 / 2.0).min(1.0)), shade));
    }
    // fiddleheads: young curled fronds in the middle, glowing on some species
    b.set_glow(p.glow.map(|g| (g, 0.5)));
    for _ in 0..2 + rng.int(2) {
        let a = rng.range(0.0, TAU);
        let base = Vec3::new(a.cos() * 0.08 * s, 0.0, a.sin() * 0.08 * s);
        let pts = bent(base, Vec3::new(a.cos() * 0.2, 1.0, a.sin() * 0.2), 0.45 * s, 4, Vec3::new(a.cos(), -0.2, a.sin()) * 0.6);
        b.tube(Mat4::IDENTITY, &pts, &[0.03 * s, 0.028 * s, 0.025 * s, 0.03 * s, 0.045 * s], 4, &mut |k, _| if k >= 3 { p.glow.unwrap_or(p.hi) } else { p.leaf });
    }
    b.set_glow(None);
}

fn cactus(b: &mut MB, rng: &mut Rng, p: &Pal, s: f32, style: u32) {
    let h = rng.range(1.1, 1.7) * s * if style == 2 { 0.55 } else { 1.0 };
    let r = 0.24 * s * if style == 2 { 1.7 } else { 1.0 };
    let prof = [(r * 0.85, 0.0), (r, h * 0.12), (r, h * 0.82), (r * 0.8, h * 0.95), (r * 0.35, h * 1.02), (0.0, h * 1.04)];
    let body = p.leaf;
    let mut paint = |seg: usize, i: u32| sc(if i % 2 == 0 { mixc(body, p.hi, 0.25) } else { mixc(body, p.lo, 0.5) }, 0.85 + 0.06 * seg as f32);
    b.lathe(Mat4::from_rotation_y(rng.range(0.0, TAU)), &prof, 12, 0.16, &mut paint);
    let mut tops = vec![Vec3::Y * h * 1.04];
    if style != 2 {
        for k in 0..1 + rng.int(2) {
            let a = rng.range(0.0, TAU);
            let y = h * rng.range(0.32, 0.6);
            let out = Vec3::new(a.cos(), 0.0, a.sin());
            let ar = r * 0.6;
            let up = (0.45 + 0.2 * k as f32) * s;
            let pts = [out * r * 0.5 + Vec3::Y * y, out * (r + 0.3 * s) + Vec3::Y * (y + 0.04 * s), out * (r + 0.42 * s) + Vec3::Y * (y + 0.2 * s), out * (r + 0.42 * s) + Vec3::Y * (y + up), out * (r + 0.42 * s) + Vec3::Y * (y + up + ar)];
            b.tube(Mat4::IDENTITY, &pts, &[ar, ar, ar, ar * 0.9, 0.0], 8, &mut |seg, i| paint(seg + 1, i));
            tops.push(pts[4]);
        }
    }
    // flowers
    let petal = p.glow.unwrap_or(if rng.chance(0.5) { [1.0, 0.45, 0.6] } else { [1.0, 0.85, 0.35] });
    for t in tops {
        if rng.chance(0.75) {
            b.set_glow(p.glow.map(|g| (g, 0.45)));
            let m = at(t.x, t.y - 0.02 * s, t.z) * Mat4::from_rotation_y(rng.range(0.0, TAU));
            b.lathe(m, &[(0.0, 0.0), (0.13 * s, 0.07 * s), (0.04 * s, 0.05 * s)], 10, 0.55, &mut |seg, _| if seg == 0 { sc(petal, 0.8) } else { petal });
            b.set_glow(None);
        }
    }
}

fn mushroom(b: &mut MB, rng: &mut Rng, p: &Pal, s: f32, style: u32) {
    let count = 1 + rng.int(3);
    let gill = p.glow.unwrap_or(sc(p.stem, 1.9));
    for k in 0..count {
        let a = rng.range(0.0, TAU);
        let o = if k == 0 { Vec3::ZERO } else { Vec3::new(a.cos(), 0.0, a.sin()) * rng.range(0.3, 0.55) * s };
        let f = if k == 0 { 1.0 } else { rng.range(0.45, 0.7) };
        let h = rng.range(0.5, 0.95) * s * f * if style == 1 { 1.4 } else { 1.0 };
        let cr = h * rng.range(0.55, 0.85) * if style == 1 { 0.6 } else { 1.0 };
        let sr = 0.07 * s * f.sqrt();
        let lean = Vec3::new(rng.range(-0.15, 0.15), 0.0, rng.range(-0.15, 0.15)) + o.normalize_or_zero() * 0.12;
        let stem = bent(o, Vec3::Y + lean, h, 3, -lean * 0.3);
        let top = stem[3];
        let stem_col = sc(p.stem, 1.8);
        b.tube(Mat4::IDENTITY, &stem, &[sr * 1.4, sr, sr * 0.9, sr * 0.85], 6, &mut |seg, i| sc(stem_col, 0.85 + 0.05 * seg as f32 + 0.06 * (i % 2) as f32));
        let prof: Vec<(f32, f32)> = match style {
            // tall cone
            1 => vec![(sr, -0.05 * cr), (cr * 0.75, -0.1 * cr), (cr * 0.55, 0.45 * cr), (cr * 0.2, 1.1 * cr), (0.0, 1.25 * cr)],
            // bell with a hanging rim
            2 => vec![(sr, 0.1 * cr), (cr * 0.95, -0.25 * cr), (cr, 0.05 * cr), (cr * 0.75, 0.42 * cr), (0.0, 0.6 * cr)],
            // flat dome
            _ => vec![(sr, -0.02 * cr), (cr, -0.04 * cr), (cr * 0.95, 0.14 * cr), (cr * 0.6, 0.4 * cr), (0.0, 0.48 * cr)],
        };
        let m = at(top.x, top.y, top.z) * Mat4::from_rotation_y(rng.range(0.0, TAU));
        b.set_glow(p.glow.map(|g| (g, 0.4)));
        b.lathe(m, &prof[..2], 10, 0.0, &mut |_, i| sc(gill, 0.85 + 0.15 * (i % 2) as f32));
        b.set_glow(None);
        b.lathe(m, &prof[1..], 10, 0.0, &mut |seg, i| sc(mixc(p.leaf, p.hi, seg as f32 * 0.3), 0.88 + 0.12 * h2(i, seg as u32)));
        // spots on the cap
        let spot = p.glow.unwrap_or(mixc(p.leaf, [1.0, 1.0, 0.95], 0.75));
        b.set_glow(p.glow.map(|g| (g, 0.6)));
        for _ in 0..3 + rng.int(4) {
            let sa = rng.range(0.0, TAU);
            let t = rng.range(0.25, 0.7);
            // point on the profile between ring 2 and 3 (upper part of the cap)
            let (r0, y0) = prof[prof.len() - 3];
            let (r1, y1) = prof[prof.len() - 2];
            let (rr, yy) = (r0 + (r1 - r0) * t, y0 + (y1 - y0) * t);
            let q = top + Vec3::new(sa.cos() * rr, yy + 0.01 * s, sa.sin() * rr);
            let n = Vec3::new(sa.cos() * 0.6, 1.0, sa.sin() * 0.6);
            b.blob(Mat4::from_rotation_translation(up_to(n), q), Vec3::new(0.05, 0.015, 0.05) * s * f.sqrt() * 1.4, 0, 0.1, rng, &mut |_, _| spot);
        }
        b.set_glow(None);
    }
}

fn lantern(b: &mut MB, rng: &mut Rng, p: &Pal, s: f32, det: u32) {
    let g = p.glow.unwrap_or([1.0, 0.8, 0.4]);
    let stalks = 1 + rng.int(3);
    for k in 0..stalks {
        let a = rng.range(0.0, TAU);
        let base = if k == 0 { Vec3::ZERO } else { Vec3::new(a.cos(), 0.0, a.sin()) * 0.15 * s };
        let tall = if k == 0 { 1.0 } else { rng.range(0.6, 0.85) };
        // a shepherd's crook: up, then curling over so the bulb hangs
        let dir = Vec3::new(a.cos() * 0.15, 1.0, a.sin() * 0.15);
        let pts = bent(base, dir, 1.3 * s * tall, 6, Vec3::new(a.cos(), -0.1, a.sin()) * 0.28);
        let n = pts.len();
        let radii: Vec<f32> = (0..n).map(|i| (0.05 - 0.025 * i as f32 / n as f32) * s).collect();
        b.tube(Mat4::IDENTITY, &pts, &radii, 5, &mut |seg, i| bark(p, seg, i));
        let tip = pts[n - 1];
        let hang = tip - Vec3::Y * 0.16 * s;
        b.lathe(at(tip.x, tip.y - 0.06 * s, tip.z), &[(0.0, 0.08 * s), (0.11 * s, -0.02 * s), (0.08 * s, -0.04 * s)], 6, 0.3, &mut |_, _| p.leaf);
        b.set_glow(Some((g, 0.6)));
        let r = 0.17 * s * tall.sqrt();
        b.blob(at(hang.x, hang.y - r * 0.4, hang.z), Vec3::new(r * 0.85, r * 1.15, r * 0.85), det, 0.06, rng, &mut |_, x| mixc(g, [1.0, 1.0, 0.95], 0.08 + 0.15 * x));
        b.set_glow(None);
    }
    for i in 0..4 {
        let a = i as f32 / 4.0 * TAU + rng.range(0.0, 0.6);
        b.leaf(&Mat4::IDENTITY, Vec3::Y * 0.02, Vec3::new(a.cos(), 0.6, a.sin()), 0.55 * s, 0.11 * s, 1.0, 4, 0.0, &mut |k| mixc(p.lo, p.leaf, k as f32 * 0.3));
    }
}

fn coral(b: &mut MB, rng: &mut Rng, p: &Pal, s: f32, lo: bool) {
    #[allow(clippy::too_many_arguments)]
    fn branch(b: &mut MB, rng: &mut Rng, p: &Pal, from: Vec3, dir: Vec3, len: f32, r: f32, depth: u32, max: u32) {
        let pts = bent(from, dir, len, 2, Vec3::new(rng.range(-0.15, 0.15), 0.1, rng.range(-0.15, 0.15)));
        let t = depth as f32 / max as f32;
        let col = mixc(mixc(p.stem, p.leaf, 0.6), p.leaf, t);
        b.tube(Mat4::IDENTITY, &pts, &[r, r * 0.85, r * 0.7], 5, &mut |seg, i| sc(col, 0.85 + 0.1 * seg as f32 + 0.06 * (i % 2) as f32));
        let end = pts[2];
        if depth >= max {
            b.set_glow(p.glow.map(|g| (g, 0.5)));
            b.blob(at(end.x, end.y, end.z), Vec3::splat(r * 1.4), 0, 0.15, rng, &mut |_, _| p.glow.unwrap_or(p.hi));
            b.set_glow(None);
            return;
        }
        let kids = if depth == 0 { 3 } else { 2 };
        for _ in 0..kids {
            let nd = (dir + Vec3::new(rng.range(-0.9, 0.9), rng.range(0.1, 0.5), rng.range(-0.9, 0.9))).normalize();
            branch(b, rng, p, end, nd, len * 0.72, r * 0.68, depth + 1, max);
        }
    }
    let max = if lo { 2 } else { 3 };
    branch(b, rng, p, Vec3::ZERO, Vec3::Y, 0.55 * s, 0.12 * s, 0, max);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::StarSystem;
    use crate::items::ORES;

    /// Decor meshes must stay cheap: they are numerous and streamed.
    #[test]
    fn decor_meshes_have_sane_budgets() {
        for (item, _, _) in ORES {
            for v in 0..3 {
                let t = crystal(item, v).tris();
                assert!(t > 40 && t < 2500, "{item:?} has {t} triangles");
            }
        }
        let mut worst = 0;
        for seed in 0..12u64 {
            let sys = StarSystem::generate(seed * 31 + 1, 0);
            for p in &sys.planets {
                for sp in &p.flora {
                    for v in 0..3 {
                        let hi = plant(sp, v, false).tris();
                        let lo = plant(sp, v, true).tris();
                        assert!(lo <= hi, "{:?}: phone mesh bigger than desktop", sp.arch);
                        assert!(hi < 2400, "{:?} has {hi} triangles", sp.arch);
                        worst = worst.max(hi);
                    }
                }
            }
        }
        assert!(specimen().tris() < 1500 && fragment().tris() < 800);
        println!("largest plant: {worst} triangles");
    }

    /// Every vertex carries a glow lookup coordinate and plain parts point at the black texel.
    #[test]
    fn glow_lookup_round_trips() {
        let lut = crate::meshgen::glow_lut();
        for c in [[0.0, 0.0, 0.0], [1.0, 0.5, 0.25], [0.2, 1.0, 0.6]] {
            let uv = crate::meshgen::glow_uv(c);
            let x = (uv[0] * crate::meshgen::GLOW_W as f32) as usize;
            let y = (uv[1] * crate::meshgen::GLOW_H as f32) as usize;
            let i = (y * crate::meshgen::GLOW_W as usize + x) * 4;
            for k in 0..3 {
                assert!((lut[i + k] as f32 / 255.0 - c[k]).abs() < 0.02, "{c:?} -> {:?}", &lut[i..i + 3]);
            }
        }
    }
}
