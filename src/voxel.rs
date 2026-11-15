//! Voxel planet: a signed density field stored in 16^3 chunks, meshed with surface nets.
//! Positive density = solid. One voxel = one metre. The planet centre is the origin.
//!
//! The world is lazy: chunks are generated on demand (by the streamer in `terrain.rs`, or
//! synchronously when an edit touches one), and any query on a chunk that is not loaded falls
//! back to the generator, so `sample`/`raycast`/`surface_point` are always exact.
use crate::galaxy::{CaveStyle, PlanetDef};
use crate::items::{Item, ORES};
use crate::util::{any_perp, fbm, mixc, noise3, scalec, smoothstep, Rng};
use bevy::math::{IVec3, Vec3};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub const CS: i32 = 16;
pub const N: usize = (CS * CS * CS) as usize;
/// Side of the sample block a chunk mesh is built from (chunk + one voxel border each side + 1).
pub const BL: usize = 18;
pub type Block = Box<[i8; BL * BL * BL]>;

pub struct Ore {
    pub item: Item,
    /// 0..1 abundance on this planet (from the planet's resource list)
    pub rich: f32,
    pub min_depth: f32,
    pub freq: f32,
    pub seed: u32,
    /// special resources: thin near the surface, rich only deep down
    pub rare: bool,
}

// Cave layers, in metres below the smoothed ground (`base` in `TerrainGen::terrain`). The
// upper layer's centre plane wanders between these depths, so do the deep layer's.
const UP_LO: f32 = 15.0;
const UP_HI: f32 = 25.0;
const DEEP_LO: f32 = 64.0;
const DEEP_HI: f32 = 92.0;
/// Tunnel noise frequency (tunnels are the zero lines of two such fields, about 50 m apart).
const TF: f32 = 1.0 / 52.0;
/// Metres per unit of tunnel noise close to its zero line (measured: mean gradient 0.022/m).
const TG: f32 = 50.0;
/// Radius of the helix the shafts wind down on, and turns between the layers.
const HELIX_R: f32 = 7.0;
const HELIX_TURNS: f32 = 2.0;
/// No surface openings within this distance of the landing site (m).
const LAND_CLEAR: f32 = 55.0;

pub struct TerrainGen {
    pub radius: f32,
    pub amp: f32,
    pub mount: f32,
    pub terrace: f32,
    pub cave: f32,
    pub seed: u32,
    pub core: f32,
    pub top: [f32; 3],
    pub soil: [f32; 3],
    pub rock: [f32; 3],
    pub deep: [f32; 3],
    pub ores: Vec<Ore>,
    pub craters: Vec<(Vec3, f32, f32)>,
    /// no terrain reaches above this radius
    pub max_r: f32,
    /// everything below this radius is solid rock (no caves, no surface)
    pub solid_r: f32,
    pub style: CaveStyle,
    /// direction of the pod's landing site (kept free of cave openings)
    pub landing: Vec3,
}

impl TerrainGen {
    pub fn new(def: &PlanetDef) -> Self {
        let seed = (def.seed & 0xffff_ffff) as u32;
        let mut ores = Vec::new();
        for (item, min_depth, freq) in ORES {
            if let Some((_, rich)) = def.resources.iter().find(|r| r.0 == item) {
                let rare = matches!(item, Item::TitaniumOre | Item::Lithium | Item::Uranite | Item::Heliosite);
                ores.push(Ore { item, rich: *rich, min_depth, freq, seed: seed.wrapping_add(1000 + item as u32 * 77), rare });
            }
        }
        let mut craters = Vec::new();
        let mut crater_depth: f32 = 0.0;
        if def.craters > 0.0 {
            let mut rng = Rng::new(def.seed ^ 0xC4A7);
            for _ in 0..26 {
                let ang = rng.range(0.03, 0.12);
                let depth = (ang * def.radius * 0.2).min(14.0);
                crater_depth = crater_depth.max(depth);
                craters.push((rng.dir(), ang, depth));
            }
        }
        let max_r = def.radius + def.amp * (1.3 + def.mount) + 2.0;
        // the deep layer's floor is at most ~102 m below the smoothed ground
        let solid_r = def.radius - def.amp * 1.4 - crater_depth - 110.0;
        let mut style = def.kind.cave_style();
        // the old per-planet cave amount still nudges tunnel width (0.01 .. 0.02)
        style.tunnel *= 0.85 + 0.15 * (def.cave / 0.016);
        let mut g = TerrainGen {
            radius: def.radius,
            amp: def.amp,
            mount: def.mount,
            terrace: def.terrace,
            cave: def.cave,
            seed,
            core: (def.radius * 0.22).max(10.0),
            top: def.top,
            soil: def.soil,
            rock: def.rock,
            deep: def.deep,
            ores,
            craters,
            max_r,
            solid_r,
            style,
            landing: Vec3::X,
        };
        g.landing = g.pick_landing(def.seed);
        g
    }

    pub fn max_height(&self) -> f32 {
        self.max_r
    }

    /// A flat, ordinary spot for the pod, away from craters.
    fn pick_landing(&self, seed: u64) -> Vec3 {
        let mut rng = Rng::new(seed ^ 0x1A4D);
        let mut best = Vec3::X;
        let mut best_score = f32::MAX;
        for _ in 0..40 {
            let mut d = rng.dir();
            d.y *= 0.4;
            let d = d.normalize();
            let u = any_perp(d);
            let v = d.cross(u);
            let h0 = self.height(d);
            let mut score = (h0 - self.radius - self.amp * 0.2).abs() * 0.3;
            for k in 0..8 {
                let a = k as f32 / 8.0 * std::f32::consts::TAU;
                let o = (d + (u * a.cos() + v * a.sin()) * (9.0 / self.radius)).normalize();
                score += (self.height(o) - h0).abs();
            }
            for (c, ang, _) in &self.craters {
                if d.dot(*c).clamp(-1.0, 1.0).acos() < ang * 1.4 {
                    score += 5.0;
                }
            }
            if score < best_score {
                best_score = score;
                best = d;
            }
        }
        best
    }

    /// Terrain surface radius in the given direction (before caves and edits).
    pub fn height(&self, dir: Vec3) -> f32 {
        self.terrain(dir).0
    }

    /// (surface radius, smoothed ground radius). The smoothed ground leaves out hills, ridges
    /// and terraces; cave layers hang from it so that their floors stay level under bumpy land.
    pub fn terrain(&self, dir: Vec3) -> (f32, f32) {
        let p = dir * self.radius;
        let s = self.seed;
        let cont = fbm(p * (1.0 / 42.0), s, 3);
        let hills = fbm(p * (1.0 / 13.0), s.wrapping_add(11), 3);
        let mut base = cont * 0.9 * self.amp;
        let mut h = base + hills * 0.3 * self.amp;
        if self.mount > 0.0 {
            let mask = smoothstep(0.0, 0.35, cont);
            let ridge = 1.0 - noise3(p * (1.0 / 24.0), s.wrapping_add(23)).abs();
            h += ridge * ridge * mask * self.mount * self.amp * 0.9;
        }
        if self.terrace > 0.0 {
            let step = 2.5;
            let q = h / step;
            let t = (q.floor() + smoothstep(0.35, 0.65, q - q.floor())) * step;
            h += (t - h) * self.terrace;
        }
        for (c, ang, depth) in &self.craters {
            let d = dir.dot(*c);
            if d > 0.98 {
                let t = d.clamp(-1.0, 1.0).acos() / ang;
                if t < 1.0 {
                    h -= depth * (1.0 - t * t);
                    base -= depth * (1.0 - t * t);
                } else if t < 1.35 {
                    h += depth * 0.25 * (1.0 - (t - 1.0) / 0.35);
                }
            }
        }
        (self.radius + h, self.radius + base)
    }

