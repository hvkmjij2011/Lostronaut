//! Planet decor (plants, crystals, samples, black box fragments, scrap, cave growths, wreck
//! sites) streamed in and out around the player. Every harvestable decor item has a stable id
//! derived from where it was generated, so harvested items (stored in `PlanetSave::removed`)
//! stay gone however often their area is reloaded.
//!
//! Id spaces: surface cells `cell * PER_CELL + k` (below `CAVE_ID`), cave chunks
//! `CAVE_ID | chunk << 4 | k`, hand-placed items from `FORCED_ID` on.
use crate::galaxy::{PlanetDef, PlanetKind};
use crate::items::Item;
use crate::meshgen;
use crate::player::Player;
use crate::props;
use crate::state::Game;
use crate::util::{any_perp, noise3, smoothstep, Rng};
use crate::voxel::{ChunkData, VoxelWorld, CS};
use crate::world::{mobile, offset_on_surface, DecorEnt, DecorKind, Gfx, PlanetRt, PlanetScoped};
use bevy::prelude::*;
use std::collections::HashSet;
use std::f32::consts::TAU;

/// Ids of decor placed by hand (fragments, starter deposits, wreck-site scrap).
pub const FORCED_ID: u32 = 0x8000_0000;
/// Ids of decor generated per cave chunk.
pub const CAVE_ID: u32 = 0x4000_0000;
/// Stream keys (`DecorCell`) of wreck sites.
const WRECK_CELL: u32 = 0x2000_0000;
const PER_CELL: u32 = 32;
/// Surface decor slots per cell; slots above this hold the satellites of crystal clusters.
const SLOTS: u32 = 8;
const CAVE_TRIES: u32 = 10;

#[derive(Component)]
pub struct DecorCell(pub u32);

/// A piece of a wreck site (not harvestable; a landmark).
#[derive(Component)]
pub struct WreckEnt(pub u32);

pub struct Decor {
    pub id: u32,
    pub kind: DecorKind,
    pub pos: Vec3,
    /// the model's +Y (the ground normal for cave decor, else the planet's up)
    pub up: Vec3,
    pub yaw: f32,
    pub scale: f32,
    /// carries a small point light (some cave growths)
    pub light: bool,
}

impl Decor {
    fn new(id: u32, kind: DecorKind, pos: Vec3, yaw: f32, scale: f32) -> Decor {
        Decor { id, kind, pos, up: pos.normalize(), yaw, scale, light: false }
    }
}

pub struct WreckPiece {
    pub kind: u8,
    pub pos: Vec3,
    pub rot: Quat,
    pub scale: f32,
}

pub struct WreckSite {
    pub centre: Vec3,
    pub pieces: Vec<WreckPiece>,
}

#[derive(Resource)]
pub struct DecorStream {
    g: u32,
    centres: Vec<Vec3>,
    live: HashSet<u32>,
    forced: Vec<Decor>,
    forced_live: HashSet<u32>,
    plant_meshes: Vec<Vec<Handle<Mesh>>>,
    scrap_meshes: Vec<Handle<Mesh>>,
    glow_meshes: Vec<Handle<Mesh>>,
    glow_color: Color,
    wreck_meshes: Vec<Handle<Mesh>>,
    /// wreck sites of this planet (story hooks can read the centres)
    pub wrecks: Vec<WreckSite>,
    wreck_live: Vec<bool>,
    cave_live: HashSet<IVec3>,
    landing: Vec3,
    pad: Vec3,
    radius: f32,
    cave_r: f32,
    wreck_r: f32,
    frame: u32,
}

fn cube_dir(face: u32, u: f32, v: f32) -> Vec3 {
    let (a, b) = ((u * std::f32::consts::FRAC_PI_4).tan(), (v * std::f32::consts::FRAC_PI_4).tan());
    let p = match face {
        0 => Vec3::new(1.0, b, -a),
        1 => Vec3::new(-1.0, b, a),
        2 => Vec3::new(a, 1.0, -b),
        3 => Vec3::new(a, -1.0, b),
        4 => Vec3::new(a, b, 1.0),
        _ => Vec3::new(-a, b, -1.0),
    };
    p.normalize()
}

/// Cells per cube face edge, about 14 m wide each.
pub fn cells_per_edge(radius: f32) -> u32 {
    ((radius * std::f32::consts::FRAC_PI_2 / 14.0).round() as u32).clamp(4, 64)
}

/// The ore of a vein within a few metres under `pos` (on the ground or a cave wall with
/// outward normal `n`), if any.
pub fn vein_below(vw: &VoxelWorld, pos: Vec3, n: Vec3) -> Option<Item> {
    for dd in [1.0f32, 2.5, 4.5, 7.0] {
        let p = pos - n * dd;
        if let Some(item) = vw.gen.ore_at(p, vw.gen.depth(p)) {
            return Some(item);
        }
    }
    None
}