    /// Depth below the (unedited) surface, the shared depth-zone measure:
    /// surface < 4 m, upper caves 4-40 m, deep caves below 40 m.
    pub fn depth(&self, p: Vec3) -> f32 {
        let r = p.length().max(1e-3);
        self.height(p / r) - r
    }

    pub fn density(&self, p: Vec3) -> f32 {
        let r = p.length();
        if r < 1e-3 {
            return 1.0;
        }
        let dir = p / r;
        let (h, base) = self.terrain(dir);
        let mut d = h - r;
        let cd = base - r;
        if d > -1.5 && cd > -16.0 && r > self.solid_r {
            d = d.min(self.caves(p, dir, cd));
        }
        if r < self.core + 1.0 {
            d = d.max(self.core + 1.0 - r);
        }
        d
    }

    /// 0 far from the landing site, 1 close to it.
    fn near_landing(&self, dir: Vec3) -> f32 {
        let a = dir.dot(self.landing).clamp(-1.0, 1.0).acos() * self.radius;
        1.0 - smoothstep(LAND_CLEAR, LAND_CLEAR + 30.0, a)
    }

    /// Two-octave noise whose zero lines are the tunnels (a function of direction only, so
    /// tunnels keep vertical walls and follow their layer).
    #[inline]
    fn tunnel_noise(q: Vec3, s: u32) -> f32 {
        noise3(q * TF, s) + 0.35 * noise3(q * (TF * 2.3), s ^ 0x5bd1)
    }

    /// Centre depth of the upper layer and how far a surface ramp lifts it (0..1).
    fn upper_mid(&self, q: Vec3, dir: Vec3) -> (f32, f32) {
        let s = self.seed;
        let mu0 = UP_LO + (UP_HI - UP_LO) * smoothstep(-0.45, 0.45, noise3(q * (1.0 / 110.0), s.wrapping_add(101)));
        let e0 = 0.62 - 0.2 * self.style.mouths;
        let mut ent = smoothstep(e0, e0 + 0.3, noise3(q * (1.0 / 120.0), s.wrapping_add(102)));
        if ent > 0.0 {
            ent *= 1.0 - self.near_landing(dir);
        }
        (mu0 + (0.8 - mu0) * ent, ent)
    }

    fn deep_mid(&self, q: Vec3) -> f32 {
        DEEP_LO + (DEEP_HI - DEEP_LO) * smoothstep(-0.45, 0.45, noise3(q * (1.0 / 140.0), self.seed.wrapping_add(111)))
    }

    /// Cave field at `p` (signed, roughly metres, positive = rock). `cd` is the depth below the
    /// smoothed ground.
    ///
    /// Layout: two layers of winding tunnels (the zero lines of two noise fields cross, which
    /// gives junctions) with halls where a third field is high. Tunnel junctions inside "shaft
    /// country" get a corkscrew shaft that winds down two turns to the same junction in the deep
    /// layer, whose tunnels follow one of the upper families so the shaft always lands in a
    /// tunnel. Some shafts continue up to the ground as funnel sinkholes; in rare "ramp" zones
    /// the upper layer rises to the surface so its tunnels open as trenches and hillside mouths.
    pub fn caves(&self, p: Vec3, dir: Vec3, cd: f32) -> f32 {
        let st = &self.style;
        let s = self.seed;
        let q = dir * self.radius;
        let mut d = 9.0f32;
        let (mu, ent) = self.upper_mid(q, dir);
        let mut a1: Option<f32> = None;
        let mut md: Option<f32> = None;

        // upper layer
        let v = cd - mu;
        if v > -14.0 && v < 8.0 {
            let x1 = Self::tunnel_noise(q, s.wrapping_add(103));
            let x2 = Self::tunnel_noise(q, s.wrapping_add(104));
            a1 = Some(x1);
            let tw = 2.0 * st.tunnel;
            let vn = if v > 0.0 { v / 1.9 } else { -v / 2.7 };
            for a in [x1, x2] {
                let hd = a.abs() * TG / tw;
                d = d.min(((hd * hd + vn * vn).sqrt() - 1.0) * 2.2);
            }
            let cth = 0.42 - 0.12 * st.chambers;
            let cn = noise3(q * (1.0 / 40.0), s.wrapping_add(105));
            if cn > cth {
                let kf = smoothstep(cth, cth + 0.07, cn);
                let kc = smoothstep(cth, cth + 0.32, cn);
                let floor = 3.2 * kf * st.size - 1.0;
                let ceil = (9.0 * kc * st.size * (1.0 - ent)).min(mu - 8.0) - 1.0;
                d = d.min((v - floor).max(-v - ceil));
            }
        }

        // deep layer
        if cd > 40.0 {
            let m = self.deep_mid(q);
            md = Some(m);
            let v = cd - m;
            if v > -24.0 && v < 10.0 {
                let x1 = *a1.get_or_insert_with(|| Self::tunnel_noise(q, s.wrapping_add(103)));
                let x2 = Self::tunnel_noise(q, s.wrapping_add(112));
                let tw = 2.8 * st.tunnel;
                let vn = if v > 0.0 { v / 2.4 } else { -v / 3.6 };
                for a in [x1, x2] {
                    let hd = a.abs() * TG / tw;
                    d = d.min(((hd * hd + vn * vn).sqrt() - 1.0) * 3.0);
                }
                let cth = 0.34 - 0.12 * st.chambers;
                let cn = noise3(q * (1.0 / 58.0), s.wrapping_add(113));
                if cn > cth {
                    let kf = smoothstep(cth, cth + 0.07, cn);
                    let kc = smoothstep(cth, cth + 0.32, cn);
                    let floor = 5.0 * kf * st.size - 1.0;
                    let ceil = (17.0 * kc * st.size).min(m - 46.0) - 1.0;
                    d = d.min((v - floor).max(-v - ceil));
                }
            }
        }

        // shafts between the layers (and sinkholes up to the ground)
        if cd < DEEP_HI + 5.0 {
            let sth = 0.3 - 0.18 * st.shafts;
            let mask = noise3(q * (1.0 / 95.0), s.wrapping_add(121));
            // no thin, pinched shafts: a shaft is either walkable or absent
            if mask > sth {
                let rs = 2.2 + 1.0 * smoothstep(sth, sth + 0.08, mask);
                let m = md.unwrap_or_else(|| self.deep_mid(q));
                let mut sink = smoothstep(0.2, 0.28, noise3(q * (1.0 / 150.0), s.wrapping_add(122))) * st.mouths.min(1.0);
                if sink > 0.0 {
                    sink *= 1.0 - self.near_landing(dir);
                }
                let top = mu + (-14.0 - mu) * sink;
                if cd > top - 4.0 && cd < m + 5.0 {
                    let th = if cd < mu { std::f32::consts::TAU * (cd - mu) / 22.0 } else { std::f32::consts::TAU * HELIX_TURNS * (cd - mu) / (m - mu) };
                    // tangent frame that fades out at the poles (the helix becomes a plain shaft there)
                    let t1 = dir.cross(Vec3::Y);
                    let t2 = dir.cross(t1);
                    let qq = q + (t1 * (th.cos() - 1.0) + t2 * th.sin()) * HELIX_R;
                    let x = Self::tunnel_noise(qq, s.wrapping_add(103)) * TG;
                    let y = Self::tunnel_noise(qq, s.wrapping_add(104)) * TG;
                    let cap = (top - cd).max(cd - m - 1.0).max(0.0);
                    let funnel = if cd < 8.0 { sink * ((8.0 - cd) * 0.9).min(7.0) } else { 0.0 };
                    d = d.min((x * x + y * y + cap * cap).sqrt() - rs - funnel);
                }
            }
        }

        if d.abs() < 2.5 {
            d += noise3(p * 0.17, s.wrapping_add(130)) * 0.9 * st.rough;
        }
        d
    }

    /// Quantised density of one voxel, with fast paths for open space and deep rock.
    #[inline]
    pub fn voxel(&self, x: i32, y: i32, z: i32) -> i8 {
        let p = Vec3::new(x as f32, y as f32, z as f32);
        let r2 = p.length_squared();
        if r2 > self.max_r * self.max_r {
            return -127;
        }
        if r2 < self.solid_r * self.solid_r {
            return 127;
        }
        q(self.density(p))
    }

    /// Chunks that are entirely open space or entirely deep rock never need generating.
    pub fn trivial_chunk(&self, c: IVec3) -> Option<ChunkData> {
        let centre = (c * CS).as_vec3() + Vec3::splat(7.5);
        let r = centre.length();
        if r - 14.0 > self.max_r {
            Some(ChunkData::Air)
        } else if r + 14.0 < self.solid_r {
            Some(ChunkData::Solid)
        } else {
            None
        }
    }

    /// Ore at `p` (`depth` below the surface). Veins are thin sheets along the zero surface of a
    /// low-frequency field, cut into patches by a second field, so a cut through rock shows
    /// bands that run for tens of metres and pinch out. Sheets thicken with depth, special
    /// resources are thin near the surface, and below the upper caves rich pockets appear.
    pub fn ore_at(&self, p: Vec3, depth: f32) -> Option<Item> {
        if depth < 0.4 {
            return None;
        }
        let deep = smoothstep(28.0, 70.0, depth);
        for o in &self.ores {
            if depth < o.min_depth {
                continue;
            }
            let grow = if o.rare { 0.55 + 0.85 * deep } else { 1.0 + 0.5 * deep };
            let t = (0.014 + 0.018 * o.rich) * grow;
            let sheet = noise3(p * (o.freq * 0.42), o.seed);
            if sheet.abs() < t && noise3(p * (o.freq * 0.16), o.seed ^ 0xA5A5) > 0.28 - 0.12 * o.rich - 0.1 * deep {
                return Some(o.item);
            }
            if deep > 0.0 && noise3(p * (o.freq * 1.1), o.seed ^ 0x5A5A) > 0.82 - 0.13 * deep * (0.5 + o.rich) {
                return Some(o.item);
            }
        }
        None
    }

    /// sRGB colour of the ground at `p` with surface normal `n`.
    pub fn color(&self, p: Vec3, n: Vec3) -> [f32; 3] {
        let r = p.length().max(1e-3);
        let depth = self.height(p / r) - r;
        self.color_at(p, n, depth)
    }

    /// Flat, banded colour fields: grass on flat ground, soil under it and on slopes, rock on
    /// cliffs, rock strata with depth, solid ore colours. No smooth gradients so that every
    /// facet reads as one clear colour.
    pub fn color_at(&self, p: Vec3, n: Vec3, depth: f32) -> [f32; 3] {
        let r = p.length().max(1e-3);
        let dir = p / r;
        let s = self.seed;
        let flat = n.dot(dir);
        // per-facet random number (facets are about a metre wide)
        let q = (p * 2.5).floor();
        let fh = hash_u((q.x as i32 as u32).wrapping_mul(73856093) ^ (q.y as i32 as u32).wrapping_mul(19349663) ^ (q.z as i32 as u32).wrapping_mul(83492791) ^ s);
        if let Some(item) = self.ore_at(p, depth) {
            // veins: a matrix colour, a second mineral and bright crystal faces per facet, so a
            // vein reads as its own material even in the flat light at the bottom of a pit
            let [a, b, c] = vein_palette(item);
            let col = if fh < 0.5 { a } else if fh < 0.82 { b } else { c };
            return scalec(col, 0.86 + 0.18 * flat.max(-0.5));
        }
        // rock is kept a little desaturated so that ores stay the most colourful thing in a wall
        let rock = desat(self.rock, 0.2);
        let deep = desat(self.deep, 0.1);
        if depth < 1.2 && flat > 0.62 {
            // ground cover: lush / dry / shaded fields with a ragged edge towards slopes and a
            // few bright facets (flowers, pebbles)
            let patch = noise3(p * 0.035, s.wrapping_add(51)) + 0.35 * noise3(p * 0.11, s.wrapping_add(52));
            let lush = scalec(mixc(self.top, [self.top[0] * 0.8, self.top[1], self.top[2] * 1.1], 0.5), 1.08);
            let dry = scalec(mixc(self.top, self.soil, 0.4), 1.06);
            let shade = scalec(mixc(self.top, [self.top[2], self.top[1], self.top[0]], 0.12), 0.84);
            let mut c = if patch > 0.22 { dry } else if patch < -0.35 { lush } else if patch < -0.1 && fh < 0.5 { shade } else { self.top };
            let edge = smoothstep(0.62, 0.78, flat) + 0.25 * (fh - 0.5);
            if edge < 0.5 {
                c = mixc(c, self.soil, 0.45);
            }
            if fh > 0.993 && patch < 0.1 {
                // a lone flower facet in the ground cover's complementary hue
                c = mixc([self.top[2], self.top[0], self.top[1]], self.top, 0.5);
            }
            return c;
        }
        if depth < 3.5 && flat > 0.35 {
            // soil with darker humus and lighter clay bands, the odd stone
            let w = depth + 0.8 * noise3(p * 0.15, s.wrapping_add(54));
            let c = if w < 1.6 { scalec(self.soil, 0.82) } else if w < 2.6 { self.soil } else { scalec(mixc(self.soil, rock, 0.3), 1.08) };
            return if fh > 0.93 { mixc(rock, c, 0.3) } else { c };
        }
        // rock strata: bands that follow the depth, wobbled by noise, alternating warm and cool
        let w = depth + 2.2 * noise3(p * 0.06, s.wrapping_add(53));
        let band = (w / 3.2).floor() as i32;
        let deep_t = smoothstep(6.0, 70.0, depth);
        let base = mixc(rock, deep, (deep_t * 3.0).floor() / 3.0);
        let mut c = match band.rem_euclid(5) {
            0 => base,
            1 => scalec(base, 0.84),
            2 => mixc(base, self.soil, 0.32),
            3 => scalec(mixc(base, [base[2], base[1], base[0]], 0.12), 0.93),
            _ => scalec(base, 1.08),
        };
        // cliffs near the surface: vertical weathering streaks
        if depth < 6.0 {
            let streak = noise3(Vec3::new(p.x, p.y * 0.15, p.z) * 0.5, s.wrapping_add(55));
            if streak > 0.35 {
                c = scalec(c, 0.88);
            }
        }
        if depth > 4.0 {
            // cave walls: darker and cooler, sediment on floors, dark ceilings, mineral crusts
            let cave = smoothstep(4.0, 30.0, depth);
            c = scalec(mixc(c, [c[0] * 0.85, c[1] * 0.92, c[2] * 1.08], cave), 1.0 - 0.12 * cave);
            if flat > 0.55 {
                c = mixc(c, self.soil, 0.35);
            } else if flat < -0.4 {
                c = scalec(c, 0.82);
            }
            let crust = noise3(p * 0.09, s.wrapping_add(56));
            if crust > 0.42 {
                c = mixc(c, [0.62, 0.66, 0.6], 0.25 + 0.15 * fh);
            }
        }
        scalec(c, 0.96 + 0.08 * fh)
    }
}