/// The decor of one surface cell. Pure apart from reading the voxel world for the exact
/// surface, so the same cell always yields the same items with the same ids. Ore crystals
/// grow only where a vein runs within a few metres below, in clusters.
pub fn cell_decor(def: &PlanetDef, vw: &VoxelWorld, g: u32, cell: u32, landing: Vec3, pad: Vec3) -> Vec<Decor> {
    let face = cell / (g * g);
    let j = (cell / g) % g;
    let i = cell % g;
    let mut rng = Rng::new(def.seed ^ 0xDEC0 ^ (cell as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let side = def.radius * std::f32::consts::FRAC_PI_2 / g as f32;
    let expect = side * side * 0.022;
    let n = ((expect + rng.f32()) as u32).min(SLOTS);
    let seed32 = (def.seed >> 8) as u32;
    let mut out = Vec::new();
    let in_cell = |rng: &mut Rng| {
        let u = (i as f32 + rng.f32()) / g as f32 * 2.0 - 1.0;
        let v = (j as f32 + rng.f32()) / g as f32 * 2.0 - 1.0;
        cube_dir(face, u, v)
    };
    let ground = |dir: Vec3| -> Option<Vec3> {
        let h = vw.gen.height(dir);
        let pos = vw.raycast(dir * (h + 5.0), -dir, 12.0)?;
        if pos.distance(landing) < 9.0 || pos.distance(pad) < 6.0 || vw.normal(pos).dot(dir) < 0.72 {
            return None;
        }
        Some(pos)
    };
    for k in 0..n {
        let dir = in_cell(&mut rng);
        let roll = rng.f32();
        let r2 = rng.f32();
        let r3 = rng.f32();
        let yaw = rng.range(0.0, TAU);
        let scale = rng.range(0.8, 1.25);
        let id = cell * PER_CELL + k;
        if roll < 0.035 {
            if let Some(pos) = ground(dir) {
                out.push(Decor::new(id, DecorKind::Sample, pos, yaw, scale));
            }
        } else if roll < 0.06 {
            if let Some(pos) = ground(dir) {
                out.push(Decor::new(id, DecorKind::Scrap, pos, yaw, scale));
            }
        } else if roll < 0.24 {
            // look for an outcrop: a few spots in the cell, the first with a vein under it
            for t in 0..4 {
                let d = if t == 0 { dir } else { in_cell(&mut rng) };
                let Some(pos) = ground(d) else { continue };
                let Some(item) = vein_below(vw, pos, d) else { continue };
                out.push(Decor::new(id, DecorKind::Crystal(item), pos, yaw, scale));
                // satellites around the main crystal
                let u = any_perp(d);
                let w = d.cross(u);
                for s in 0..(1 + (r2 * 2.99) as u32) {
                    let a = yaw + s as f32 * 2.3;
                    let o = (pos + (u * a.cos() + w * a.sin()) * rng.range(1.1, 1.9)).normalize();
                    if let Some(p2) = ground(o).filter(|p2| p2.distance(pos) < 2.4) {
                        out.push(Decor::new(cell * PER_CELL + SLOTS + k * 3 + s, DecorKind::Crystal(item), p2, rng.range(0.0, TAU), scale * rng.range(0.5, 0.75)));
                    }
                }
                break;
            }
        } else {
            if def.flora.is_empty() {
                continue;
            }
            let biome = noise3(dir * def.radius * 0.035, seed32);
            if r3 > def.flora_density * (0.35 + 0.9 * smoothstep(-0.25, 0.3, biome)) {
                continue;
            }
            // species cluster by a second noise so that regions have a dominant plant
            let zone = noise3(dir * def.radius * 0.02, seed32 ^ 0x55) * 0.5 + 0.5;
            let idx = ((zone * 0.6 + r2 * 0.4) * def.flora.len() as f32) as usize;
            if let Some(pos) = ground(dir) {
                out.push(Decor::new(id, DecorKind::Plant(idx.min(def.flora.len() - 1) as u8), pos, yaw, scale));
            }
        }
    }
    out
}

fn chunk_key(c: IVec3) -> u32 {
    (c.x & 127) as u32 | ((c.y & 127) as u32) << 7 | ((c.z & 127) as u32) << 14
}

/// Decor in the caves of one chunk: light-giving growths on floors and ceilings, ore crystals
/// where a vein meets the cave wall, now and then a piece of scrap that fell in.
pub fn cave_decor(def: &PlanetDef, vw: &VoxelWorld, c: IVec3) -> Vec<Decor> {
    let mut out = Vec::new();
    let key = chunk_key(c);
    let mut rng = Rng::new(def.seed ^ 0xCA7E ^ (key as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let base = (c * CS).as_vec3();
    let mut lit = false;
    for k in 0..CAVE_TRIES {
        let p = base + Vec3::new(rng.f32(), rng.f32(), rng.f32()) * CS as f32;
        let (r1, r2, yaw) = (rng.f32(), rng.f32(), rng.range(0.0, TAU));
        let up = p.normalize();
        let ray = if r1 < 0.6 {
            -up
        } else if r1 < 0.85 {
            up
        } else {
            let t = any_perp(up);
            Quat::from_axis_angle(up, r2 * TAU) * t
        };
        if vw.gen.depth(p) < 6.0 || vw.sample(p) > -0.3 {
            continue;
        }
        let Some(hit) = vw.raycast(p, ray, 9.0) else { continue };
        if vw.gen.depth(hit) < 5.0 {
            continue;
        }
        let n = vw.normal(hit);
        let id = CAVE_ID | key << 4 | k;
        let mut d = if let (Some(item), true) = (vein_below(vw, hit, n), r2 < 0.75) {
            Decor::new(id, DecorKind::Crystal(item), hit, yaw, rng.range(0.7, 1.1))
        } else if r2 < 0.5 {
            Decor::new(id, DecorKind::Glow((r2 * 6.0) as u8 % 3), hit, yaw, rng.range(0.8, 1.4))
        } else if n.dot(up) > 0.6 && r2 > 0.92 {
            Decor::new(id, DecorKind::Scrap, hit, yaw, 1.0)
        } else {
            continue;
        };
        d.up = n;
        if matches!(d.kind, DecorKind::Glow(_)) && !lit {
            d.light = true;
            lit = true;
        }
        out.push(d);
    }
    out
}

/// A few large wreck sites per planet. The first lies within sight of the pod; the others are
/// spread over the planet. Every site sits on open, fairly flat ground.
pub fn wreck_sites(def: &PlanetDef, vw: &VoxelWorld, landing: Vec3) -> Vec<WreckSite> {
    let mut rng = Rng::new(def.seed ^ 0x3EC4);
    let count = 3 + rng.int(2) as usize;
    let gen = &vw.gen;
    let ok = |p: Vec3| -> bool {
        let up = p.normalize();
        if gen.depth(p).abs() > 1.2 || vw.normal(p).dot(up) < 0.8 {
            return false;
        }
        // no cave opening under or next to the site
        let u = any_perp(up);
        let w = up.cross(u);
        [Vec3::ZERO, u * 9.0, -u * 9.0, w * 9.0, -w * 9.0].iter().all(|o| gen.density(p + *o - up * 3.0) > 0.0)
    };
    let mut centres: Vec<Vec3> = Vec::new();
    for attempt in 0..400 {
        if centres.len() == count {
            break;
        }
        let p = if centres.is_empty() && attempt < 60 {
            offset_on_surface(vw, landing, rng.range(62.0, 85.0), rng.range(0.0, TAU))
        } else {
            let d = rng.dir();
            vw.surface_point(d).unwrap_or(d * def.radius)
        };
        let far_enough = centres.iter().all(|c| c.normalize().angle_between(p.normalize()) * def.radius > 160.0);
        let clear_of_pod = centres.is_empty() || p.normalize().angle_between(landing.normalize()) * def.radius > 140.0;
        if far_enough && clear_of_pod && ok(p) {
            centres.push(p);
        }
    }
    centres
        .into_iter()
        .map(|c| {
            let up = c.normalize();
            let t = Quat::from_axis_angle(up, rng.range(0.0, TAU)) * any_perp(up);
            let side = up.cross(t);
            // debris strewn along the skid line: bell at the head, plates at the tail
            let layout = [
                (props::W_BELL, 9.0, 1.5),
                (props::W_TANK, 2.5, -2.5),
                (props::W_RIB, -1.5, 4.0),
                (props::W_PLATE_A, -5.0, -1.0),
                (props::W_PLATE_B, -11.0, 2.5),
            ];
            let pieces = layout
                .iter()
                .map(|(kind, along, across)| {
                    let target = c + t * (*along + rng.range(-1.0, 1.0)) + side * (*across + rng.range(-1.0, 1.0));
                    let pos = vw.surface_point(target.normalize()).unwrap_or(target);
                    let n = vw.normal(pos).lerp(pos.normalize(), 0.5).normalize();
                    let rot = Quat::from_rotation_arc(Vec3::Y, n) * Quat::from_rotation_y(rng.range(0.0, TAU)) * Quat::from_rotation_x(rng.range(-0.12, 0.12));
                    WreckPiece { kind: *kind, pos: pos - n * rng.range(0.15, 0.45), rot, scale: rng.range(0.9, 1.15) }
                })
                .collect();
            WreckSite { centre: c, pieces }
        })
        .collect()
}

/// Black box fragments (in or near the wreck sites), the scrap strewn around the sites, and the
/// guaranteed starter deposits near the landing site.
pub fn forced_decor(def: &PlanetDef, vw: &VoxelWorld, landing: Vec3, home: bool, wrecks: &[WreckSite]) -> Vec<Decor> {
    let mut rng = Rng::new(def.seed ^ 0xF0C5);
    let mut id = FORCED_ID;
    let near_site = |rng: &mut Rng, k: usize, lo: f32, hi: f32| -> Option<Vec3> {
        let s = wrecks.get(k)?;
        Some(offset_on_surface(vw, s.centre, rng.range(lo, hi), rng.range(0.0, TAU)))
    };
    let mut out = Vec::new();
    // fragments: the first close to the pod, the rest in the wreck sites
    let frag_far = [26.0, 48.0, def.radius * 0.6, def.radius * 1.3];
    for (k, dist) in frag_far.iter().enumerate() {
        let ang = rng.range(0.0, TAU);
        let pos = if k == 0 { None } else { near_site(&mut rng, k - 1, 2.0, 6.0) }.unwrap_or_else(|| offset_on_surface(vw, landing, *dist, ang));
        out.push(Decor::new(id, DecorKind::Fragment, pos, rng.range(0.0, TAU), 1.1));
        id += 1;
    }
    let mut forced: Vec<(DecorKind, f32)> = Vec::new();
    if home {
        for (k, d) in [
            (Item::IronOre, 13.0), (Item::IronOre, 19.0), (Item::IronOre, 27.0), (Item::CopperOre, 15.0), (Item::CopperOre, 24.0),
            (Item::Quartz, 17.0), (Item::Quartz, 29.0), (Item::Carbon, 22.0), (Item::Carbon, 33.0),
        ] {
            forced.push((DecorKind::Crystal(k), d));
        }
        forced.push((DecorKind::Sample, 16.0));
        forced.push((DecorKind::Sample, 30.0));
        forced.push((DecorKind::Sample, 41.0));
    } else {
        for (item, _) in def.resources.iter().filter(|r| matches!(r.0, Item::TitaniumOre | Item::Lithium | Item::Uranite | Item::Heliosite)) {
            forced.push((DecorKind::Crystal(*item), 18.0));
            forced.push((DecorKind::Crystal(*item), 34.0));
        }
    }
    for (kind, dist) in forced {
        let pos = offset_on_surface(vw, landing, dist, rng.range(0.0, TAU));
        out.push(Decor::new(id, kind, pos, rng.range(0.0, TAU), 1.1));
        id += 1;
    }
    // scrap: a short trail by the first fragment and a field around every wreck site
    let first = out[0].pos;
    for _ in 0..3 {
        let pos = offset_on_surface(vw, first, rng.range(1.5, 6.0), rng.range(0.0, TAU));
        out.push(Decor::new(id, DecorKind::Scrap, pos, rng.range(0.0, TAU), rng.range(0.8, 1.1)));
        id += 1;
    }
    for k in 0..wrecks.len() {
        for _ in 0..8 {
            if let Some(pos) = near_site(&mut rng, k, 4.0, 18.0) {
                out.push(Decor::new(id, DecorKind::Scrap, pos, rng.range(0.0, TAU), rng.range(0.8, 1.2)));
            }
            id += 1;
        }
    }
    out
}

fn glow_color(kind: PlanetKind) -> [f32; 3] {
    match kind {
        PlanetKind::Temperate => [0.35, 1.0, 0.8],
        PlanetKind::Desert => [1.0, 0.75, 0.35],
        PlanetKind::Ice => [0.45, 0.75, 1.0],
        PlanetKind::Volcanic => [1.0, 0.45, 0.15],
        PlanetKind::Barren => [0.7, 0.85, 1.0],
        PlanetKind::Toxic => [0.75, 1.0, 0.25],
        PlanetKind::Crystal => [0.85, 0.45, 1.0],
    }
}

impl DecorStream {
    pub fn new(def: &PlanetDef, vw: &VoxelWorld, landing: Vec3, home: bool, meshes: &mut Assets<Mesh>) -> DecorStream {
        let g = cells_per_edge(def.radius);
        let mut centres = Vec::with_capacity((6 * g * g) as usize);
        for face in 0..6 {
            for j in 0..g {
                for i in 0..g {
                    centres.push(cube_dir(face, (i as f32 + 0.5) / g as f32 * 2.0 - 1.0, (j as f32 + 0.5) / g as f32 * 2.0 - 1.0));
                }
            }
        }
        let plant_meshes = def.flora.iter().map(|sp| (0..3).map(|v| meshes.add(meshgen::plant_mesh(sp, v))).collect()).collect();
        let gc = glow_color(def.kind);
        let wrecks = wreck_sites(def, vw, landing);
        eprintln!("[decor] {} wreck sites at {:?} m from the pod", wrecks.len(), wrecks.iter().map(|w| (w.centre.normalize().angle_between(landing.normalize()) * def.radius) as i32).collect::<Vec<_>>());
        DecorStream {
            g,
            centres,
            live: HashSet::new(),
            forced: forced_decor(def, vw, landing, home, &wrecks),
            forced_live: HashSet::new(),
            plant_meshes,
            scrap_meshes: (0..3).map(|v| meshes.add(props::scrap_mesh(v))).collect(),
            glow_meshes: (0..3).map(|v| meshes.add(props::glow_mesh(gc, v))).collect(),
            glow_color: Color::srgb(gc[0], gc[1], gc[2]),
            wreck_meshes: props::wreck_meshes().into_iter().map(|m| meshes.add(m)).collect(),
            wreck_live: vec![false; wrecks.len()],
            wrecks,
            cave_live: HashSet::new(),
            landing,
            pad: offset_on_surface(vw, landing, 11.0, 0.0),
            radius: if mobile() { 75.0 } else { 110.0 },
            cave_r: if mobile() { 30.0 } else { 44.0 },
            wreck_r: if mobile() { 260.0 } else { 420.0 },
            frame: 0,
        }
    }

    pub fn wreck_centres(&self) -> Vec<Vec3> {
        self.wrecks.iter().map(|w| w.centre).collect()
    }
}

pub(crate) fn spawn_decor(commands: &mut Commands, gfx: &Gfx, ds: &DecorStream, def: &PlanetDef, d: &Decor, cell: u32) {
    let dup = d.up;
    let variant = (d.id % 3) as usize;
    let (mesh, mat, radius, sink) = match d.kind {
        DecorKind::Plant(s) => (ds.plant_meshes[s as usize][variant].clone(), gfx.flora.clone(), 0.7 * def.flora[s as usize].size, 0.12),
        DecorKind::Crystal(item) => (gfx.crystal[&item][variant].clone(), gfx.shiny.clone(), 0.8, 0.15),
        DecorKind::Sample => (gfx.sample.clone(), gfx.shiny.clone(), 0.6, 0.1),
        DecorKind::Fragment => (gfx.fragment.clone(), gfx.flora.clone(), 0.9, 0.05),
        DecorKind::Scrap => (ds.scrap_meshes[variant].clone(), gfx.vc.clone(), 0.7, 0.06),
        DecorKind::Glow(v) => (ds.glow_meshes[v as usize % 3].clone(), gfx.glow.clone(), 0.6, 0.04),
    };
    let rot = Quat::from_rotation_arc(Vec3::Y, dup) * Quat::from_rotation_y(d.yaw);
    let mut e = commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(mat),
        Transform { translation: d.pos - dup * sink, rotation: rot, scale: Vec3::splat(d.scale) },
        DecorEnt { id: d.id, kind: d.kind, radius },
        DecorCell(cell),
        PlanetScoped,
    ));
    if d.light && !mobile() {
        e.with_children(|p| {
            p.spawn((PointLight { color: ds.glow_color, intensity: 60_000.0, range: 11.0, shadows_enabled: false, ..default() }, Transform::from_xyz(0.0, 0.8, 0.0)));
        });
    }
}

#[allow(clippy::too_many_arguments)]
pub fn stream_decor(
    mut commands: Commands,
    vw: Res<VoxelWorld>,
    rt: Res<PlanetRt>,
    game: Res<Game>,
    gfx: Res<Gfx>,
    mut ds: ResMut<DecorStream>,
    player: Query<&Transform, With<Player>>,
    ents: Query<(Entity, &DecorCell)>,
) {
    let ds = &mut *ds;
    ds.frame = ds.frame.wrapping_add(1);
    if ds.frame % 6 != 0 {
        return;
    }
    let Ok(ptf) = player.single() else { return };
    let focus = ptf.translation;
    let removed: HashSet<u32> = game.planets[game.planet].removed.iter().copied().collect();
    let def = &rt.def;
    let r = def.radius;
    let loaded = |p: Vec3| vw.chunks.contains_key(&(p / CS as f32).floor().as_ivec3());
    let mut drop: HashSet<u32> = HashSet::new();
    let mut budget = 8;
    for ci in 0..ds.centres.len() {
        let c = ds.centres[ci];
        let ci = ci as u32;
        let d = (c * r).distance(focus);
        if ds.live.contains(&ci) {
            if d > ds.radius + 30.0 {
                drop.insert(ci);
            }
        } else if d < ds.radius && budget > 0 && loaded(c * vw.gen.height(c)) {
            budget -= 1;
            for item in cell_decor(def, &vw, ds.g, ci, ds.landing, ds.pad) {
                if !removed.contains(&item.id) {
                    spawn_decor(&mut commands, &gfx, ds, def, &item, ci);
                }
            }
            ds.live.insert(ci);
        }
    }
    for k in 0..ds.forced.len() {
        let cell = FORCED_ID + k as u32;
        let d = ds.forced[k].pos.distance(focus);
        if ds.forced_live.contains(&cell) {
            if d > ds.radius + 30.0 {
                drop.insert(cell);
                ds.forced_live.remove(&cell);
            }
        } else if d < ds.radius {
            if !removed.contains(&ds.forced[k].id) {
                spawn_decor(&mut commands, &gfx, ds, def, &ds.forced[k], cell);
            }
            ds.forced_live.insert(cell);
        }
    }

    // caves: chunks around the player that hold both rock and open space
    let fc = (focus / CS as f32).floor().as_ivec3();
    let k = (ds.cave_r / CS as f32).ceil() as i32;
    let mut cave_budget = 3;
    for z in -k..=k {
        for y in -k..=k {
            for x in -k..=k {
                let c = fc + IVec3::new(x, y, z);
                if cave_budget == 0 || ds.cave_live.contains(&c) {
                    continue;
                }
                let centre = (c * CS).as_vec3() + Vec3::splat(8.0);
                if centre.distance(focus) > ds.cave_r || !matches!(vw.chunks.get(&c), Some(ChunkData::Data(_))) || vw.gen.depth(centre) < -4.0 {
                    continue;
                }
                cave_budget -= 1;
                for item in cave_decor(def, &vw, c) {
                    if !removed.contains(&item.id) {
                        spawn_decor(&mut commands, &gfx, ds, def, &item, CAVE_ID | chunk_key(c));
                    }
                }
                ds.cave_live.insert(c);
            }
        }
    }
    let cave_r = ds.cave_r;
    ds.cave_live.retain(|c| {
        let keep = ((*c * CS).as_vec3() + Vec3::splat(8.0)).distance(focus) < cave_r + 20.0;
        if !keep {
            drop.insert(CAVE_ID | chunk_key(*c));
        }
        keep
    });

    // wreck sites: big landmarks, streamed with a long radius
    for k in 0..ds.wrecks.len() {
        let d = ds.wrecks[k].centre.distance(focus);
        let cell = WRECK_CELL + k as u32;
        if ds.wreck_live[k] && d > ds.wreck_r + 40.0 {
            drop.insert(cell);
            ds.wreck_live[k] = false;
        } else if !ds.wreck_live[k] && d < ds.wreck_r {
            for p in &ds.wrecks[k].pieces {
                commands.spawn((
                    Mesh3d(ds.wreck_meshes[p.kind as usize].clone()),
                    MeshMaterial3d(gfx.vc.clone()),
                    Transform { translation: p.pos, rotation: p.rot, scale: Vec3::splat(p.scale) },
                    WreckEnt(k as u32),
                    DecorCell(cell),
                    PlanetScoped,
                ));
            }
            ds.wreck_live[k] = true;
        }
    }

    if !drop.is_empty() {
        for c in &drop {
            ds.live.remove(c);
        }
        for (e, cell) in &ents {
            if drop.contains(&cell.0) {
                commands.entity(e).despawn();
            }
        }
    }
}

/// Developer switch `LOSTRANOUT_GOTO=upper|deep|entrance|wreck|vein`: where to put the player
/// (and what to look at) instead of the pod, for screenshots of the generated world.
pub fn debug_spot(def: &PlanetDef, vw: &VoxelWorld, landing: Vec3) -> Option<(Vec3, Vec3)> {
    let what = std::env::var("LOSTRANOUT_GOTO").ok()?;
    let spot = debug_spot_of(def, vw, landing, &what);
    eprintln!("[goto] {what}: {:?}", spot.map(|(p, l)| (vw.gen.depth(p), p.distance(l))));
    spot
}

fn debug_spot_of(def: &PlanetDef, vw: &VoxelWorld, landing: Vec3, what: &str) -> Option<(Vec3, Vec3)> {
    let gen = &vw.gen;
    let up0 = landing.normalize();
    let u0 = any_perp(up0);
    let w0 = up0.cross(u0);
    // directions on a widening spiral around the landing site
    let spiral = (1..400).flat_map(move |ring| {
        let rad = ring as f32 * 4.0;
        let n = ((rad * TAU / 6.0) as usize).max(1);
        (0..n).map(move |k| {
            let a = k as f32 / n as f32 * TAU + ring as f32 * 0.7;
            (up0 * def.radius + (u0 * a.cos() + w0 * a.sin()) * rad).normalize()
        })
    });
    let open = |p: Vec3| gen.density(p) < 0.0;
    match what {
        "upper" | "deep" => {
            let (lo, hi) = if what == "upper" { (8.0, 34.0) } else { (50.0, 110.0) };
            for dir in spiral {
                let h = gen.height(dir);
                let u = any_perp(dir);
                let w = dir.cross(u);
                let mut dd = lo;
                while dd < hi {
                    let p = dir * (h - dd);
                    dd += 1.5;
                    if gen.density(p) > -0.6 || !open(p + dir * 2.5) {
                        continue;
                    }
                    // floor below, then a hall: open in (almost) every direction at 7 m
                    let mut f = p;
                    for _ in 0..12 {
                        if !open(f - dir * 0.5) {
                            break;
                        }
                        f -= dir * 0.5;
                    }
                    let probe = f + dir * 1.6;
                    let dirs: Vec<Vec3> = (0..8).map(|k| Quat::from_axis_angle(dir, k as f32 / 8.0 * TAU) * u).collect();
                    let free = dirs.iter().filter(|t| open(probe + **t * 7.0) && open(probe + **t * 3.5)).count();
                    if free >= 7 && open(f + dir * 4.0) {
                        let _ = w;
                        let best = dirs.iter().max_by_key(|t| (1..12).take_while(|k| open(probe + **t * *k as f32 * 2.0)).count()).copied().unwrap_or(u);
                        return Some((f + dir * 0.3, f + best * 12.0));
                    }
                }
            }
            None
        }
        "entrance" => {
            for dir in spiral.skip(200) {
                let h = gen.height(dir);
                let u = any_perp(dir);
                let wide = [u, -u, dir.cross(u), -dir.cross(u)].iter().filter(|t| open(dir * (h - 2.0) + **t * 3.0)).count() >= 3;
                if wide && open(dir * (h - 4.0)) && open(dir * (h - 1.0)) && open(dir * (h - 8.0)) {
                    // stand on solid ground 14 m back towards the pod and look at the opening
                    let hole = dir * h;
                    for back in [10.0f32, 13.0, 16.0, 8.0] {
                        let toward = (landing - hole).reject_from(dir).normalize_or(any_perp(dir));
                        let sd = (hole + toward * back).normalize();
                        let sh = gen.height(sd);
                        if !open(sd * (sh - 1.5)) && !open(sd * (sh - 5.0)) {
                            return Some((sd * (sh + 0.3), hole - dir * 3.0));
                        }
                    }
                }
            }
            None
        }
        "wreck" => {
            let s = wreck_sites(def, vw, landing).into_iter().next()?;
            let p = offset_on_surface(vw, s.centre, 20.0, 0.6);
            Some((p + p.normalize() * 0.3, s.centre))
        }
        "scrap" => {
            // right in front of a piece of scrap at the first wreck site (for the dig bot)
            let sites = wreck_sites(def, vw, landing);
            let f = forced_decor(def, vw, landing, false, &sites);
            let s = f.iter().rev().find(|d| d.kind == DecorKind::Scrap && d.pos.distance(sites[0].centre) < 20.0)?;
            let p = offset_on_surface(vw, s.pos, 1.3, 1.0);
            Some((p + p.normalize() * 0.3, s.pos))
        }
        "vein" => {
            let g = cells_per_edge(def.radius);
            let far = Vec3::splat(1e6);
            let mut best: Option<(f32, Vec3)> = None;
            for cell in 0..6 * g * g {
                let c = {
                    let face = cell / (g * g);
                    let (j, i) = ((cell / g) % g, cell % g);
                    cube_dir(face, (i as f32 + 0.5) / g as f32 * 2.0 - 1.0, (j as f32 + 0.5) / g as f32 * 2.0 - 1.0)
                };
                let dist = c.angle_between(up0) * def.radius;
                if dist > 160.0 || dist < 20.0 {
                    continue;
                }
                let items = cell_decor(def, vw, g, cell, landing, far);
                let n = items.iter().filter(|d| matches!(d.kind, DecorKind::Crystal(_))).count();
                if n >= 3 && best.is_none_or(|b| dist < b.0) {
                    best = items.iter().find(|d| matches!(d.kind, DecorKind::Crystal(_))).map(|d| (dist, d.pos));
                }
            }
            let (_, pos) = best?;
            let p = offset_on_surface(vw, pos, 5.0, 2.0);
            Some((p + p.normalize() * 0.3, pos))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::StarSystem;

    #[test]
    fn decor_ids_are_stable_and_unique() {
        let sys = StarSystem::generate(9, 0);
        let def = &sys.planets[sys.home];
        let vw = VoxelWorld::new(def);
        let g = cells_per_edge(def.radius);
        let far = Vec3::splat(1e6);
        let mut ids = HashSet::new();
        let mut count = 0;
        for cell in (0..6 * g * g).step_by(97) {
            let a = cell_decor(def, &vw, g, cell, far, far);
            let b = cell_decor(def, &vw, g, cell, far, far);
            assert_eq!(a.len(), b.len());
            for (x, y) in a.iter().zip(&b) {
                assert_eq!(x.id, y.id);
                assert_eq!(x.pos, y.pos);
                assert!(ids.insert(x.id), "duplicate decor id {}", x.id);
                assert!(x.id < CAVE_ID);
            }
            count += a.len();
        }
        assert!(count > 20, "too little decor: {count}");
    }

    #[test]
    fn wreck_sites_exist_on_open_ground() {
        for seed in [5u64, 9, 21] {
            let sys = StarSystem::generate(seed, 0);
            for (i, def) in sys.planets.iter().enumerate() {
                let vw = VoxelWorld::new(def);
                let landing = vw.surface_point(vw.gen.landing).unwrap();
                let sites = wreck_sites(def, &vw, landing);
                assert!(sites.len() >= 3, "seed {seed} planet {i} ({:?}): only {} wreck sites", def.kind, sites.len());
                let d0 = sites[0].centre.normalize().angle_between(landing.normalize()) * def.radius;
                assert!((40.0..110.0).contains(&d0), "first wreck site should be in sight of the pod, is {d0} m away");
                for s in &sites {
                    assert_eq!(s.pieces.len(), props::W_PIECES as usize);
                    assert!(vw.gen.depth(s.centre).abs() < 1.5);
                }
                let forced = forced_decor(def, &vw, landing, i == sys.home, &sites);
                let frags: Vec<&Decor> = forced.iter().filter(|d| d.kind == DecorKind::Fragment).collect();
                assert_eq!(frags.len(), 4);
                for (k, f) in frags.iter().enumerate().skip(1) {
                    assert!(f.pos.distance(sites[k - 1].centre) < 12.0, "fragment {k} should lie at wreck site {}", k - 1);
                }
                let scrap = forced.iter().filter(|d| d.kind == DecorKind::Scrap).count();
                assert!(scrap >= 3 + 8 * 3);
                let ids: HashSet<u32> = forced.iter().map(|d| d.id).collect();
                assert_eq!(ids.len(), forced.len());
            }
        }
    }

    #[test]
    fn crystals_mark_veins_and_caves_get_decor() {
        let sys = StarSystem::generate(5, 0);
        let def = &sys.planets[sys.home];
        let mut vw = VoxelWorld::new(def);
        let g = cells_per_edge(def.radius);
        let far = Vec3::splat(1e6);
        let mut crystals = 0;
        for cell in (0..6 * g * g).step_by(7) {
            let items = cell_decor(def, &vw, g, cell, far, far);
            for d in &items {
                if let DecorKind::Crystal(item) = d.kind {
                    crystals += 1;
                    if d.id % PER_CELL < SLOTS {
                        // the main crystal of a cluster stands right over its vein
                        assert_eq!(vein_below(&vw, d.pos, d.pos.normalize()), Some(item), "a crystal without a vein below it");
                    } else {
                        // satellites stand next to their main crystal
                        let near = items.iter().filter(|m| m.kind == d.kind && m.id % PER_CELL < SLOTS).map(|m| m.pos.distance(d.pos)).fold(f32::MAX, f32::min);
                        assert!(near < 2.5, "satellite {near} m from its main crystal");
                    }
                }
            }
        }
        assert!(crystals > 60, "too few ore crystals on the surface: {crystals}");

        // cave chunks around a hall in the upper layer
        let mut ids = HashSet::new();
        let mut kinds = (0, 0, 0);
        let mut rng = Rng::new(4);
        let mut chunks = 0;
        let mut seen = HashSet::new();
        while chunks < 60 {
            let dir = rng.dir();
            let p = dir * (vw.gen.height(dir) - rng.range(10.0, 90.0));
            if vw.gen.density(p) > -0.5 {
                continue;
            }
            let c = (p / CS as f32).floor().as_ivec3();
            if !seen.insert(c) {
                continue;
            }
            vw.load_now(&[c]);
            chunks += 1;
            let a = cave_decor(def, &vw, c);
            let b = cave_decor(def, &vw, c);
            assert_eq!(a.len(), b.len());
            for (x, y) in a.iter().zip(&b) {
                assert_eq!((x.id, x.pos), (y.id, y.pos));
                assert!(x.id >= CAVE_ID && x.id < FORCED_ID);
                assert!(ids.insert(x.id), "duplicate cave decor id");
                assert!(vw.gen.depth(x.pos) > 4.0, "cave decor must be underground");
                match x.kind {
                    DecorKind::Glow(_) => kinds.0 += 1,
                    DecorKind::Crystal(_) => kinds.1 += 1,
                    DecorKind::Scrap => kinds.2 += 1,
                    _ => panic!("unexpected cave decor"),
                }
            }
        }
        println!("cave decor in {chunks} chunks: glow {} crystal {} scrap {}", kinds.0, kinds.1, kinds.2);
        assert!(kinds.0 > 20 && kinds.1 > 5, "caves need light-giving growths and wall crystals: {kinds:?}");
    }
}