fn desat(c: [f32; 3], k: f32) -> [f32; 3] {
    let g = c[0] * 0.3 + c[1] * 0.59 + c[2] * 0.11;
    mixc(c, [g, g, g], k)
}

/// Matrix, second mineral and crystal-face colours of each ore as it appears in a wall.
fn vein_palette(item: Item) -> [[f32; 3]; 3] {
    match item {
        Item::IronOre => [[0.6, 0.28, 0.16], [0.3, 0.22, 0.22], [0.86, 0.48, 0.22]],
        Item::CopperOre => [[0.12, 0.55, 0.44], [0.1, 0.3, 0.3], [0.32, 0.82, 0.64]],
        Item::Quartz => [[0.88, 0.88, 0.94], [0.7, 0.71, 0.82], [1.0, 1.0, 1.0]],
        Item::Carbon => [[0.09, 0.09, 0.1], [0.17, 0.17, 0.19], [0.4, 0.43, 0.52]],
        Item::Ice => [[0.78, 0.92, 1.0], [0.5, 0.75, 0.94], [0.96, 0.99, 1.0]],
        Item::TitaniumOre => [[0.5, 0.57, 0.73], [0.34, 0.38, 0.5], [0.82, 0.86, 0.94]],
        Item::Lithium => [[0.9, 0.56, 0.78], [0.72, 0.46, 0.7], [0.99, 0.84, 0.93]],
        Item::Uranite => [[0.13, 0.15, 0.11], [0.5, 1.0, 0.3], [0.82, 0.92, 0.3]],
        Item::Heliosite => [[1.0, 0.6, 0.12], [0.82, 0.38, 0.07], [1.0, 0.9, 0.5]],
        other => {
            let c = other.def().color;
            [c, scalec(c, 0.75), scalec(c, 1.2)]
        }
    }
}

#[inline]
fn hash_u(a: u32) -> f32 {
    let mut h = a.wrapping_mul(0x9E37_79B1);
    h ^= h >> 15;
    h = h.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 13;
    (h & 0xffff) as f32 / 65535.0
}

pub enum ChunkData {
    Solid,
    Air,
    Data(Box<[i8; N]>),
}

pub enum Brush {
    Dig,
    Fill,
    Flatten { anchor: Vec3, normal: Vec3 },
}

#[derive(Default)]
pub struct EditOut {
    pub ores: Vec<(Item, f32)>,
    /// net soil volume: positive = gained by digging, negative = used for filling
    pub soil: f32,
}

pub struct MeshData {
    pub pos: Vec<[f32; 3]>,
    pub nor: Vec<[f32; 3]>,
    pub col: Vec<[f32; 4]>,
    pub idx: Vec<u32>,
}

#[derive(bevy::prelude::Resource)]
pub struct VoxelWorld {
    pub gen: Arc<TerrainGen>,
    pub chunks: HashMap<IVec3, ChunkData>,
    pub half: i32,
    /// chunks whose mesh is out of date because of an edit
    pub dirty: HashSet<IVec3>,
    /// edited chunks: never evicted, saved with the game
    pub modified: HashSet<IVec3>,
}

#[inline]
fn q(d: f32) -> i8 {
    (d.clamp(-1.0, 1.0) * 127.0) as i8
}

#[inline]
fn solid_frac(d: f32) -> f32 {
    (d * 0.5 + 0.5).clamp(0.0, 1.0)
}

#[inline]
fn vidx(x: i32, y: i32, z: i32) -> usize {
    ((x & 15) + ((y & 15) << 4) + ((z & 15) << 8)) as usize
}

pub fn gen_chunk(gen: &TerrainGen, c: IVec3) -> ChunkData {
    if let Some(t) = gen.trivial_chunk(c) {
        return t;
    }
    let mut data = Box::new([0i8; N]);
    let (mut any_s, mut any_a) = (false, false);
    let base = c * CS;
    let mut i = 0;
    for z in 0..CS {
        for y in 0..CS {
            for x in 0..CS {
                let v = gen.voxel(base.x + x, base.y + y, base.z + z);
                if v > 0 {
                    any_s = true
                } else {
                    any_a = true
                }
                data[i] = v;
                i += 1;
            }
        }
    }
    if !any_a && data.iter().all(|v| *v == 127) {
        ChunkData::Solid
    } else if !any_s && data.iter().all(|v| *v == -127) {
        ChunkData::Air
    } else {
        ChunkData::Data(data)
    }
}

impl VoxelWorld {
    /// An empty, lazily generated world.
    pub fn new(def: &PlanetDef) -> VoxelWorld {
        let gen = Arc::new(TerrainGen::new(def));
        let half = ((gen.max_height() + 14.0) / CS as f32).ceil() as i32 + 1;
        VoxelWorld { gen, chunks: HashMap::new(), half, dirty: HashSet::new(), modified: HashSet::new() }
    }

    /// Generates every missing chunk in `coords` in parallel (used while the loading screen shows).
    pub fn load_now(&mut self, coords: &[IVec3]) {
        let todo: Vec<IVec3> = coords.iter().copied().filter(|c| !self.chunks.contains_key(c)).collect();
        if todo.is_empty() {
            return;
        }
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 12);
        let gen = &self.gen;
        let results: Vec<Vec<(IVec3, ChunkData)>> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..threads)
                .map(|t| {
                    let todo = &todo;
                    s.spawn(move || todo.iter().skip(t).step_by(threads).map(|c| (*c, gen_chunk(gen, *c))).collect::<Vec<_>>())
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        for r in results {
            for (c, d) in r {
                self.chunks.insert(c, d);
            }
        }
    }

    /// True when the chunk's data is available without running the generator.
    pub fn ready(&self, c: IVec3) -> bool {
        self.chunks.contains_key(&c) || self.gen.trivial_chunk(c).is_some()
    }

    #[inline]
    pub fn get(&self, x: i32, y: i32, z: i32) -> i8 {
        match self.chunks.get(&IVec3::new(x >> 4, y >> 4, z >> 4)) {
            Some(ChunkData::Data(d)) => d[vidx(x, y, z)],
            Some(ChunkData::Solid) => 127,
            Some(ChunkData::Air) => -127,
            None => self.gen.voxel(x, y, z),
        }
    }

    fn set(&mut self, x: i32, y: i32, z: i32, v: i8) {
        let c = IVec3::new(x >> 4, y >> 4, z >> 4);
        if !self.chunks.contains_key(&c) {
            let d = gen_chunk(&self.gen, c);
            self.chunks.insert(c, d);
        }
        let Some(chunk) = self.chunks.get_mut(&c) else { return };
        let fill = match chunk {
            ChunkData::Solid => Some(127i8),
            ChunkData::Air => Some(-127i8),
            ChunkData::Data(_) => None,
        };
        if let Some(f) = fill {
            *chunk = ChunkData::Data(Box::new([f; N]));
        }
        if let ChunkData::Data(d) = chunk {
            d[vidx(x, y, z)] = v;
        }
        self.modified.insert(c);
    }

    /// Trilinear density in [-1, 1].
    pub fn sample(&self, p: Vec3) -> f32 {
        let f = p.floor();
        let (x, y, z) = (f.x as i32, f.y as i32, f.z as i32);
        let t = p - f;
        let g = |dx, dy, dz| self.get(x + dx, y + dy, z + dz) as f32;
        let l = |a: f32, b: f32, t: f32| a + (b - a) * t;
        let x00 = l(g(0, 0, 0), g(1, 0, 0), t.x);
        let x10 = l(g(0, 1, 0), g(1, 1, 0), t.x);
        let x01 = l(g(0, 0, 1), g(1, 0, 1), t.x);
        let x11 = l(g(0, 1, 1), g(1, 1, 1), t.x);
        l(l(x00, x10, t.y), l(x01, x11, t.y), t.z) / 127.0
    }

    /// Outward surface normal.
    pub fn normal(&self, p: Vec3) -> Vec3 {
        let e = 0.6;
        let g = Vec3::new(
            self.sample(p + Vec3::X * e) - self.sample(p - Vec3::X * e),
            self.sample(p + Vec3::Y * e) - self.sample(p - Vec3::Y * e),
            self.sample(p + Vec3::Z * e) - self.sample(p - Vec3::Z * e),
        );
        let n = -g;
        if n.length_squared() < 1e-8 {
            p.normalize_or_zero()
        } else {
            n.normalize()
        }
    }

    pub fn raycast(&self, o: Vec3, d: Vec3, max: f32) -> Option<Vec3> {
        let step = 0.35;
        let mut t = 0.0;
        if self.sample(o) > 0.0 {
            return Some(o);
        }
        while t < max {
            let nt = (t + step).min(max);
            if self.sample(o + d * nt) > 0.0 {
                let (mut a, mut b) = (t, nt);
                for _ in 0..6 {
                    let m = 0.5 * (a + b);
                    if self.sample(o + d * m) > 0.0 {
                        b = m
                    } else {
                        a = m
                    }
                }
                return Some(o + d * a);
            }
            t = nt;
        }
        None
    }

    /// Highest solid point along the given direction from the centre.
    pub fn surface_point(&self, dir: Vec3) -> Option<Vec3> {
        let top = self.gen.max_r + 2.0;
        self.raycast(dir * top, -dir, top - self.gen.core)
    }

    /// `rate` is the largest density change applied to a voxel by this call (density runs
    /// from -1 = air to +1 = solid), so tools work at a steady, controllable speed.
    pub fn edit(&mut self, c: Vec3, radius: f32, brush: &Brush, rate: f32) -> EditOut {
        let mut out = EditOut::default();
        let lim = self.gen.max_r + 6.0;
        let r = radius + 1.0;
        let lo = (c - Vec3::splat(r)).floor().as_ivec3();
        let hi = (c + Vec3::splat(r)).ceil().as_ivec3();
        let gen = self.gen.clone();
        let mut changed = false;
        for z in lo.z..=hi.z {
            for y in lo.y..=hi.y {
                for x in lo.x..=hi.x {
                    let p = Vec3::new(x as f32, y as f32, z as f32);
                    let dist = p.distance(c);
                    if dist > r {
                        continue;
                    }
                    let pr = p.length();
                    if pr < gen.core || pr > lim {
                        continue;
                    }
                    let s = (dist - radius).clamp(-1.0, 1.0);
                    let old_i = self.get(x, y, z);
                    let old = old_i as f32 / 127.0;
                    let target = match brush {
                        Brush::Dig => old.min(s),
                        Brush::Fill => old.max(-s),
                        Brush::Flatten { anchor, normal } => {
                            let plane = (*anchor - p).dot(*normal).clamp(-1.0, 1.0);
                            let w = (-s).clamp(0.0, 1.0);
                            old + (plane - old) * w
                        }
                    };
                    let mut new_i = q(old + (target - old).clamp(-rate, rate));
                    let tgt_i = q(target);
                    if new_i == old_i && tgt_i != old_i {
                        new_i = old_i + (tgt_i as i32 - old_i as i32).signum() as i8;
                    }
                    if new_i == old_i {
                        continue;
                    }
                    let dv = solid_frac(old) - solid_frac(new_i as f32 / 127.0);
                    if dv > 0.0 {
                        let depth = gen.height(p / pr.max(1e-3)) - pr;
                        match gen.ore_at(p, depth) {
                            Some(item) => match out.ores.iter_mut().find(|o| o.0 == item) {
                                Some(o) => o.1 += dv,
                                None => out.ores.push((item, dv)),
                            },
                            None => out.soil += dv,
                        }
                    } else {
                        out.soil += dv;
                    }
                    self.set(x, y, z, new_i);
                    changed = true;
                }
            }
        }
        if changed {
            // a chunk's mesh reads voxels from c*16-1 to c*16+17
            for cz in ((lo.z - 17) >> 4)..=((hi.z + 1) >> 4) {
                for cy in ((lo.y - 17) >> 4)..=((hi.y + 1) >> 4) {
                    for cx in ((lo.x - 17) >> 4)..=((hi.x + 1) >> 4) {
                        self.dirty.insert(IVec3::new(cx, cy, cz));
                    }
                }
            }
        }
        out
    }

    /// Copies the voxels a chunk's mesh depends on. `None` when the whole neighbourhood is
    /// uniform (nothing to mesh).
    pub fn block(&self, c: IVec3) -> Option<Block> {
        let mut uniform: Option<bool> = None;
        let mut mixed = false;
        'o: for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let k = c + IVec3::new(dx, dy, dz);
                    let solid = match self.chunks.get(&k) {
                        Some(ChunkData::Data(_)) => {
                            mixed = true;
                            break 'o;
                        }
                        Some(ChunkData::Solid) => true,
                        Some(ChunkData::Air) => false,
                        None => match self.gen.trivial_chunk(k) {
                            Some(ChunkData::Solid) => true,
                            Some(_) => false,
                            None => {
                                mixed = true;
                                break 'o;
                            }
                        },
                    };
                    match uniform {
                        None => uniform = Some(solid),
                        Some(u) if u != solid => {
                            mixed = true;
                            break 'o;
                        }
                        _ => {}
                    }
                }
            }
        }
        if !mixed {
            return None;
        }
        let mut s: Block = Box::new([0i8; BL * BL * BL]);
        let base = c * CS - IVec3::ONE;
        for dz in -1..=1i32 {
            for dy in -1..=1i32 {
                for dx in -1..=1i32 {
                    let k = c + IVec3::new(dx, dy, dz);
                    // block-space range covered by this neighbour
                    let rng = |d: i32| -> (usize, usize) {
                        match d {
                            -1 => (0, 1),
                            0 => (1, 17),
                            _ => (17, 18),
                        }
                    };
                    let (x0, x1) = rng(dx);
                    let (y0, y1) = rng(dy);
                    let (z0, z1) = rng(dz);
                    let chunk = self.chunks.get(&k);
                    for z in z0..z1 {
                        for y in y0..y1 {
                            for x in x0..x1 {
                                let (wx, wy, wz) = (base.x + x as i32, base.y + y as i32, base.z + z as i32);
                                let v = match chunk {
                                    Some(ChunkData::Data(d)) => d[vidx(wx, wy, wz)],
                                    Some(ChunkData::Solid) => 127,
                                    Some(ChunkData::Air) => -127,
                                    None => self.gen.voxel(wx, wy, wz),
                                };
                                s[x + y * BL + z * BL * BL] = v;
                            }
                        }
                    }
                }
            }
        }
        Some(s)
    }

    pub fn mesh_chunk(&self, c: IVec3) -> Option<MeshData> {
        let b = self.block(c)?;
        mesh_block(&self.gen, c, &b)
    }

    pub fn export_edits(&self) -> Vec<([i32; 3], Vec<u8>)> {
        let mut out = Vec::new();
        let mut list: Vec<&IVec3> = self.modified.iter().collect();
        list.sort_by_key(|c| (c.x, c.y, c.z));
        for c in list {
            if let Some(ChunkData::Data(d)) = self.chunks.get(c) {
                let mut rle = Vec::new();
                let mut i = 0;
                while i < N {
                    let v = d[i];
                    let mut n = 1;
                    while i + n < N && d[i + n] == v && n < 255 {
                        n += 1;
                    }
                    rle.push(n as u8);
                    rle.push(v as u8);
                    i += n;
                }
                out.push((c.to_array(), rle));
            }
        }
        out
    }

    pub fn import_edits(&mut self, edits: &[([i32; 3], Vec<u8>)]) {
        for (c, rle) in edits {
            let c = IVec3::from_array(*c);
            let mut data = Box::new([0i8; N]);
            let mut i = 0;
            for pair in rle.chunks_exact(2) {
                for _ in 0..pair[0] {
                    if i < N {
                        data[i] = pair[1] as i8;
                        i += 1;
                    }
                }
            }
            self.chunks.insert(c, ChunkData::Data(data));
            self.modified.insert(c);
            for dz in -1..=1 {
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        self.dirty.insert(c + IVec3::new(dx, dy, dz));
                    }
                }
            }
        }
    }
}

/// Surface nets over a sample block, emitted as flat-shaded triangles (every triangle has its
/// own three vertices, one normal and one colour) for a crisp, faceted look.
pub fn mesh_block(gen: &TerrainGen, c: IVec3, s: &[i8; BL * BL * BL]) -> Option<MeshData> {
    let (mut has_s, mut has_a) = (false, false);
    for v in s.iter() {
        if *v > 0 {
            has_s = true
        } else {
            has_a = true
        }
        if has_s && has_a {
            break;
        }
    }
    if !(has_s && has_a) {
        return None;
    }
    let base = c * CS - IVec3::ONE;
    const C: usize = 17;
    let mut vid = vec![u32::MAX; C * C * C];
    let mut verts: Vec<Vec3> = Vec::new();
    let at = |x: usize, y: usize, z: usize| s[x + y * BL + z * BL * BL] as f32;
    for z in 0..C {
        for y in 0..C {
            for x in 0..C {
                let v = [
                    at(x, y, z),
                    at(x + 1, y, z),
                    at(x, y + 1, z),
                    at(x + 1, y + 1, z),
                    at(x, y, z + 1),
                    at(x + 1, y, z + 1),
                    at(x, y + 1, z + 1),
                    at(x + 1, y + 1, z + 1),
                ];
                let mut mask = 0u8;
                for (i, d) in v.iter().enumerate() {
                    if *d > 0.0 {
                        mask |= 1 << i;
                    }
                }
                if mask == 0 || mask == 255 {
                    continue;
                }
                const EDGES: [(usize, usize); 12] = [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)];
                let mut sum = Vec3::ZERO;
                let mut cnt = 0.0;
                for (a, b) in EDGES {
                    if (v[a] > 0.0) != (v[b] > 0.0) {
                        let t = v[a] / (v[a] - v[b]);
                        let pa = Vec3::new((a & 1) as f32, ((a >> 1) & 1) as f32, ((a >> 2) & 1) as f32);
                        let pb = Vec3::new((b & 1) as f32, ((b >> 1) & 1) as f32, ((b >> 2) & 1) as f32);
                        sum += pa + (pb - pa) * t;
                        cnt += 1.0;
                    }
                }
                vid[x + y * C + z * C * C] = verts.len() as u32;
                verts.push(base.as_vec3() + Vec3::new(x as f32, y as f32, z as f32) + sum / cnt);
            }
        }
    }
    if verts.is_empty() {
        return None;
    }
    // terrain height over the chunk, sampled on a coarse lattice and interpolated (the colour
    // bands need the depth below the surface for every triangle)
    const HL: usize = 5;
    let mut hl = [0f32; HL * HL * HL];
    let origin = (c * CS).as_vec3() - Vec3::splat(2.0);
    let hstep = 20.0 / (HL - 1) as f32;
    for z in 0..HL {
        for y in 0..HL {
            for x in 0..HL {
                let p = origin + Vec3::new(x as f32, y as f32, z as f32) * hstep;
                hl[x + y * HL + z * HL * HL] = gen.height(p.normalize_or(Vec3::Y));
            }
        }
    }
    let height_at = |p: Vec3| -> f32 {
        let l = ((p - origin) / hstep).clamp(Vec3::ZERO, Vec3::splat((HL - 1) as f32 - 1e-3));
        let f = l.floor();
        let t = l - f;
        let (x, y, z) = (f.x as usize, f.y as usize, f.z as usize);
        let g = |dx: usize, dy: usize, dz: usize| hl[(x + dx) + (y + dy) * HL + (z + dz) * HL * HL];
        let lp = |a: f32, b: f32, t: f32| a + (b - a) * t;
        let x00 = lp(g(0, 0, 0), g(1, 0, 0), t.x);
        let x10 = lp(g(0, 1, 0), g(1, 1, 0), t.x);
        let x01 = lp(g(0, 0, 1), g(1, 0, 1), t.x);
        let x11 = lp(g(0, 1, 1), g(1, 1, 1), t.x);
        lp(lp(x00, x10, t.y), lp(x01, x11, t.y), t.z)
    };

    let mut m = MeshData { pos: Vec::new(), nor: Vec::new(), col: Vec::new(), idx: Vec::new() };
    let tri = |m: &mut MeshData, a: Vec3, b: Vec3, cc: Vec3| {
        let n = (b - a).cross(cc - a);
        let len = n.length();
        if len < 1e-6 {
            return;
        }
        let n = n / len;
        let centre = (a + b + cc) / 3.0;
        let depth = height_at(centre) - centre.length();
        let col = gen.color_at(centre, n, depth);
        // tiny per-facet brightness jitter keeps neighbouring facets apart
        let h = (centre * 4.0).floor();
        let j = 0.94 + 0.1 * hash_u((h.x as i32 as u32).wrapping_mul(73856093) ^ (h.y as i32 as u32).wrapping_mul(19349663) ^ (h.z as i32 as u32).wrapping_mul(83492791));
        let lc = [(col[0] * j).max(0.0).powf(2.2), (col[1] * j).max(0.0).powf(2.2), (col[2] * j).max(0.0).powf(2.2), 1.0];
        let i0 = m.pos.len() as u32;
        for p in [a, b, cc] {
            m.pos.push(p.to_array());
            m.nor.push(n.to_array());
            m.col.push(lc);
        }
        m.idx.extend_from_slice(&[i0, i0 + 1, i0 + 2]);
    };
    let cell = |x: usize, y: usize, z: usize| vid[x + y * C + z * C * C];
    for z in 1..=16usize {
        for y in 1..=16usize {
            for x in 1..=16usize {
                let s0 = at(x, y, z) > 0.0;
                let p = [x, y, z];
                for a in 0..3 {
                    let mut pn = p;
                    pn[a] += 1;
                    let s1 = at(pn[0], pn[1], pn[2]) > 0.0;
                    if s0 == s1 {
                        continue;
                    }
                    let u = (a + 1) % 3;
                    let w = (a + 2) % 3;
                    let mk = |du: usize, dw: usize| {
                        let mut cc = p;
                        cc[u] -= 1 - du;
                        cc[w] -= 1 - dw;
                        cell(cc[0], cc[1], cc[2])
                    };
                    let (v00, v10, v11, v01) = (mk(0, 0), mk(1, 0), mk(1, 1), mk(0, 1));
                    if v00 == u32::MAX || v10 == u32::MAX || v11 == u32::MAX || v01 == u32::MAX {
                        continue;
                    }
                    let (p00, p10, p11, p01) = (verts[v00 as usize], verts[v10 as usize], verts[v11 as usize], verts[v01 as usize]);
                    // split the quad along its shorter diagonal
                    let short = p00.distance_squared(p11) <= p10.distance_squared(p01);
                    match (s0, short) {
                        (true, true) => {
                            tri(&mut m, p00, p10, p11);
                            tri(&mut m, p00, p11, p01);
                        }
                        (true, false) => {
                            tri(&mut m, p00, p10, p01);
                            tri(&mut m, p10, p11, p01);
                        }
                        (false, true) => {
                            tri(&mut m, p00, p11, p10);
                            tri(&mut m, p00, p01, p11);
                        }
                        (false, false) => {
                            tri(&mut m, p00, p01, p10);
                            tri(&mut m, p10, p01, p11);
                        }
                    }
                }
            }
        }
    }
    if m.idx.is_empty() {
        None
    } else {
        Some(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::StarSystem;

    fn world() -> VoxelWorld {
        let sys = StarSystem::generate(5, 0);
        VoxelWorld::new(&sys.planets[sys.home])
    }

    #[test]
    fn planet_is_solid_inside_and_empty_outside() {
        let w = world();
        let r = w.gen.radius;
        for dir in [Vec3::X, Vec3::Y, -Vec3::Z, Vec3::new(1.0, 1.0, 1.0).normalize()] {
            assert!(w.sample(dir * (w.gen.core * 0.5)) > 0.0, "core must be solid");
            assert!(w.sample(dir * (w.gen.max_r + 5.0)) < 0.0, "space must be empty");
            let s = w.surface_point(dir).expect("surface");
            assert!((s.length() - r).abs() < w.gen.amp * 3.0 + 2.0);
            assert!(w.normal(s).dot(dir) > 0.2, "surface normal should point outwards");
        }
    }

    #[test]
    fn lazy_and_loaded_worlds_agree() {
        let mut w = world();
        let p = w.surface_point(Vec3::Z).unwrap();
        let c = (p / CS as f32).floor().as_ivec3();
        let probes: Vec<Vec3> = (0..40).map(|i| p + Vec3::new((i % 5) as f32 * 1.7 - 4.0, (i / 5 % 4) as f32 * 1.3 - 2.0, (i / 20) as f32 * 2.1 - 1.0)).collect();
        let lazy: Vec<f32> = probes.iter().map(|q| w.sample(*q)).collect();
        let mut coords = Vec::new();
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    coords.push(c + IVec3::new(dx, dy, dz));
                }
            }
        }
        w.load_now(&coords);
        for (q, l) in probes.iter().zip(lazy) {
            assert_eq!(w.sample(*q), l);
        }
        let a = w.mesh_chunk(c);
        let b = mesh_block(&w.gen, c, &w.block(c).unwrap());
        assert_eq!(a.map(|m| m.idx.len()), b.map(|m| m.idx.len()));
    }

    #[test]
    fn digging_removes_ground_and_survives_a_save_roundtrip() {
        let mut w = world();
        let p = w.surface_point(Vec3::X).unwrap();
        let below = p - Vec3::X * 1.5;
        assert!(w.sample(below) > 0.0);
        let mut soil = 0.0;
        for _ in 0..30 {
            let out = w.edit(below, 2.5, &Brush::Dig, 0.5);
            soil += out.soil + out.ores.iter().map(|o| o.1).sum::<f32>();
        }
        assert!(w.sample(below) < 0.0, "the brush centre must be air after digging");
        assert!(soil > 20.0, "digging a 2.5 m sphere must yield material, got {soil}");
        assert!(!w.dirty.is_empty() && !w.modified.is_empty());

        let edits = w.export_edits();
        let mut fresh = world();
        assert!(fresh.sample(below) > 0.0);
        fresh.import_edits(&edits);
        for d in [Vec3::ZERO, Vec3::Y, Vec3::Z * 2.0, -Vec3::X] {
            assert_eq!(w.sample(below + d), fresh.sample(below + d));
        }

        // filling brings the ground back and costs soil
        let mut used = 0.0;
        for _ in 0..40 {
            used += w.edit(below, 2.5, &Brush::Fill, 0.5).soil;
        }
        assert!(w.sample(below) > 0.0);
        assert!(used < -20.0);
    }

    #[test]
    fn surface_chunks_produce_closed_looking_meshes() {
        let w = world();
        let p = w.surface_point(Vec3::Y).unwrap();
        let c = IVec3::new(p.x as i32 >> 4, p.y as i32 >> 4, p.z as i32 >> 4);
        let m = w.mesh_chunk(c).expect("a chunk at the surface must have geometry");
        assert!(m.idx.len() >= 300 && m.idx.len() % 3 == 0);
        assert_eq!(m.pos.len(), m.nor.len());
        assert_eq!(m.pos.len(), m.col.len());
        // triangles must face away from the planet centre on average
        let mut outward = 0;
        for t in m.idx.chunks(3) {
            let (a, b, cc) = (Vec3::from_array(m.pos[t[0] as usize]), Vec3::from_array(m.pos[t[1] as usize]), Vec3::from_array(m.pos[t[2] as usize]));
            if (b - a).cross(cc - a).dot(a) > 0.0 {
                outward += 1;
            }
        }
        assert!(outward * 10 > m.idx.len() / 3 * 8, "winding is inverted: {outward} of {}", m.idx.len() / 3);
        // flat shading: each triangle's normal matches its geometry
        for t in m.idx.chunks(3).take(50) {
            let (a, b, cc) = (Vec3::from_array(m.pos[t[0] as usize]), Vec3::from_array(m.pos[t[1] as usize]), Vec3::from_array(m.pos[t[2] as usize]));
            let n = (b - a).cross(cc - a).normalize();
            assert!(n.dot(Vec3::from_array(m.nor[t[0] as usize])) > 0.999);
        }
        assert!(w.mesh_chunk(IVec3::new(0, 0, 0)).is_none(), "the solid core needs no mesh");
    }
}

#[cfg(test)]
mod cave_tests {
    use super::*;
    use crate::galaxy::StarSystem;

    /// Open fraction of random points (in %) between the two depths below the real surface.
    fn open_pct(g: &TerrainGen, seed: u64, lo: f32, hi: f32, n: u32) -> f32 {
        let mut rng = Rng::new(seed);
        let mut open = 0;
        for _ in 0..n {
            let dir = rng.dir();
            let p = dir * (g.height(dir) - rng.range(lo, hi));
            if g.density(p) < 0.0 {
                open += 1;
            }
        }
        open as f32 * 100.0 / n as f32
    }

    #[test]
    fn caves_exist_in_both_bands_on_every_kind() {
        let mut kinds = Vec::new();
        for seed in [1u64, 5, 9, 33] {
            let sys = StarSystem::generate(seed, 0);
            for def in &sys.planets {
                let g = TerrainGen::new(def);
                let surf = open_pct(&g, seed, 0.5, 4.0, 3000);
                let upper = open_pct(&g, seed, 8.0, 34.0, 3000);
                let deep = open_pct(&g, seed, 50.0, 105.0, 3000);
                let between = open_pct(&g, seed, 41.0, 44.0, 3000);
                println!("seed {seed} {:?}: surface {surf:.1}% upper {upper:.1}% between {between:.1}% deep {deep:.1}%", def.kind);
                assert!(upper > 3.0, "{:?}: upper layer too sparse ({upper}%)", def.kind);
                assert!(deep > 3.0, "{:?}: deep layer too sparse ({deep}%)", def.kind);
                assert!(surf < 5.0, "{:?}: the surface is riddled with holes ({surf}%)", def.kind);
                assert!(between < upper * 0.6 && between < deep * 0.6, "{:?}: the layers should be separated by rock", def.kind);
                assert!(g.density(Vec3::X * g.core * 0.9) > 0.0 && g.density(Vec3::Y * (g.solid_r - 2.0)) > 0.0, "the core stays solid");
                kinds.push(def.kind);
            }
        }
        kinds.sort_by_key(|k| *k as u8);
        kinds.dedup();
        assert_eq!(kinds.len(), 7, "every planet kind was checked");
    }

    #[test]
    fn landing_site_is_open_surface() {
        for seed in 0..12u64 {
            let sys = StarSystem::generate(seed, seed as u32 % 3);
            for def in &sys.planets {
                let g = TerrainGen::new(def);
                let up = g.landing;
                let u = any_perp(up);
                let w = up.cross(u);
                for k in 0..120 {
                    let a = k as f32 * 2.39996;
                    let rad = (k as f32 / 120.0).sqrt() * 40.0;
                    let dir = (up * g.radius + (u * a.cos() + w * a.sin()) * rad).normalize();
                    let h = g.height(dir);
                    assert!(g.density(dir * (h + 1.0)) < 0.0, "air above the landing site");
                    for dd in [0.6f32, 2.0, 4.0, 6.0] {
                        assert!(g.density(dir * (h - dd)) > 0.0, "seed {seed} {:?}: opening {rad:.0} m from the landing site, {dd} m deep", def.kind);
                    }
                }
            }
        }
    }

    /// From inside a shaft between the layers, open space leads up into the upper layer and
    /// down into the deep layer.
    #[test]
    fn shafts_link_the_layers() {
        let mut linked = 0;
        for seed in [5u64, 9] {
            let sys = StarSystem::generate(seed, 0);
            let def = &sys.planets[sys.home];
            let g = TerrainGen::new(def);
            let mut rng = Rng::new(seed ^ 77);
            let mut tried = 0;
            for _ in 0..40000 {
                if tried == 3 {
                    break;
                }
                let dir = rng.dir();
                let (_, base) = g.terrain(dir);
                let start = dir * (base - 43.0);
                if g.density(start) > -0.3 {
                    continue;
                }
                tried += 1;
                // flood fill on a 1 m lattice, kept within 45 m of the shaft
                let s0 = start.round().as_ivec3();
                let mut seen = HashSet::new();
                let mut todo = vec![s0];
                seen.insert(s0);
                let (mut top, mut bottom) = (false, false);
                while let Some(c) = todo.pop() {
                    let p = c.as_vec3();
                    let cd = g.terrain(p.normalize()).1 - p.length();
                    top |= cd < 30.0;
                    bottom |= cd > 58.0;
                    if (top && bottom) || seen.len() > 400_000 {
                        break;
                    }
                    for d in [IVec3::X, -IVec3::X, IVec3::Y, -IVec3::Y, IVec3::Z, -IVec3::Z] {
                        let n = c + d;
                        if (n.as_vec3() - start).reject_from(dir).length() < 45.0 && !seen.contains(&n) && g.density(n.as_vec3()) < 0.0 {
                            seen.insert(n);
                            todo.push(n);
                        }
                    }
                }
                println!("seed {seed}: shaft at {start:?} reaches upper {top} deep {bottom} ({} cells)", seen.len());
                if top && bottom {
                    linked += 1;
                }
            }
        }
        assert!(linked >= 5, "shafts must connect the two layers, {linked} of 6 did");
    }

    #[test]
    fn ore_gets_richer_with_depth() {
        let sys = StarSystem::generate(5, 0);
        for def in &sys.planets {
            let g = TerrainGen::new(def);
            let mut rng = Rng::new(3);
            let (mut shallow, mut deep) = (0, 0);
            for _ in 0..8000 {
                let dir = rng.dir();
                let h = g.height(dir);
                let a = rng.range(1.0, 12.0);
                let b = rng.range(55.0, 105.0);
                shallow += g.ore_at(dir * (h - a), a).is_some() as u32;
                deep += g.ore_at(dir * (h - b), b).is_some() as u32;
            }
            assert!(shallow > 160, "{:?}: veins must reach close to the surface ({shallow})", def.kind);
            assert!(deep as f32 > shallow as f32 * 1.6, "{:?}: deep rock must be richer ({deep} vs {shallow})", def.kind);
        }
        // special resources only show up thin near the surface, richly deep down
        let def = sys.planets.iter().find(|p| p.resources.iter().any(|r| r.0 == Item::Uranite)).unwrap();
        let g = TerrainGen::new(def);
        assert!(g.ores.iter().any(|o| o.item == Item::Uranite && o.rare));
    }

    #[test]
    fn chunk_generation_is_fast_enough() {
        let sys = StarSystem::generate(5, 0);
        let g = TerrainGen::new(&sys.planets[sys.home]);
        let t = std::time::Instant::now();
        let mut n = 0;
        for k in 0..8 {
            let dir = Rng::new(k).dir();
            let h = g.height(dir);
            for dd in [0.0f32, 16.0, 32.0, 64.0, 80.0] {
                let _ = gen_chunk(&g, (dir * (h - dd) / CS as f32).floor().as_ivec3());
                n += 1;
            }
        }
        let ms = t.elapsed().as_secs_f32() * 1000.0 / n as f32;
        println!("gen_chunk: {ms:.2} ms per chunk");
        // generous: the test profile is barely optimised and machines differ
        assert!(ms < 25.0, "chunk generation got slow: {ms} ms");
    }
}
