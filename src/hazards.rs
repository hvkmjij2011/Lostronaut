//! Hazards and what happens when the astronaut blacks out.
//!
//! Where hazards may appear follows the owner's rule, by depth zone and planet distance tier
//! (see `allowed`): the home planet only in the deep caves, the nearest planets also in the upper
//! caves, the far planets also on the surface. Placement is deterministic per surface cell
//! (planet seed + cell, like decor) and streams in and out around the player; only live
//! hazards near the player are simulated. Dug-out hazards are remembered in
//! `PlanetSave::hazards_cleared` by their stable id.
//!
//! Every hazard is telegraphed before it hurts (glow, swelling, dust, rings on the ground), gets
//! a HUD marker when close, and MIRA warns the first time each kind is met. Far planets also
//! have electric storm regions on the surface (`storm`).
//!
//! Blacking out (the system lives in `gameplay::blackout`) uses the pure helpers at the end of
//! this file: the backpack becomes a `Bundle` where the player fell, and the player is put on
//! the ground next to the base's main structure.
use crate::galaxy::{PlanetDef, PlanetKind};
use crate::items::{tx, Item, Lang, L2, SK};
use crate::player::{CamRig, Controls, Player};
use crate::state::{Bundle, Game, Structure};
use crate::util::{any_perp, noise3, Rng};
use crate::voxel::VoxelWorld;
use crate::world::{mobile, offset_on_surface, PlanetRt, PlanetScoped};
use crate::{Cine, MainCam, Sys, ToolMode, Ui};
use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use bevy_egui::egui::{self, Align2, Color32, FontId, Stroke};
use bevy_egui::EguiContexts;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::f32::consts::TAU;

// ---------------------------------------------------------------- rules (pure)

/// Depth bands of the shared contract: depth = gen.height(dir) - r.
pub const SURFACE_MAX: f32 = 4.0;
pub const UPPER_MAX: f32 = 40.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Zone {
    Surface,
    Upper,
    Deep,
}

pub fn zone_of(depth: f32) -> Zone {
    if depth < SURFACE_MAX {
        Zone::Surface
    } else if depth < UPPER_MAX {
        Zone::Upper
    } else {
        Zone::Deep
    }
}

/// Planet distance tier: home, near (|i - home| == 1), far (|i - home| >= 2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tier {
    Home,
    Near,
    Far,
}

pub fn tier_of(planet: usize, home: usize) -> Tier {
    match planet.abs_diff(home) {
        0 => Tier::Home,
        1 => Tier::Near,
        _ => Tier::Far,
    }
}

/// The owner's rule: home planet only in deep caves; nearest planets also in the upper caves;
/// far planets also on the surface.
pub fn allowed(tier: Tier, zone: Zone) -> bool {
    match tier {
        Tier::Home => zone == Zone::Deep,
        Tier::Near => zone != Zone::Surface,
        Tier::Far => true,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum HazKind {
    /// toxic spore pods: swell when approached, burst into an oxygen-eating cloud
    Spores,
    /// scalding vent / cryo geyser: glows, then erupts on a fixed cycle
    Vent,
    /// radiation hotspot around a uranite core
    Radiation,
    /// unstable cave ceiling: dust falls, then rocks
    Rockfall,
    /// crystal shards: cut and slow
    Shards,
    /// acid or magma pool: burns and slows
    Pool,
    /// electric storm on far planets' surface: rings on the ground, then lightning
    Storm,
}

impl HazKind {
    pub const ALL: [HazKind; 7] = [HazKind::Spores, HazKind::Vent, HazKind::Radiation, HazKind::Rockfall, HazKind::Shards, HazKind::Pool, HazKind::Storm];

    pub fn name(self, pk: PlanetKind) -> L2 {
        match self {
            HazKind::Spores => ("Zehirli Spor Kesesi", "Toxic Spore Pod"),
            HazKind::Vent if pk == PlanetKind::Ice => ("Kriyo Gayzer", "Cryo Geyser"),
            HazKind::Vent => ("Kaynar Buhar Bacası", "Scalding Vent"),
            HazKind::Radiation => ("Radyasyon Odağı", "Radiation Hotspot"),
            HazKind::Rockfall => ("Çatlak Tavan", "Unstable Ceiling"),
            HazKind::Shards => ("Kristal Diken", "Crystal Shards"),
            HazKind::Pool if pk == PlanetKind::Volcanic => ("Magma Gölcüğü", "Magma Pool"),
            HazKind::Pool => ("Asit Gölcüğü", "Acid Pool"),
            HazKind::Storm => ("Elektrik Fırtınası", "Electric Storm"),
        }
    }

    /// MIRA's warning the first time this kind is met.
    pub fn warning(self) -> L2 {
        match self {
            HazKind::Spores => (
                "MİRA: Spor keseleri yaklaşınca şişip patlar, bulut oksijenini yer. Uzak dur ya da uzaktan kaz.",
                "MIRA: Spore pods swell and burst when you get close; the cloud eats oxygen. Keep back or dig them from range.",
            ),
            HazKind::Vent => ("MİRA: Baca parlarsa birkaç saniyede fışkırır. Geçişini zamanla.", "MIRA: When a vent glows it erupts within seconds. Time your crossing."),
            HazKind::Radiation => ("MİRA: Radyasyon! Halkanın içinde sağlığın erir. Merkezdeki uranit kazılabilir.", "MIRA: Radiation! Inside the ring your health melts. The uranite core can be dug out."),
            HazKind::Rockfall => ("MİRA: Yerdeki moloz, tavanın gevşek olduğunu gösterir. Toz dökülürse hemen çekil.", "MIRA: Rubble on the floor means a loose ceiling. If dust falls, step away."),
            HazKind::Shards => ("MİRA: Kristal dikenler keser ve yavaşlatır. Etrafından dolaş ya da kazıp kuvars al.", "MIRA: Crystal shards cut and slow you. Go around, or dig them for quartz."),
            HazKind::Pool => ("MİRA: Parlayan gölcüğe basma: yakar ve yavaşlatır.", "MIRA: Don't step into the glowing pool: it burns and slows you."),
            HazKind::Storm => (
                "MİRA: Elektrik fırtınası! Yerde parlayan halka görürsen kaç. Mağaralar ve üs güvenli.",
                "MIRA: Electric storm! If a ring lights up under you, run. Caves and the base are safe.",
            ),
        }
    }

    /// What digging the hazard out with the excavator gives (None: cannot be dug out).
    pub fn reward(self) -> Option<(Item, u32)> {
        match self {
            HazKind::Spores => Some((Item::Organic, 3)),
            HazKind::Radiation => Some((Item::Uranite, 3)),
            HazKind::Shards => Some((Item::Quartz, 3)),
            _ => None,
        }
    }
}

/// Hazard kinds that fit a planet type in a zone, with relative weights.
pub fn kinds_for(pk: PlanetKind, uranite: bool, zone: Zone) -> Vec<(HazKind, f32)> {
    use HazKind::*;
    use PlanetKind as P;
    let cave = zone != Zone::Surface;
    let mut v = Vec::new();
    if cave {
        v.push((Rockfall, 3.0));
    }
    let shards = match (pk, zone) {
        (P::Crystal, _) => 4.0,
        (P::Ice, _) | (P::Barren, _) => 1.5,
        (_, Zone::Deep) => 1.5,
        (P::Desert, _) => 1.0,
        (_, Zone::Upper) => 0.8,
        _ => 0.0,
    };
    if shards > 0.0 {
        v.push((Shards, shards));
    }
    match pk {
        P::Temperate => v.push((Spores, 3.0)),
        P::Toxic => {
            v.push((Spores, 3.0));
            v.push((Pool, 2.0));
        }
        P::Desert | P::Ice => v.push((Vent, 2.0)),
        P::Volcanic => {
            v.push((Vent, 3.0));
            v.push((Pool, if cave { 1.5 } else { 2.5 }));
        }
        P::Barren | P::Crystal => {}
    }
    if uranite {
        v.push((Radiation, if zone == Zone::Deep { 2.5 } else { 1.5 }));
    }
    v
}

/// One placed hazard. `pos` is the floor point it stands on.
#[derive(Clone, Debug)]
pub struct Hazard {
    pub id: u32,
    pub kind: HazKind,
    pub pos: Vec3,
    pub depth: f32,
    pub zone: Zone,
    /// free height above the floor (rockfall), 0 elsewhere
    pub ceiling: f32,
    /// 0..1 size variation
    pub size: f32,
    /// 0..1 timing offset (vents)
    pub phase: f32,
    pub yaw: f32,
}

impl Hazard {
    /// Radius of the area that hurts.
    pub fn reach(&self) -> f32 {
        match self.kind {
            HazKind::Spores => 3.2,
            HazKind::Vent => 1.6,
            HazKind::Radiation => 6.5,
            HazKind::Rockfall => 2.3,
            HazKind::Shards => 1.5 + 0.5 * self.size,
            HazKind::Pool => 2.0 + 1.0 * self.size,
            HazKind::Storm => 2.4,
        }
    }
}

const PER_CELL: u32 = 4;
const TRIES: u32 = 3;

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

/// First cave floor along `dir` between depths `from` and `to` (metres below the surface) with
/// at least 3 m of open space above it.
fn cave_floor(vw: &VoxelWorld, dir: Vec3, h: f32, from: f32, to: f32) -> Option<Vec3> {
    let mut air = 0;
    let mut d = from;
    while d <= to {
        let r = h - d;
        if r < vw.gen.core + 2.0 {
            return None;
        }
        let p = dir * r;
        let solid = vw.get(p.x.round() as i32, p.y.round() as i32, p.z.round() as i32) > 0;
        if solid {
            if air >= 3 {
                return vw.raycast(dir * (r + 1.5), -dir, 3.0);
            }
            air = 0;
        } else {
            air += 1;
        }
        d += 1.0;
    }
    None
}

/// True when the ground within `r` of `pos` stays within 0.45 m of its height (pools and
/// vents are flat discs and look broken on a slope).
fn level(vw: &VoxelWorld, pos: Vec3, r: f32) -> bool {
    let up = pos.normalize();
    let u = any_perp(up);
    let v = up.cross(u);
    (0..6).all(|k| {
        let a = k as f32 / 6.0 * TAU;
        let p = pos + (u * a.cos() + v * a.sin()) * r;
        match vw.raycast(p + up * 1.5, -up, 3.0) {
            Some(g) => (g - pos).dot(up).abs() < 0.45,
            None => false,
        }
    })
}

/// Chance that one placement try in a cell produces a hazard (if the zone has room there).
fn zone_chance(z: Zone) -> f32 {
    match z {
        // tuned against the layered caves: a 14 m cell is mostly cave floor down there, so
        // these are per-cell chances of a hazard in an already open, walkable spot
        Zone::Surface => 0.15,
        Zone::Upper => 0.3,
        Zone::Deep => 0.32,
    }
}

/// The hazards of one surface cell (the decor grid, ~14 m cells). Pure apart from reading the
/// voxel world, so a cell always yields the same hazards with the same ids. `avoid` lists
/// spheres (landing site, pad) no hazard may sit in.
pub fn cell_hazards(def: &PlanetDef, tier: Tier, vw: &VoxelWorld, g: u32, cell: u32, avoid: &[(Vec3, f32)]) -> Vec<Hazard> {
    let face = cell / (g * g);
    let j = (cell / g) % g;
    let i = cell % g;
    let mut rng = Rng::new(def.seed ^ 0x4A2A_D500 ^ (cell as u64 + 1).wrapping_mul(0xA24B_AED4_963E_E407));
    let uranite = def.resources.iter().any(|r| r.0 == Item::Uranite);
    let zones: Vec<Zone> = [Zone::Surface, Zone::Upper, Zone::Deep].into_iter().filter(|z| allowed(tier, *z)).collect();
    let mut out: Vec<Hazard> = Vec::new();
    for k in 0..TRIES {
        // a fixed number of draws per try keeps every candidate stable on its own
        let (u, v, zr, chance, pick, size, phase, yaw) = (rng.f32(), rng.f32(), rng.f32(), rng.f32(), rng.f32(), rng.f32(), rng.f32(), rng.range(0.0, TAU));
        let zone = zones[((zr * zones.len() as f32) as usize).min(zones.len() - 1)];
        if chance > zone_chance(zone) {
            continue;
        }
        let dir = cube_dir(face, (i as f32 + u) / g as f32 * 2.0 - 1.0, (j as f32 + v) / g as f32 * 2.0 - 1.0);
        let h = vw.gen.height(dir);
        let floor = match zone {
            Zone::Surface => vw.raycast(dir * (h + 6.0), -dir, 14.0),
            Zone::Upper => cave_floor(vw, dir, h, SURFACE_MAX + 0.5, UPPER_MAX - 0.5),
            Zone::Deep => cave_floor(vw, dir, h, UPPER_MAX + 0.5, 160.0),
        };
        let Some(pos) = floor else { continue };
        let up = pos.normalize();
        let depth = vw.gen.height(up) - pos.length();
        let zone = zone_of(depth);
        if !allowed(tier, zone) || avoid.iter().any(|(c, r)| c.distance(pos) < *r) || out.iter().any(|o| o.pos.distance(pos) < 7.0) {
            continue;
        }
        let flat = vw.normal(pos).dot(up);
        if flat < 0.55 {
            continue;
        }
        let ceiling = if zone != Zone::Surface { vw.raycast(pos + up * 0.6, up, 9.0).map(|c| c.distance(pos)) } else { None };
        let mut kinds = kinds_for(def.kind, uranite, zone);
        kinds.retain(|(kd, _)| match kd {
            HazKind::Rockfall => ceiling.is_some_and(|c| (2.6..9.0).contains(&c)),
            HazKind::Pool | HazKind::Vent => flat > 0.85 && level(vw, pos, 2.6),
            _ => true,
        });
        if kinds.is_empty() {
            continue;
        }
        let total: f32 = kinds.iter().map(|k| k.1).sum();
        let mut acc = 0.0;
        let mut kind = kinds[kinds.len() - 1].0;
        for (kd, w) in &kinds {
            acc += w / total;
            if pick <= acc {
                kind = *kd;
                break;
            }
        }
        out.push(Hazard { id: cell * PER_CELL + k, kind, pos, depth, zone, ceiling: ceiling.unwrap_or(0.0), size, phase, yaw });
    }
    out
}

/// Storm field over a far planet's surface: storms rage where this is above `STORM_IN`.
pub fn storm_level(def: &PlanetDef, dir: Vec3) -> f32 {
    noise3(dir * def.radius * 0.012, (def.seed as u32) ^ 0x57AB)
}
pub const STORM_IN: f32 = 0.16;
/// distant lightning is visible from here on, before the storm can hit
pub const STORM_NEAR: f32 = 0.0;

// ---------------------------------------------------------------- blackout helpers (pure)

/// Moves the whole backpack into a new bundle at `pos` on the current planet. Soil and Data
/// stay with the player. Returns the bundle id (None when the backpack was empty).
pub fn drop_bundle(game: &mut Game, pos: Vec3) -> Option<u32> {
    let items: Vec<(Item, u32)> = game.inv.slots.iter_mut().filter_map(|s| s.take()).collect();
    if items.is_empty() {
        return None;
    }
    let id = game.next_id;
    game.next_id += 1;
    game.bundles.push(Bundle { id, planet: game.planet, pos: pos.to_array(), items });
    Some(id)
}

/// Where the bundle is left: where the player fell, unless that is inside a hazard (a pool, a
/// radiation ring...), then just outside its edge so getting the gear back does not hurt.
/// `hazards` are (centre, reach); `ground` finds the floor near a point.
pub fn bundle_spot(fell: Vec3, hazards: &[(Vec3, f32)], ground: &dyn Fn(Vec3) -> Option<Vec3>) -> Vec3 {
    let Some(&(c, r)) = hazards.iter().find(|(c, r)| {
        let up = c.normalize();
        let d = fell - *c;
        (d - up * d.dot(up)).length() < *r + 0.8 && d.dot(up).abs() < 3.0
    }) else {
        return fell;
    };
    let up = c.normalize();
    let out = (fell - c).reject_from(up).normalize_or(any_perp(up));
    let p = c + out * (r + 1.2) + up * (fell - c).dot(up).max(0.0);
    ground(p).unwrap_or(p)
}

/// Puts what fits from bundle `id` back into the backpack; the bundle disappears once empty.
/// Returns what was taken.
pub fn recover_bundle(game: &mut Game, id: u32) -> Vec<(Item, u32)> {
    let Some(i) = game.bundles.iter().position(|b| b.id == id) else { return Vec::new() };
    let mut b = game.bundles.remove(i);
    let mut taken: Vec<(Item, u32)> = Vec::new();
    for (item, n) in b.items.iter_mut() {
        let left = game.inv.add(*item, *n);
        if left < *n {
            match taken.iter_mut().find(|t| t.0 == *item) {
                Some(t) => t.1 += *n - left,
                None => taken.push((*item, *n - left)),
            }
        }
        *n = left;
    }
    b.items.retain(|x| x.1 > 0);
    if !b.items.is_empty() {
        game.bundles.insert(i, b);
    }
    taken
}

/// The base's main structure MIRA carries the player to: the escape pod on the home planet,
/// otherwise the shuttle, otherwise the base's oxygen source (the one closest to the landing
/// site). Returns its position and footprint radius; the landing site if there is nothing.
pub fn base_anchor(structs: &[Structure], home: bool, landing: Vec3) -> (Vec3, f32) {
    let by_landing = |k: &dyn Fn(SK) -> bool| structs.iter().filter(|s| k(s.kind)).min_by(|a, b| Vec3::from_array(a.pos).distance(landing).total_cmp(&Vec3::from_array(b.pos).distance(landing)));
    let pod = by_landing(&|k| k == SK::Pod);
    let shuttle = by_landing(&|k| k == SK::Shuttle);
    let oxygen = by_landing(&|k| k != SK::Tether && k.def().oxygen > 0.0);
    let pick = if home { pod.or(shuttle) } else { shuttle }.or(oxygen).or(pod);
    match pick {
        Some(s) => (Vec3::from_array(s.pos), s.kind.def().radius),
        None => (landing, 0.0),
    }
}

/// A spot on the ground right next to `anchor` (a structure of footprint `radius`): searched
/// on rings around it, walking down at most 6 m from just above the structure's own height, so
/// it is never far above or below the base (a cave roof, an overhang) and never in the air.
/// The spot has head room and is not inside any structure.
pub fn respawn_spot(density: &dyn Fn(Vec3) -> f32, anchor: Vec3, radius: f32, structs: &[Structure]) -> Vec3 {
    let up = anchor.normalize();
    let u = any_perp(up);
    let v = up.cross(u);
    let solid = |p: Vec3| density(p) > 0.0;
    for ring in [1.3, 2.2, 3.4, 5.0, 7.0] {
        let dist = radius + ring;
        for k in 0..16 {
            let a = k as f32 / 16.0 * TAU + 0.4;
            let top = anchor + (u * a.cos() + v * a.sin()) * dist + up * 2.5;
            if solid(top) {
                continue;
            }
            let mut ground = None;
            let mut t = 0.0;
            while t < 6.0 {
                let nt = t + 0.25;
                if solid(top - up * nt) {
                    let (mut lo, mut hi) = (t, nt);
                    for _ in 0..8 {
                        let m = 0.5 * (lo + hi);
                        if solid(top - up * m) {
                            hi = m
                        } else {
                            lo = m
                        }
                    }
                    ground = Some(top - up * lo);
                    break;
                }
                t = nt;
            }
            let Some(g) = ground else { continue };
            let gu = g.normalize();
            if (g - anchor).dot(up).abs() > 2.0 || [0.4, 1.0, 1.6, 2.1].iter().any(|h| solid(g + gu * *h)) {
                continue;
            }
            let e = 0.5;
            let grad = Vec3::new(density(g + Vec3::X * e) - density(g - Vec3::X * e), density(g + Vec3::Y * e) - density(g - Vec3::Y * e), density(g + Vec3::Z * e) - density(g - Vec3::Z * e));
            if (-grad).normalize_or(gu).dot(gu) < 0.6 {
                continue;
            }
            let blocked = structs.iter().any(|s| {
                let sp = Vec3::from_array(s.pos);
                let su = sp.normalize();
                let d = g - sp;
                (d - su * d.dot(su)).length() < s.kind.def().radius * 0.8 + 0.7
            });
            if !blocked {
                return g + gu * 0.05;
            }
        }
    }
    anchor + up * 0.6
}

/// Exact MIRA line after every blackout (owner's wording).
pub const CARRIED_HOME: L2 = ("MİRA ağırlıkları bırakıp seni üsse geri taşıdı.", "MIRA dropped the weight and carried you back to base.");

// ---------------------------------------------------------------- runtime state

/// Transient hazard feedback shared by the systems and the HUD.
#[derive(Resource)]
pub struct HazFx {
    /// screen-edge damage flash 0..1 and its colour
    pub flash: f32,
    pub flash_col: [f32; 3],
    /// movement factor applied to the input next frame (1 = normal)
    pub slow: f32,
    slow_timer: f32,
    /// last hazard that hurt and when (game time)
    pub cause: Option<HazKind>,
    pub cause_t: f64,
    /// radiation reading 0..1 for the HUD
    pub geiger: f32,
    /// under an electric storm
    pub storm: bool,
    /// blackout fade (seconds left) and its text
    pub fade: f32,
    pub fade_text: String,
    pub fade_sub: String,
    /// nearby hazards for HUD markers: (marker position, kind, dangerous right now)
    pub marks: Vec<(Vec3, HazKind, bool)>,
    storm_next: f32,
    ambient_next: f32,
    /// debug-only: hazards to place by hand (bot runs)
    pub debug_spawn: Vec<(HazKind, Vec3, f32)>,
    debug_id: u32,
}

impl Default for HazFx {
    fn default() -> Self {
        HazFx {
            flash: 0.0,
            flash_col: [1.0, 0.2, 0.1],
            slow: 1.0,
            slow_timer: 0.0,
            cause: None,
            cause_t: -100.0,
            geiger: 0.0,
            storm: false,
            fade: 0.0,
            fade_text: String::new(),
            fade_sub: String::new(),
            marks: Vec::new(),
            storm_next: 2.0,
            ambient_next: 1.0,
            debug_spawn: Vec::new(),
            debug_id: 0,
        }
    }
}

impl HazFx {
    fn hurt(&mut self, game: &mut Game, kind: HazKind, hp: f32, o2: f32, hit: bool) {
        crate::story::fire(game, crate::story::StoryEvent::HazardHit);
        game.health -= hp;
        game.oxygen = (game.oxygen - o2).max(0.0);
        self.flash = if hit { 1.0 } else { self.flash.max(0.55) };
        self.flash_col = if o2 > hp { [0.35, 1.0, 0.45] } else if kind == HazKind::Storm { [0.6, 0.8, 1.0] } else { [1.0, 0.15, 0.1] };
        if hit || game.time - self.cause_t > 1.0 {
            eprintln!("[haz] {:?} hurts: health {:.0} o2 {:.0}", kind, game.health, game.oxygen);
        }
        self.cause = Some(kind);
        self.cause_t = game.time;
    }
    /// Resets the per-life state (after a blackout).
    pub fn revive(&mut self) {
        self.flash = 0.0;
        self.slow = 1.0;
        self.slow_timer = 0.0;
        self.cause = None;
        self.geiger = 0.0;
    }
}

#[derive(Resource)]
pub struct HazGfx {
    sphere: Handle<Mesh>,
    cube: Handle<Mesh>,
    cyl: Handle<Mesh>,
    cone: Handle<Mesh>,
    ring: Handle<Mesh>,
    spore: Handle<StandardMaterial>,
    spore_hot: Handle<StandardMaterial>,
    cloud: Handle<StandardMaterial>,
    rock: Handle<StandardMaterial>,
    rubble: Handle<StandardMaterial>,
    dust: Handle<StandardMaterial>,
    vent_core: Handle<StandardMaterial>,
    vent_hot: Handle<StandardMaterial>,
    cryo_core: Handle<StandardMaterial>,
    cryo_hot: Handle<StandardMaterial>,
    steam: Handle<StandardMaterial>,
    frost: Handle<StandardMaterial>,
    uranite: Handle<StandardMaterial>,
    halo: Handle<StandardMaterial>,
    halo_ring: Handle<StandardMaterial>,
    shard: Handle<StandardMaterial>,
    acid: Handle<StandardMaterial>,
    magma: Handle<StandardMaterial>,
    crust: Handle<StandardMaterial>,
    strike_ring: Handle<StandardMaterial>,
    bolt: Handle<StandardMaterial>,
    bolt_glow: Handle<StandardMaterial>,
    pack: Handle<StandardMaterial>,
    strap: Handle<StandardMaterial>,
    beam: Handle<StandardMaterial>,
}

pub fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>) {
    let srgb = |c: [f32; 3]| Color::srgb(c[0], c[1], c[2]);
    let mut solid = |c: [f32; 3], glow: f32| {
        mats.add(StandardMaterial { base_color: srgb(c), emissive: LinearRgba::from(srgb(c)) * glow, perceptual_roughness: 0.6, ..default() })
    };
    let spore = solid([0.45, 0.32, 0.5], 0.25);
    let spore_hot = solid([0.85, 1.0, 0.3], 2.5);
    let rock = solid([0.36, 0.3, 0.26], 0.0);
    let rubble = solid([0.72, 0.66, 0.55], 0.15);
    let vent_core = solid([0.55, 0.22, 0.08], 0.6);
    let vent_hot = solid([1.0, 0.55, 0.15], 3.0);
    let cryo_core = solid([0.25, 0.45, 0.6], 0.6);
    let cryo_hot = solid([0.6, 0.95, 1.0], 3.0);
    let uranite = solid([0.7, 1.0, 0.25], 2.2);
    let shard = solid([0.75, 0.55, 1.0], 0.9);
    let acid = solid([0.45, 1.0, 0.25], 1.4);
    let magma = solid([1.0, 0.42, 0.08], 2.4);
    let crust = solid([0.18, 0.16, 0.15], 0.0);
    let pack = solid([1.0, 0.55, 0.12], 0.5);
    let strap = solid([0.2, 0.2, 0.22], 0.0);
    let mut clear = |c: [f32; 4]| {
        mats.add(StandardMaterial { base_color: Color::srgba(c[0], c[1], c[2], c[3]), unlit: true, alpha_mode: AlphaMode::Blend, cull_mode: None, double_sided: true, fog_enabled: false, ..default() })
    };
    let g = HazGfx {
        sphere: meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap()),
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        cyl: meshes.add(Cylinder::new(1.0, 1.0)),
        cone: meshes.add(Cone::new(1.0, 1.0)),
        ring: meshes.add(Torus::new(0.93, 1.0)),
        spore,
        spore_hot,
        cloud: clear([0.35, 0.85, 0.15, 0.42]),
        rock,
        rubble,
        dust: clear([0.85, 0.8, 0.7, 0.7]),
        vent_core,
        vent_hot,
        cryo_core,
        cryo_hot,
        steam: clear([1.0, 0.85, 0.7, 0.45]),
        frost: clear([0.8, 0.95, 1.0, 0.45]),
        uranite,
        halo: clear([0.75, 1.0, 0.3, 0.1]),
        halo_ring: clear([0.8, 1.0, 0.3, 0.65]),
        shard,
        acid,
        magma,
        crust,
        strike_ring: clear([0.45, 0.35, 1.0, 0.95]),
        bolt: clear([0.85, 0.8, 1.0, 1.0]),
        bolt_glow: clear([0.5, 0.35, 1.0, 0.35]),
        pack,
        strap,
        beam: clear([1.0, 0.6, 0.15, 0.35]),
    };
    commands.insert_resource(g);
}

#[derive(Component)]
pub struct HazPart;

#[derive(Component)]
pub struct HazEnt {
    pub h: Hazard,
    pk: PlanetKind,
    parts: Vec<Entity>,
    base: Vec<Transform>,
    /// 0 idle, 1 telegraphing, 2 active, 3 spent / recharging
    pub stage: u8,
    t: f32,
    dig: f32,
}

#[derive(Resource)]
pub struct HazStream {
    planet: usize,
    landing: Vec3,
    tier: Tier,
    g: u32,
    centres: Vec<Vec3>,
    live: HashSet<u32>,
    avoid: Vec<(Vec3, f32)>,
    radius: f32,
    frame: u32,
    /// (zone, kind) of everything spawned so far, for the log
    pub spawned: u32,
}

/// Forget the streamed hazards when leaving the planet (their entities are planet scoped).
pub fn reset(mut commands: Commands, mut fx: ResMut<HazFx>) {
    commands.remove_resource::<HazStream>();
    let debug = std::mem::take(&mut fx.debug_spawn);
    *fx = HazFx { debug_spawn: debug, ..default() };
}

fn part(m: &Handle<Mesh>, mat: &Handle<StandardMaterial>, tf: Transform, vis: bool) -> (Handle<Mesh>, Handle<StandardMaterial>, Transform, bool) {
    (m.clone(), mat.clone(), tf, vis)
}

fn spawn_hazard(commands: &mut Commands, hg: &HazGfx, pk: PlanetKind, h: &Hazard) {
    let up = h.pos.normalize();
    let rot = Quat::from_rotation_arc(Vec3::Y, up) * Quat::from_rotation_y(h.yaw);
    let s = h.size;
    let mut p = Vec::new();
    match h.kind {
        HazKind::Spores => {
            for (k, (x, z)) in [(0.45, 0.1), (-0.35, 0.35), (0.0, -0.45), (-0.2, -0.05)].into_iter().enumerate() {
                let r = [0.55, 0.45, 0.5, 0.32][k] * (0.85 + 0.3 * s);
                p.push(part(&hg.sphere, &hg.spore, Transform::from_xyz(x, r * 0.8, z).with_scale(Vec3::new(r, r * 1.15, r)), true));
            }
            p.push(part(&hg.sphere, &hg.cloud, Transform::from_xyz(0.0, 1.0, 0.0), false));
        }
        HazKind::Vent => {
            let (core, gas) = if pk == PlanetKind::Ice { (&hg.cryo_core, &hg.frost) } else { (&hg.vent_core, &hg.steam) };
            p.push(part(&hg.ring, &hg.crust, Transform::from_xyz(0.0, 0.05, 0.0).with_scale(Vec3::new(1.3, 2.5, 1.3)), true));
            p.push(part(&hg.cyl, core, Transform::from_xyz(0.0, 0.02, 0.0).with_scale(Vec3::new(0.95, 0.08, 0.95)), true));
            p.push(part(&hg.cyl, gas, Transform::from_xyz(0.0, 4.0, 0.0).with_scale(Vec3::new(1.3, 8.0, 1.3)), false));
            for _ in 0..3 {
                p.push(part(&hg.sphere, gas, Transform::from_scale(Vec3::splat(0.35)), false));
            }
        }
        HazKind::Radiation => {
            for (k, (x, z, tilt)) in [(0.0, 0.0, 0.0), (0.4, 0.2, 0.35), (-0.3, 0.3, -0.4), (0.1, -0.4, 0.3)].into_iter().enumerate() {
                let hgt = [1.4, 0.9, 0.8, 0.7][k];
                let rot = Quat::from_rotation_z(tilt) * Quat::from_rotation_x(tilt * 0.7);
                p.push(part(&hg.cone, &hg.uranite, Transform::from_xyz(x, hgt * 0.45, z).with_rotation(rot).with_scale(Vec3::new(0.28, hgt, 0.28)), true));
            }
            p.push(part(&hg.sphere, &hg.halo, Transform::from_xyz(0.0, 0.5, 0.0).with_scale(Vec3::splat(6.5)), true));
            p.push(part(&hg.ring, &hg.halo_ring, Transform::from_xyz(0.0, 0.08, 0.0).with_scale(Vec3::new(6.5, 2.0, 6.5)), true));
        }
        HazKind::Rockfall => {
            let c = h.ceiling;
            for (x, z, sz) in [(0.5, 0.2, 0.9), (-0.4, 0.5, 0.75), (0.0, -0.6, 0.8)] {
                p.push(part(&hg.cube, &hg.rock, Transform::from_xyz(x, c - 0.3, z).with_rotation(Quat::from_euler(EulerRot::XYZ, x, z, x + z)).with_scale(Vec3::splat(sz)), true));
            }
            // a heap of pale rubble under the loose patch tells where it comes down
            p.push(part(&hg.cube, &hg.rubble, Transform::from_xyz(0.1, 0.05, -0.1).with_rotation(Quat::from_euler(EulerRot::XYZ, 0.3, 0.7, 0.2)).with_scale(Vec3::new(0.6, 0.35, 0.5)), true));
            for (x, z) in [(1.2, 0.4), (-0.9, -0.9), (0.3, -1.4), (-1.3, 0.8), (0.6, 1.2)] {
                p.push(part(&hg.cube, &hg.rubble, Transform::from_xyz(x, 0.06, z).with_rotation(Quat::from_euler(EulerRot::XYZ, x, x * 3.0, z)).with_scale(Vec3::splat(0.3)), true));
            }
            for _ in 0..4 {
                p.push(part(&hg.sphere, &hg.dust, Transform::from_scale(Vec3::splat(0.07)), false));
            }
        }
        HazKind::Shards => {
            let r = 1.5 + 0.5 * s;
            for k in 0..9 {
                let a = k as f32 * 2.4;
                let d = if k == 0 { 0.0 } else { r * (0.35 + 0.6 * ((k * 7 % 5) as f32 / 4.0)) };
                let hgt = 0.6 + 0.8 * ((k * 3 % 4) as f32 / 3.0);
                let lean = Quat::from_rotation_y(a) * Quat::from_rotation_z(0.25 + 0.15 * (k % 3) as f32);
                p.push(part(&hg.cone, &hg.shard, Transform::from_xyz(a.cos() * d, hgt * 0.42, a.sin() * d).with_rotation(lean).with_scale(Vec3::new(0.16, hgt, 0.16)), true));
            }
        }
        HazKind::Pool => {
            let r = 2.0 + s;
            let liquid = if pk == PlanetKind::Volcanic { &hg.magma } else { &hg.acid };
            p.push(part(&hg.cyl, liquid, Transform::from_xyz(0.0, -0.08, 0.0).with_scale(Vec3::new(r, 0.3, r)), true));
            p.push(part(&hg.ring, &hg.crust, Transform::from_xyz(0.0, 0.03, 0.0).with_scale(Vec3::new(r + 0.15, 3.0, r + 0.15)), true));
            for k in 0..4 {
                let a = k as f32 * 1.7;
                p.push(part(&hg.sphere, liquid, Transform::from_xyz(a.cos() * r * 0.5, 0.06, a.sin() * r * 0.5).with_scale(Vec3::splat(0.15)), true));
            }
        }
        HazKind::Storm => {}
    }
    let root = commands.spawn((Transform::from_translation(h.pos).with_rotation(rot), Visibility::Visible, PlanetScoped)).id();
    let mut ids = Vec::new();
    let mut base = Vec::new();
    commands.entity(root).with_children(|c| {
        for (m, mat, tf, vis) in p {
            ids.push(c.spawn((Mesh3d(m), MeshMaterial3d(mat), tf, if vis { Visibility::Inherited } else { Visibility::Hidden }, NotShadowCaster, HazPart)).id());
            base.push(tf);
        }
    });
    commands.entity(root).insert(HazEnt { h: h.clone(), pk, parts: ids, base, stage: 0, t: 0.0, dig: 0.0 });
}

/// Streams hazards in and out around the player, cell by cell (like decor).
#[allow(clippy::too_many_arguments)]
pub fn stream_hazards(
    mut commands: Commands,
    mut vw: ResMut<VoxelWorld>,
    rt: Res<PlanetRt>,
    sys: Res<Sys>,
    game: Res<Game>,
    hg: Res<HazGfx>,
    mut fx: ResMut<HazFx>,
    stream: Option<ResMut<HazStream>>,
    player: Query<&Transform, With<Player>>,
    ents: Query<(Entity, &HazEnt)>,
) {
    let Some(mut st) = stream else {
        let g = crate::decor::cells_per_edge(rt.def.radius);
        let mut centres = Vec::with_capacity((6 * g * g) as usize);
        for face in 0..6 {
            for j in 0..g {
                for i in 0..g {
                    centres.push(cube_dir(face, (i as f32 + 0.5) / g as f32 * 2.0 - 1.0, (j as f32 + 0.5) / g as f32 * 2.0 - 1.0));
                }
            }
        }
        let pad = offset_on_surface(&vw, rt.landing, 11.0, 0.0);
        let tier = tier_of(game.planet, sys.0.home);
        eprintln!("[haz] planet {} ({:?}) tier {:?}", game.planet, rt.def.kind, tier);
        commands.insert_resource(HazStream {
            planet: game.planet,
            landing: rt.landing,
            tier,
            g,
            centres,
            live: HashSet::new(),
            avoid: vec![(rt.landing, 28.0), (pad, 18.0)],
            radius: if mobile() { 55.0 } else { 70.0 },
            frame: 0,
            spawned: 0,
        });
        return;
    };
    let st = &mut *st;
    if st.planet != game.planet || st.landing != rt.landing {
        commands.remove_resource::<HazStream>();
        return;
    }
    let Ok(ptf) = player.single() else { return };
    let focus = ptf.translation;
    // debug placements (bot runs only) bypass the zone rules on purpose
    for (kind, pos, size) in std::mem::take(&mut fx.debug_spawn) {
        fx.debug_id += 1;
        let up = pos.normalize();
        if matches!(kind, HazKind::Pool | HazKind::Vent) {
            for _ in 0..3 {
                vw.edit(pos, 4.0, &crate::voxel::Brush::Flatten { anchor: pos, normal: up }, 1.0);
            }
        }
        let ceiling = if kind == HazKind::Rockfall { vw.raycast(pos + up * 0.6, up, 9.0).map(|c| c.distance(pos)).unwrap_or(4.0) } else { 0.0 };
        let h = Hazard { id: u32::MAX - fx.debug_id, kind, pos, depth: vw.gen.height(up) - pos.length(), zone: zone_of(vw.gen.height(up) - pos.length()), ceiling, size, phase: 0.0, yaw: 0.3 };
        eprintln!("[haz] debug {:?} at depth {:.1}", kind, h.depth);
        spawn_hazard(&mut commands, &hg, rt.def.kind, &h);
    }
    st.frame = st.frame.wrapping_add(1);
    if st.frame % 5 != 0 {
        return;
    }
    let cleared: HashSet<u32> = game.planets[game.planet].hazards_cleared.iter().copied().collect();
    let r = focus.length();
    let mut drop: HashSet<u32> = HashSet::new();
    let mut budget = 4;
    for (ci, c) in st.centres.iter().enumerate() {
        let ci = ci as u32;
        let d = (*c * r).distance(focus);
        if st.live.contains(&ci) {
            if d > st.radius + 25.0 {
                drop.insert(ci);
            }
        } else if d < st.radius && budget > 0 {
            budget -= 1;
            for h in cell_hazards(&rt.def, st.tier, &vw, st.g, ci, &st.avoid) {
                if cleared.contains(&h.id) || near_structure(game.structs(), h.pos) {
                    continue;
                }
                st.spawned += 1;
                eprintln!("[haz] {:?} {:?} depth {:.0} id {} (dist {:.0})", h.kind, h.zone, h.depth, h.id, h.pos.distance(focus));
                spawn_hazard(&mut commands, &hg, rt.def.kind, &h);
            }
            st.live.insert(ci);
        }
    }
    if !drop.is_empty() {
        for c in &drop {
            st.live.remove(c);
        }
        for (e, he) in &ents {
            if he.h.id < u32::MAX / 2 && drop.contains(&(he.h.id / PER_CELL)) {
                commands.entity(e).despawn();
            }
        }
    }
}

/// Hazards keep out of bases: nothing within 12 m of a structure (3 m of an oxygen relay).
fn near_structure(structs: &[Structure], p: Vec3) -> bool {
    structs.iter().any(|s| Vec3::from_array(s.pos).distance(p) < if s.kind == SK::Tether { 3.0 } else { 12.0 } + s.kind.def().radius)
}

type Parts<'w, 's> = Query<'w, 's, (&'static mut Transform, &'static mut Visibility, &'static mut MeshMaterial3d<StandardMaterial>), (With<HazPart>, Without<HazEnt>, Without<Player>, Without<StrikePart>)>;

fn set_tf(parts: &mut Parts, e: Entity, tf: Transform) {
    if let Ok((mut t, _, _)) = parts.get_mut(e) {
        *t = tf;
    }
}
fn set_vis(parts: &mut Parts, e: Entity, on: bool) {
    if let Ok((_, mut v, _)) = parts.get_mut(e) {
        let want = if on { Visibility::Inherited } else { Visibility::Hidden };
        if *v != want {
            *v = want;
        }
    }
}
fn set_mat(parts: &mut Parts, e: Entity, m: &Handle<StandardMaterial>) {
    if let Ok((_, _, mut mm)) = parts.get_mut(e) {
        if mm.0 != *m {
            mm.0 = m.clone();
        }
    }
}

/// Animates live hazards, applies their effects to the player, handles digging them out and
/// MIRA's first warnings.
#[allow(clippy::too_many_arguments)]
pub fn update_hazards(
    mut commands: Commands,
    time: Res<Time>,
    (mut game, mut ui, cine, c): (ResMut<Game>, ResMut<Ui>, Res<Cine>, Res<Controls>),
    (vw, hg): (Res<VoxelWorld>, Res<HazGfx>),
    mut fx: ResMut<HazFx>,
    cam: Query<(&Camera, &GlobalTransform), With<MainCam>>,
    mut player: Query<(&Transform, &mut Player)>,
    mut hz: Query<(Entity, &mut HazEnt)>,
    mut parts: Parts,
    mut structs_seen: Local<usize>,
) {
    let dt = time.delta_secs().min(0.05);
    fx.flash = (fx.flash - dt * 1.6).max(0.0);
    fx.slow_timer = (fx.slow_timer - dt).max(0.0);
    let mut slow: f32 = if fx.slow_timer > 0.0 { 0.5 } else { 1.0 };
    fx.geiger = 0.0;
    fx.marks.clear();
    let Ok((ptf, mut pl)) = player.single_mut() else { return };
    if cine.0.is_some() || ui.paused {
        fx.slow = 1.0;
        return;
    }
    let l = game.lang;
    let ppos = ptf.translation;
    let pup = ppos.normalize();
    // a new structure pushes hazards out of its base
    if game.structs().len() != *structs_seen {
        *structs_seen = game.structs().len();
        for (e, he) in &hz {
            if near_structure(game.structs(), he.h.pos) {
                commands.entity(e).despawn();
            }
        }
    }
    // excavator aimed at a hazard
    let dig_hit = if pl.using_tool && c.tool && ui.tool_mode == ToolMode::Dig {
        cam.single().ok().and_then(|(camera, gt)| camera.viewport_to_world(gt, c.pointer?).ok()).and_then(|ray| vw.raycast(ray.origin, *ray.direction, 80.0))
    } else {
        None
    };
    let now = game.time;
    let mut marks: Vec<(f32, Vec3, HazKind, bool)> = Vec::new();
    let mut warn: Option<HazKind> = None;
    for (e, mut he) in &mut hz {
        let he = &mut *he;
        let h = he.h.clone();
        let up = h.pos.normalize();
        let rel = ppos - h.pos;
        let hgt = rel.dot(up);
        let flat = (rel - up * hgt).length();
        let dist = rel.length();
        if dist > 90.0 {
            continue;
        }
        he.t += dt;
        let t = he.t;
        let pk = he.pk;
        match h.kind {
            HazKind::Spores => {
                let pods = 4;
                match he.stage {
                    0 => {
                        for k in 0..pods {
                            let b = he.base[k];
                            set_tf(&mut parts, he.parts[k], b.with_scale(b.scale * (1.0 + 0.06 * (t * 2.0 + k as f32).sin())));
                        }
                        if flat < 4.5 && hgt.abs() < 3.0 {
                            he.stage = 1;
                            he.t = 0.0;
                            for k in 0..pods {
                                set_mat(&mut parts, he.parts[k], &hg.spore_hot);
                            }
                        }
                    }
                    1 => {
                        let f = (t / 1.2).min(1.0);
                        for k in 0..pods {
                            let b = he.base[k];
                            let j = Vec3::new((t * 37.0 + k as f32).sin(), 0.0, (t * 41.0 + k as f32).cos()) * 0.04 * f;
                            set_tf(&mut parts, he.parts[k], Transform { translation: b.translation + j, scale: b.scale * (1.0 + 0.55 * f), ..b });
                        }
                        if t >= 1.2 {
                            he.stage = 2;
                            he.t = 0.0;
                            for k in 0..pods {
                                set_vis(&mut parts, he.parts[k], false);
                            }
                            set_vis(&mut parts, he.parts[pods], true);
                        }
                    }
                    2 => {
                        let r = 3.2 * (t / 0.5).min(1.0) * if t > 3.5 { (1.0 - (t - 3.5)).max(0.05) } else { 1.0 };
                        set_tf(&mut parts, he.parts[pods], Transform::from_xyz(0.0, 1.0, 0.0).with_scale(Vec3::splat(r)));
                        if (ppos + pup * 0.9).distance(h.pos + up * 1.0) < r {
                            fx.hurt(&mut game, h.kind, 2.0 * dt, 18.0 * dt, false);
                        }
                        if t >= 4.5 {
                            he.stage = 3;
                            he.t = 0.0;
                            set_vis(&mut parts, he.parts[pods], false);
                            for k in 0..pods {
                                set_mat(&mut parts, he.parts[k], &hg.spore);
                                set_vis(&mut parts, he.parts[k], true);
                            }
                        }
                    }
                    _ => {
                        let f = 0.25 + 0.75 * (t / 14.0).min(1.0);
                        for k in 0..pods {
                            let b = he.base[k];
                            set_tf(&mut parts, he.parts[k], b.with_scale(b.scale * f));
                        }
                        if t >= 14.0 {
                            he.stage = 0;
                        }
                    }
                }
            }
            HazKind::Vent => {
                const CYCLE: f64 = 8.5;
                let cyc = ((now + h.phase as f64 * CYCLE) % CYCLE) as f32;
                let (dim, hot) = if pk == PlanetKind::Ice { (&hg.cryo_core, &hg.cryo_hot) } else { (&hg.vent_core, &hg.vent_hot) };
                let stage = if cyc < 4.5 { 0 } else if cyc < 6.0 { 1 } else { 2 };
                he.stage = stage;
                set_mat(&mut parts, he.parts[1], if stage == 0 { dim } else { hot });
                let pulse = if stage == 1 { 1.0 + 0.25 * (cyc * 22.0).sin().abs() } else { 1.0 };
                let b = he.base[1];
                set_tf(&mut parts, he.parts[1], b.with_scale(Vec3::new(b.scale.x * pulse, b.scale.y, b.scale.z * pulse)));
                for k in 0..3 {
                    let on = stage == 1;
                    set_vis(&mut parts, he.parts[3 + k], on);
                    if on {
                        let y = ((cyc - 4.5) * 1.8 + k as f32 * 0.6) % 1.8 * 1.6;
                        let a = k as f32 * 2.1;
                        set_tf(&mut parts, he.parts[3 + k], Transform::from_xyz(a.cos() * 0.3, 0.3 + y, a.sin() * 0.3).with_scale(Vec3::splat(0.25 + 0.15 * y)));
                    }
                }
                set_vis(&mut parts, he.parts[2], stage == 2);
                if stage == 2 {
                    let e2 = cyc - 6.0;
                    let hh = 8.0 * (e2 / 0.3).min(1.0) * if e2 > 2.1 { ((2.5 - e2) / 0.4).max(0.05) } else { 1.0 };
                    set_tf(&mut parts, he.parts[2], Transform::from_xyz(0.0, hh * 0.5, 0.0).with_scale(Vec3::new(1.2 + 0.1 * (e2 * 30.0).sin(), hh, 1.2)));
                    if flat < h.reach() && hgt > -1.0 && hgt < hh {
                        fx.hurt(&mut game, h.kind, 28.0 * dt, 0.0, false);
                        // thrown up and out of the plume
                        let out = (rel - up * hgt).normalize_or(any_perp(up));
                        let vn = pl.vel.dot(up);
                        pl.vel += up * (7.5 - vn).max(0.0) + out * 6.0 * dt;
                    }
                }
            }
            HazKind::Radiation => {
                let r = h.reach();
                let b = he.base[4];
                set_tf(&mut parts, he.parts[4], b.with_scale(Vec3::splat(r * (1.0 + 0.03 * (t * 3.0).sin()))));
                let d = (ppos + pup * 0.9).distance(h.pos + up * 0.6);
                if d < r + 3.0 {
                    fx.geiger = fx.geiger.max((1.0 - (d - 1.0) / (r + 2.0)).clamp(0.0, 1.0));
                }
                he.stage = if d < r { 2 } else { 0 };
                if d < r {
                    fx.hurt(&mut game, h.kind, (2.0 + 8.0 * (1.0 - d / r)) * dt, 0.0, false);
                }
            }
            HazKind::Rockfall => {
                let rocks = 3;
                let dust0 = 9;
                let c0 = h.ceiling - 0.3;
                match he.stage {
                    0 => {
                        if flat < 2.5 && hgt > -0.5 && hgt < h.ceiling {
                            he.stage = 1;
                            eprintln!("[haz] Rockfall: dust falling");
                            he.t = 0.0;
                        }
                    }
                    1 => {
                        for k in 0..rocks {
                            let b = he.base[k];
                            let j = Vec3::new((t * 45.0 + k as f32).sin(), 0.0, (t * 53.0 + k as f32 * 2.0).cos()) * 0.07;
                            set_tf(&mut parts, he.parts[k], Transform { translation: b.translation + j, ..b });
                        }
                        for k in 0..4 {
                            set_vis(&mut parts, he.parts[dust0 + k], true);
                            let f = (t * 1.6 + k as f32 * 0.27) % 1.0;
                            let a = k as f32 * 1.9;
                            set_tf(&mut parts, he.parts[dust0 + k], Transform::from_xyz(a.cos() * 0.6, c0 - f * c0, a.sin() * 0.6).with_scale(Vec3::splat(0.08)));
                        }
                        if t >= 1.1 {
                            he.stage = 2;
                            he.t = 0.0;
                        }
                    }
                    2 => {
                        let y = c0 - 0.5 * 16.0 * t * t;
                        for k in 0..rocks {
                            let b = he.base[k];
                            let mut tf = b;
                            tf.translation.y = y.max(0.3 + 0.1 * k as f32);
                            set_tf(&mut parts, he.parts[k], tf);
                        }
                        if y <= 0.3 {
                            for k in 0..4 {
                                set_vis(&mut parts, he.parts[dust0 + k], false);
                            }
                            if flat < h.reach() && hgt > -0.5 && hgt < 2.2 {
                                fx.hurt(&mut game, h.kind, 32.0, 0.0, true);
                                fx.slow_timer = 2.5;
                            }
                            he.stage = 3;
                            he.t = 0.0;
                        }
                    }
                    _ => {
                        if t > 3.0 {
                            for k in 0..rocks {
                                set_vis(&mut parts, he.parts[k], false);
                            }
                        }
                        if t > 40.0 {
                            for k in 0..rocks {
                                set_tf(&mut parts, he.parts[k], he.base[k]);
                                set_vis(&mut parts, he.parts[k], true);
                            }
                            he.stage = 0;
                        }
                    }
                }
            }
            HazKind::Shards => {
                he.stage = 0;
                if flat < h.reach() && hgt > -0.5 && hgt < 1.4 {
                    he.stage = 2;
                    fx.hurt(&mut game, h.kind, 12.0 * dt, 0.0, false);
                    slow = slow.min(0.45);
                }
            }
            HazKind::Pool => {
                let r = h.reach();
                for k in 0..4 {
                    let b = he.base[2 + k];
                    let f = (t * 0.9 + k as f32 * 0.37) % 1.0;
                    set_tf(&mut parts, he.parts[2 + k], b.with_scale(Vec3::splat(0.05 + 0.25 * f)));
                }
                he.stage = 0;
                if flat < r && hgt > -0.6 && hgt < 0.9 {
                    he.stage = 2;
                    let burn = if pk == PlanetKind::Volcanic { 24.0 } else { 16.0 };
                    fx.hurt(&mut game, h.kind, burn * dt, 0.0, false);
                    slow = slow.min(0.5);
                }
            }
            HazKind::Storm => {}
        }
        // excavator: dig the hazard out for its reward
        if let (Some(hit), Some((item, n))) = (dig_hit, h.kind.reward()) {
            if hit.distance(h.pos + up * 0.4) < 1.2 + ui.brush {
                he.dig += dt;
                let f = 1.0 - 0.6 * (he.dig / 0.9).min(1.0);
                for (k, b) in he.base.iter().enumerate() {
                    if h.kind != HazKind::Radiation || k < 4 {
                        set_tf(&mut parts, he.parts[k], b.with_scale(b.scale * f));
                    }
                }
                if he.dig >= 0.9 {
                    crate::gameplay::give(&mut game, &mut ui, item, n);
                    let p = game.planet;
                    if h.id < u32::MAX / 2 {
                        game.planets[p].hazards_cleared.push(h.id);
                    }
                    ui.toast(format!("{}: {}", if l == Lang::Tr { "Temizlendi" } else { "Cleared" }, tx(l, h.kind.name(pk))), [0.6, 1.0, 0.6]);
                    commands.entity(e).despawn();
                    continue;
                }
            }
        }
        if dist < 16.0 {
            let danger = he.stage == 1 || he.stage == 2;
            let at = match h.kind {
                HazKind::Radiation => h.pos + up * 2.0,
                HazKind::Vent => h.pos + up * 2.2,
                _ => h.pos + up * 1.6,
            };
            marks.push((dist, at, h.kind, danger));
        }
        if dist < 13.0 && !game.hazards_seen.contains(&h.kind) && warn.is_none() {
            warn = Some(h.kind);
        }
    }
    if let Some(k) = warn {
        game.hazards_seen.push(k);
        ui.toast(tx(l, k.warning()).to_string(), [1.0, 0.75, 0.35]);
    }
    marks.sort_by(|a, b| a.0.total_cmp(&b.0));
    fx.marks = marks.into_iter().take(3).map(|m| (m.1, m.2, m.3)).collect();
    fx.slow = slow;
}

/// Applies hazard slowing to the movement input (runs between input and movement).
pub fn slow_input(fx: Res<HazFx>, mut c: ResMut<Controls>) {
    if fx.slow < 1.0 {
        c.mv *= fx.slow;
        c.sprint = false;
    }
}

// ---------------------------------------------------------------- electric storms

#[derive(Component)]
pub struct Strike {
    t: f32,
    pos: Vec3,
    harmful: bool,
    done: bool,
    parts: [Entity; 4],
}

#[derive(Component)]
pub struct StrikePart;

fn spawn_strike(commands: &mut Commands, hg: &HazGfx, pos: Vec3, harmful: bool) {
    let up = pos.normalize();
    let root = commands.spawn((Transform::from_translation(pos).with_rotation(Quat::from_rotation_arc(Vec3::Y, up)), Visibility::Visible, PlanetScoped)).id();
    let mut ids = [Entity::PLACEHOLDER; 4];
    let specs = [
        (&hg.ring, &hg.strike_ring, Transform::from_xyz(0.0, 0.08, 0.0).with_scale(Vec3::new(2.4, 3.0, 2.4))),
        (&hg.cyl, &hg.bolt_glow, Transform::from_xyz(0.0, 15.0, 0.0).with_scale(Vec3::new(0.12, 30.0, 0.12))),
        (&hg.cyl, &hg.bolt, Transform::from_xyz(0.0, 20.0, 0.0).with_scale(Vec3::new(0.18, 40.0, 0.18))),
        (&hg.cyl, &hg.bolt_glow, Transform::from_xyz(0.0, 20.0, 0.0).with_scale(Vec3::new(0.9, 40.0, 0.9))),
    ];
    commands.entity(root).with_children(|c| {
        for (k, (m, mat, tf)) in specs.into_iter().enumerate() {
            ids[k] = c.spawn((Mesh3d(m.clone()), MeshMaterial3d(mat.clone()), tf, Visibility::Hidden, NotShadowCaster, StrikePart)).id();
        }
    });
    commands.entity(root).insert(Strike { t: if harmful { 0.0 } else { 1.6 }, pos, harmful, done: false, parts: ids });
}

/// Electric storms on far planets: distant lightning when a storm region is near, then rings on
/// the ground (1.6 s warning) followed by a strike wherever the player is in the open. Caves
/// (below the surface band) and bases are safe.
#[allow(clippy::too_many_arguments)]
pub fn storm(
    mut commands: Commands,
    time: Res<Time>,
    (mut game, mut ui, cine, sys): (ResMut<Game>, ResMut<Ui>, Res<Cine>, Res<Sys>),
    (vw, rt, hg): (Res<VoxelWorld>, Res<PlanetRt>, Res<HazGfx>),
    mut fx: ResMut<HazFx>,
    mut player: Query<(&Transform, &mut Player)>,
    mut strikes: Query<(Entity, &mut Strike)>,
    mut parts: Query<(&mut Transform, &mut Visibility), (With<StrikePart>, Without<Player>, Without<HazPart>)>,
) {
    let dt = time.delta_secs().min(0.05);
    fx.storm = false;
    let Ok((ptf, mut pl)) = player.single_mut() else { return };
    if cine.0.is_some() || ui.paused {
        return;
    }
    let ppos = ptf.translation;
    let pup = ppos.normalize();
    // animate strikes
    for (e, mut s) in &mut strikes {
        s.t += dt;
        let t = s.t;
        let up = s.pos.normalize();
        let tele = t < 1.6;
        if let Ok((mut tf, mut v)) = parts.get_mut(s.parts[0]) {
            *v = if tele && s.harmful { Visibility::Inherited } else { Visibility::Hidden };
            let k = 2.4 * (1.0 + 0.12 * (t * 14.0).sin());
            tf.scale = Vec3::new(k, 3.0, k);
        }
        if let Ok((_, mut v)) = parts.get_mut(s.parts[1]) {
            *v = if tele && s.harmful && t > 0.6 { Visibility::Inherited } else { Visibility::Hidden };
        }
        for k in [2, 3] {
            if let Ok((_, mut v)) = parts.get_mut(s.parts[k]) {
                *v = if !tele && t < 1.85 { Visibility::Inherited } else { Visibility::Hidden };
            }
        }
        if !tele && !s.done {
            s.done = true;
            let rel = ppos - s.pos;
            let hgt = rel.dot(up);
            let flat = (rel - up * hgt).length();
            if s.harmful && flat < 2.4 && hgt > -1.0 && hgt < 3.0 {
                fx.hurt(&mut game, HazKind::Storm, 25.0, 0.0, true);
                let out = (rel - up * hgt).normalize_or(any_perp(up));
                pl.vel += out * 6.0 + up * 4.0;
            } else if s.pos.distance(ppos) < 40.0 {
                fx.flash = fx.flash.max(0.25);
                fx.flash_col = [0.75, 0.85, 1.0];
            }
        }
        if t > 2.0 {
            commands.entity(e).despawn();
        }
    }
    if tier_of(game.planet, sys.0.home) != Tier::Far {
        return;
    }
    let depth = vw.gen.height(pup) - ppos.length();
    if zone_of(depth) != Zone::Surface {
        return;
    }
    let lvl = storm_level(&rt.def, pup);
    if lvl < STORM_NEAR {
        return;
    }
    let mut rng = Rng::new(game.time.to_bits() ^ 0x5707);
    // distant lightning tells a storm is near before it can hurt
    fx.ambient_next -= dt;
    if fx.ambient_next <= 0.0 {
        fx.ambient_next = rng.range(1.2, 2.8);
        let p = offset_on_surface(&vw, ppos, rng.range(18.0, 40.0), rng.range(0.0, TAU));
        spawn_strike(&mut commands, &hg, p, false);
    }
    if lvl < STORM_IN {
        return;
    }
    fx.storm = true;
    let l = game.lang;
    if !game.hazards_seen.contains(&HazKind::Storm) {
        game.hazards_seen.push(HazKind::Storm);
        ui.toast(tx(l, HazKind::Storm.warning()).to_string(), [1.0, 0.75, 0.35]);
    }
    let safe = ppos.distance(rt.landing) < 30.0 || game.structs().iter().any(|s| s.kind != SK::Tether && Vec3::from_array(s.pos).distance(ppos) < 16.0);
    if safe {
        return;
    }
    fx.storm_next -= dt;
    if fx.storm_next <= 0.0 {
        fx.storm_next = rng.range(2.6, 4.2);
        let flat_vel = pl.vel - pup * pl.vel.dot(pup);
        let target = if rng.f32() < 0.6 {
            ppos + flat_vel * 1.2 + (any_perp(pup) * rng.range(-1.0, 1.0) + pup.cross(any_perp(pup)) * rng.range(-1.0, 1.0))
        } else {
            let a = rng.range(0.0, TAU);
            let u = any_perp(pup);
            ppos + (u * a.cos() + pup.cross(u) * a.sin()) * rng.range(3.0, 7.0)
        };
        let tu = target.normalize();
        if let Some(g) = vw.raycast(target + tu * 6.0, -tu, 14.0) {
            spawn_strike(&mut commands, &hg, g, true);
        }
    }
}

// ---------------------------------------------------------------- dropped backpack bundles

#[derive(Component)]
pub struct BundleEnt(pub u32);

/// Shows the bundles of the current planet and gives them back when the player walks up.
#[allow(clippy::too_many_arguments)]
pub fn bundles(
    mut commands: Commands,
    hg: Res<HazGfx>,
    mut game: ResMut<Game>,
    mut ui: ResMut<Ui>,
    cine: Res<Cine>,
    player: Query<&Transform, With<Player>>,
    ents: Query<(Entity, &BundleEnt)>,
) {
    let p = game.planet;
    let here: Vec<(u32, Vec3)> = game.bundles.iter().filter(|b| b.planet == p).map(|b| (b.id, Vec3::from_array(b.pos))).collect();
    let mut shown: HashSet<u32> = HashSet::new();
    for (e, b) in &ents {
        if here.iter().any(|h| h.0 == b.0) {
            shown.insert(b.0);
        } else {
            commands.entity(e).despawn();
        }
    }
    for (id, pos) in &here {
        if shown.contains(id) {
            continue;
        }
        let up = pos.normalize();
        commands
            .spawn((Transform::from_translation(*pos).with_rotation(Quat::from_rotation_arc(Vec3::Y, up)), Visibility::Visible, BundleEnt(*id), PlanetScoped))
            .with_children(|c| {
                c.spawn((Mesh3d(hg.cube.clone()), MeshMaterial3d(hg.pack.clone()), Transform::from_xyz(0.0, 0.3, 0.0).with_rotation(Quat::from_rotation_y(0.5)).with_scale(Vec3::new(0.8, 0.6, 0.5))));
                c.spawn((Mesh3d(hg.cube.clone()), MeshMaterial3d(hg.strap.clone()), Transform::from_xyz(0.0, 0.38, 0.0).with_rotation(Quat::from_rotation_y(0.5)).with_scale(Vec3::new(0.84, 0.12, 0.54))));
                c.spawn((Mesh3d(hg.cyl.clone()), MeshMaterial3d(hg.beam.clone()), Transform::from_xyz(0.0, 8.0, 0.0).with_scale(Vec3::new(0.07, 16.0, 0.07)), NotShadowCaster));
            });
    }
    if cine.0.is_some() {
        return;
    }
    let Ok(ptf) = player.single() else { return };
    let l = game.lang;
    for (id, pos) in here {
        if pos.distance(ptf.translation) > 2.6 {
            continue;
        }
        let taken = recover_bundle(&mut game, id);
        for (item, n) in &taken {
            ui.pickup(*item, *n);
        }
        let left = game.bundles.iter().any(|b| b.id == id);
        if left {
            if ui.full_cd <= 0.0 {
                ui.full_cd = 3.0;
                ui.toast(tx(l, ("Sırt çantası dolu: eşyaların bir kısmı yerde kaldı", "Backpack full: some of your gear is still on the ground")).to_string(), [1.0, 0.6, 0.3]);
            }
        } else {
            ui.toast(tx(l, ("Eşyalarını geri aldın", "You recovered your gear")).to_string(), [0.6, 1.0, 0.6]);
        }
    }
}

// ---------------------------------------------------------------- HUD

fn col(c: [f32; 3], a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8, (a.clamp(0.0, 1.0) * 255.0) as u8)
}

fn vignette(painter: &egui::Painter, r: egui::Rect, c: [f32; 3], a: f32) {
    let w = r.width().min(r.height()) * 0.17;
    let (c0, c1) = (col(c, a), Color32::TRANSPARENT);
    let o = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()];
    let i = [o[0] + egui::vec2(w, w), o[1] + egui::vec2(-w, w), o[2] + egui::vec2(-w, -w), o[3] + egui::vec2(w, -w)];
    let mut m = egui::Mesh::default();
    for k in 0..4 {
        let k2 = (k + 1) % 4;
        let b = m.vertices.len() as u32;
        m.colored_vertex(o[k], c0);
        m.colored_vertex(o[k2], c0);
        m.colored_vertex(i[k2], c1);
        m.colored_vertex(i[k], c1);
        m.add_triangle(b, b + 1, b + 2);
        m.add_triangle(b, b + 2, b + 3);
    }
    painter.add(egui::Shape::mesh(m));
}

/// Damage vignette, hazard and bundle markers, storm / radiation readout, blackout fade.
#[allow(clippy::too_many_arguments)]
pub fn hud(
    mut contexts: EguiContexts,
    time: Res<Time>,
    ui_s: Res<Ui>,
    game: Res<Game>,
    rt: Res<PlanetRt>,
    cine: Res<Cine>,
    mut fx: ResMut<HazFx>,
    cam: Query<(&Camera, &GlobalTransform), With<MainCam>>,
    player: Query<&Transform, With<Player>>,
) {
    let Some(ctx) = contexts.try_ctx_mut() else { return };
    let ctx = ctx.clone();
    let l = ui_s.lang;
    let screen = ctx.screen_rect();
    let narrow = screen.width() < 900.0;
    fx.fade = (fx.fade - time.delta_secs()).max(0.0);
    let bg = ctx.layer_painter(egui::LayerId::background());
    if cine.0.is_none() {
        // the light dims under a storm
        if fx.storm {
            bg.rect_filled(screen, 0.0, Color32::from_rgba_unmultiplied(15, 18, 45, 80));
        }
        // damage flash, and a slow pulse while health is low
        let low = if game.health < 30.0 { (0.25 + 0.15 * (time.elapsed_secs() * 4.0).sin()) * (1.0 - game.health / 30.0) } else { 0.0 };
        if fx.flash > 0.01 {
            vignette(&bg, screen, fx.flash_col, fx.flash * 0.75);
        }
        if low > 0.01 {
            vignette(&bg, screen, [1.0, 0.1, 0.05], low);
        }
        if let (Ok((camera, cgt)), Ok(ptf)) = (cam.single(), player.single()) {
            let ppos = ptf.translation;
            let size = if narrow { 15.0 } else { 14.0 };
            for (pos, kind, danger) in &fx.marks {
                if let Ok(sp) = camera.world_to_viewport(cgt, *pos) {
                    let p = egui::pos2(sp.x, sp.y);
                    let c = if *danger { Color32::from_rgb(255, 90, 70) } else { Color32::from_rgb(255, 190, 70) };
                    let tri = vec![p + egui::vec2(0.0, -9.0), p + egui::vec2(8.0, 6.0), p + egui::vec2(-8.0, 6.0)];
                    bg.add(egui::Shape::convex_polygon(tri, c, Stroke::new(1.5_f32, Color32::from_black_alpha(170))));
                    bg.text(p + egui::vec2(0.0, 1.5), Align2::CENTER_CENTER, "!", FontId::proportional(11.0), Color32::BLACK);
                    bg.text(p + egui::vec2(0.0, -12.0), Align2::CENTER_BOTTOM, tx(l, kind.name(rt.def.kind)), FontId::proportional(size), c);
                }
            }
            // dropped gear: always marked, with an arrow on the screen edge when off-screen
            for b in game.bundles.iter().filter(|b| b.planet == game.planet) {
                let pos = Vec3::from_array(b.pos);
                let d = pos.distance(ppos);
                let label = format!("{} {:.0} m", tx(l, ("Eşyaların", "Your gear")), d);
                let c = Color32::from_rgb(255, 160, 50);
                let at = pos + pos.normalize() * 2.8;
                let local = cgt.affine().inverse().transform_point3(at);
                let on = camera.world_to_viewport(cgt, at).ok().filter(|sp| local.z < 0.0 && screen.contains(egui::pos2(sp.x, sp.y)));
                if let Some(sp) = on {
                    let p = egui::pos2(sp.x, sp.y);
                    let dia = vec![p + egui::vec2(0.0, -8.0), p + egui::vec2(8.0, 0.0), p + egui::vec2(0.0, 8.0), p + egui::vec2(-8.0, 0.0)];
                    bg.add(egui::Shape::convex_polygon(dia, c, Stroke::new(1.5_f32, Color32::from_black_alpha(170))));
                    bg.text(p + egui::vec2(0.0, -11.0), Align2::CENTER_BOTTOM, label, FontId::proportional(size + 1.0), c);
                } else {
                    let dir = egui::vec2(local.x, -local.y);
                    let dir = if dir.length() < 1e-3 { egui::vec2(0.0, 1.0) } else { dir.normalized() };
                    let ctr = screen.center();
                    let half = screen.size() * 0.5 - egui::vec2(46.0, 60.0);
                    let k = (half.x / dir.x.abs().max(1e-3)).min(half.y / dir.y.abs().max(1e-3));
                    let p = ctr + dir * k;
                    let n = egui::vec2(-dir.y, dir.x);
                    let tri = vec![p + dir * 12.0, p - dir * 6.0 + n * 9.0, p - dir * 6.0 - n * 9.0];
                    bg.add(egui::Shape::convex_polygon(tri, c, Stroke::new(1.5_f32, Color32::from_black_alpha(170))));
                    // label on the inner side of the arrow
                    let (anchor, off) = if dir.x > 0.5 {
                        (Align2::RIGHT_CENTER, egui::vec2(-12.0, 0.0))
                    } else if dir.x < -0.5 {
                        (Align2::LEFT_CENTER, egui::vec2(12.0, 0.0))
                    } else if dir.y > 0.0 {
                        (Align2::CENTER_BOTTOM, egui::vec2(0.0, -14.0))
                    } else {
                        (Align2::CENTER_TOP, egui::vec2(0.0, 14.0))
                    };
                    bg.text(p + off, anchor, label, FontId::proportional(size), c);
                }
            }
        }
        // storm / radiation readout above the bottom bar
        let mut lines: Vec<(String, Color32)> = Vec::new();
        if fx.storm {
            lines.push((tx(l, ("⚡ Elektrik fırtınası: açıkta kalma", "⚡ Electric storm: don't stay in the open")).to_string(), Color32::from_rgb(190, 200, 255)));
        }
        if fx.geiger > 0.0 {
            lines.push((format!("⚠ {} {:.0}%", tx(l, ("Radyasyon", "Radiation")), fx.geiger * 100.0), Color32::from_rgb(200, 255, 90)));
        }
        if !lines.is_empty() {
            let y = if cfg!(target_os = "android") { -150.0 } else { -130.0 };
            egui::Area::new("hazard_status".into()).anchor(Align2::CENTER_BOTTOM, [0.0, y]).interactable(false).show(&ctx, |ui| {
                egui::Frame::new().fill(Color32::from_black_alpha(215)).corner_radius(6.0).inner_margin(egui::Margin::symmetric(10, 4)).show(ui, |ui| {
                    for (text, c) in lines {
                        ui.add(egui::Label::new(egui::RichText::new(text).size(16.0).color(c)).extend());
                    }
                });
            });
        }
    }
    // blackout: a cut to black that fades out on the base, with MIRA's line
    if fx.fade > 0.0 {
        let a = (fx.fade / 1.4).min(1.0);
        let fg = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("blackout")));
        fg.rect_filled(screen, 0.0, Color32::from_black_alpha((a * 255.0) as u8));
        let ta = (fx.fade / 0.6).min(1.0);
        let size = if narrow { 20.0 } else { 26.0 };
        fg.text(screen.center() - egui::vec2(0.0, 14.0), Align2::CENTER_CENTER, &fx.fade_text, FontId::proportional(size), col([1.0, 0.75, 0.4], ta));
        fg.text(screen.center() + egui::vec2(0.0, 18.0), Align2::CENTER_CENTER, &fx.fade_sub, FontId::proportional(size * 0.65), col([0.85, 0.85, 0.9], ta));
    }
}

/// Turns the astronaut towards `to`; the camera swings `side` radians around so the target is
/// not hidden behind the astronaut.
fn face(rig: &mut CamRig, pl: &mut Player, from: Vec3, to: Vec3, side: f32) {
    let u = from.normalize();
    let f = (to - from).reject_from(u).normalize_or(any_perp(u));
    rig.heading = Quat::from_axis_angle(u, side) * f;
    pl.facing = f;
    rig.pitch = if side != 0.0 { 0.6 } else { 0.45 };
    if side != 0.0 {
        rig.dist = 10.0;
    }
}

/// Movement input that walks towards `target` whatever way the camera looks.
fn steer(rig: &CamRig, ppos: Vec3, target: Option<Vec3>, speed: f32) -> Vec2 {
    let up = ppos.normalize();
    let Some(f) = target.and_then(|t| (t - ppos).reject_from(up).try_normalize()) else { return Vec2::new(0.0, speed) };
    let heading = rig.heading.reject_from(up).normalize_or(any_perp(up));
    let right = heading.cross(up);
    Vec2::new(f.dot(right), f.dot(heading)) * speed
}

/// Bilingual text for what knocked the player out.
pub fn cause_text(fx: &HazFx, game: &Game, pk: PlanetKind) -> String {
    let l = game.lang;
    let what = match fx.cause {
        Some(k) if game.time - fx.cause_t < 4.0 => tx(l, k.name(pk)).to_string(),
        _ if game.oxygen <= 0.0 => tx(l, ("oksijen bitti", "oxygen ran out")).to_string(),
        _ => tx(l, ("sert bir düşüş", "a hard fall")).to_string(),
    };
    format!("{}: {}", tx(l, ("Sebep", "Cause")), what)
}

// ---------------------------------------------------------------- bot runs

fn haz_from_env() -> Option<HazKind> {
    let v = std::env::var("LOSTRANOUT_HAZ").ok()?;
    HazKind::ALL.into_iter().find(|k| format!("{k:?}").eq_ignore_ascii_case(&v))
}

/// LOSTRANOUT_BOT=hazard (LOSTRANOUT_HAZ=<kind>): puts one hazard of that kind 7 m ahead and
/// walks into it. LOSTRANOUT_BOT=hazcave: jumps next to the nearest naturally placed hazard
/// (optionally of kind LOSTRANOUT_HAZ), faces it and walks in. LOSTRANOUT_BOT=blackout: walks
/// away from the pod with a full backpack, blacks out in an acid pool, then goes back for the
/// gear. LOSTRANOUT_BOT=storm: jumps into the nearest storm region of a far planet.
#[allow(clippy::too_many_arguments)]
pub fn debug_bot(
    time: Res<Time>,
    mut c: ResMut<Controls>,
    mut rig: ResMut<CamRig>,
    mut fx: ResMut<HazFx>,
    mut game: ResMut<Game>,
    (vw, rt): (Res<VoxelWorld>, Res<PlanetRt>),
    mut player: Query<(&mut Transform, &mut Player)>,
    hz: Query<&HazEnt>,
    mut st: Local<(Option<f32>, u32, f32, Option<Vec3>)>,
    (mut ui, cam): (ResMut<Ui>, Query<(&Camera, &GlobalTransform), With<MainCam>>),
) {
    let Ok(mode) = std::env::var("LOSTRANOUT_BOT") else { return };
    if !matches!(mode.as_str(), "hazard" | "hazcave" | "blackout" | "storm") {
        return;
    }
    let Ok((mut ptf, mut pl)) = player.single_mut() else { return };
    let start = *st.0.get_or_insert(time.elapsed_secs());
    let t = time.elapsed_secs() - start;
    let ppos = ptf.translation;
    let up = ppos.normalize();
    let walk_from: f32 = std::env::var("LOSTRANOUT_WALKAT").ok().and_then(|s| s.parse().ok()).unwrap_or(3.0);
    match mode.as_str() {
        "hazard" => {
            if st.1 == 0 && t > 1.0 {
                st.1 = 1;
                let kind = haz_from_env().unwrap_or(HazKind::Vent);
                // 30 m out from the landing site, looking further out, the hazard 7 m ahead
                let ang: f32 = std::env::var("LOSTRANOUT_ANGLE").ok().and_then(|s| s.parse().ok()).unwrap_or(4.0);
                let from = offset_on_surface(&vw, rt.landing, 30.0, ang);
                let pos = offset_on_surface(&vw, rt.landing, 37.0, ang);
                ptf.translation = from + from.normalize() * 0.05;
                pl.vel = Vec3::ZERO;
                fx.debug_spawn.push((kind, pos, 0.5));
                face(&mut *rig, &mut *pl, from, pos, 0.8);
                st.3 = Some(pos);
                let _ = (ppos, up);
                eprintln!("[bot] hazard {:?} placed 7 m ahead", kind);
            }
            if st.1 == 1 && t > walk_from {
                if std::env::var("LOSTRANOUT_DIG").is_ok() {
                    // dig it out from where we stand instead of walking in
                    ui.tool_mode = ToolMode::Dig;
                    if let (Some(target), Ok((camera, gt))) = (st.3, cam.single()) {
                        c.pointer = camera.world_to_viewport(gt, target + target.normalize() * 0.4).ok();
                        c.tool = c.pointer.is_some();
                    }
                } else {
                    c.mv = steer(&rig, ppos, st.3, 0.55);
                }
            }
        }
        "hazcave" => {
            let want = haz_from_env();
            if st.1 == 0 && t > 2.0 {
                let surface = std::env::var("LOSTRANOUT_ZONE").as_deref() == Ok("surface");
                let best = hz.iter().filter(|h| want.is_none_or(|k| h.h.kind == k) && (!surface || h.h.zone == Zone::Surface)).min_by(|a, b| a.h.pos.distance(ppos).total_cmp(&b.h.pos.distance(ppos)));
                if let Some(h) = best {
                    // stand 6 m away on the same floor, on the side with the most room
                    let hu = h.h.pos.normalize();
                    let u = any_perp(hu);
                    let v = hu.cross(u);
                    let mut spot = None;
                    for k in 0..16 {
                        let a = k as f32 / 16.0 * TAU;
                        let p = h.h.pos + (u * a.cos() + v * a.sin()) * 6.0 + hu * 1.5;
                        if [0.0, 0.8, 1.6].iter().all(|o| vw.sample(p + hu * *o) < 0.0) {
                            if let Some(g) = vw.raycast(p, -hu, 4.0) {
                                spot = Some(g + hu * 0.05);
                                break;
                            }
                        }
                    }
                    if let Some(s) = spot {
                        ptf.translation = s;
                        pl.vel = Vec3::ZERO;
                        face(&mut *rig, &mut *pl, s, h.h.pos, 0.8);
                        if h.h.zone != Zone::Surface {
                            rig.dist = 5.5;
                            rig.pitch = 0.25;
                        }
                        st.3 = Some(h.h.pos);
                        st.1 = 1;
                        st.2 = t;
                        eprintln!("[bot] at {:?} {:?} depth {:.0}, {:.1} m away", h.h.kind, h.h.zone, h.h.depth, s.distance(h.h.pos));
                    }
                } else if t > 12.0 {
                    eprintln!("[bot] no hazard streamed in");
                    st.1 = 9;
                }
            }
            if st.1 == 1 && t > st.2 + walk_from {
                c.mv = steer(&rig, ppos, st.3, 0.5);
            }
        }
        "blackout" => {
            if st.1 == 0 && t > 1.0 {
                st.1 = 1;
                let p = offset_on_surface(&vw, rt.landing, 38.0, 2.2);
                ptf.translation = p + p.normalize() * 0.05;
                pl.vel = Vec3::ZERO;
                game.health = 25.0;
                let ahead = offset_on_surface(&vw, rt.landing, 44.0, 2.2);
                fx.debug_spawn.push((HazKind::Pool, ahead, 0.4));
                face(&mut *rig, &mut *pl, p, ahead, 0.0);
                eprintln!("[bot] 38 m from the pod with {} stacks, walking into an acid pool", game.inv.slots.iter().flatten().count());
            }
            if st.1 == 1 {
                c.mv = Vec2::new(0.0, if t > 2.5 { 0.6 } else { 0.0 });
                if game.deaths > 0 {
                    st.1 = 2;
                    st.2 = t;
                    eprintln!("[bot] blacked out, now {:.1} m from the pod; bundles {:?}", ppos.distance(rt.landing), game.bundles.iter().map(|b| (b.id, b.items.len())).collect::<Vec<_>>());
                }
            }
            let back: f32 = std::env::var("LOSTRANOUT_BACK").ok().and_then(|s| s.parse().ok()).unwrap_or(7.0);
            if st.1 == 2 && t > st.2 + back {
                if let Some(b) = game.bundles.first() {
                    let bp = Vec3::from_array(b.pos);
                    // 6 m short of the bundle, on the pod side, looking at it
                    let towards = (rt.landing - bp).reject_from(bp.normalize()).normalize_or(any_perp(bp.normalize()));
                    let dir = (bp + towards * 6.0).normalize();
                    let s = vw.surface_point(dir).unwrap_or(dir * bp.length());
                    ptf.translation = s + s.normalize() * 0.05;
                    pl.vel = Vec3::ZERO;
                    game.health = 100.0;
                    face(&mut *rig, &mut *pl, s, bp, 0.0);
                    st.1 = 3;
                    st.2 = t;
                }
            }
            if st.1 == 3 {
                c.mv = Vec2::new(0.0, if t > st.2 + 2.5 { 0.5 } else { 0.0 });
                if game.bundles.is_empty() {
                    eprintln!("[bot] gear recovered: {} stacks", game.inv.slots.iter().flatten().count());
                    st.1 = 4;
                }
            }
        }
        "storm" => {
            if st.1 == 0 && t > 1.0 {
                st.1 = 1;
                let mut rng = Rng::new(7);
                let mut best: Option<(f32, Vec3)> = None;
                for _ in 0..4000 {
                    let d = rng.dir();
                    let lv = storm_level(&rt.def, d);
                    let dist = (d * rt.def.radius).distance(rt.landing);
                    if lv > STORM_IN + 0.08 && dist > 40.0 && best.is_none_or(|b| dist < b.0) {
                        best = Some((dist, d));
                    }
                }
                match best {
                    Some((dist, d)) => {
                        let s = vw.surface_point(d).unwrap_or(d * rt.def.radius);
                        ptf.translation = s + s.normalize() * 0.05;
                        pl.vel = Vec3::ZERO;
                        rig.pitch = 0.6;
                        eprintln!("[bot] in a storm region {:.0} m from the landing site", dist);
                    }
                    None => eprintln!("[bot] no storm region on this planet"),
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::StarSystem;
    use crate::items::Lang;
    use crate::voxel::Brush;

    #[test]
    fn zones_follow_the_shared_depth_bands() {
        assert_eq!(zone_of(0.0), Zone::Surface);
        assert_eq!(zone_of(3.9), Zone::Surface);
        assert_eq!(zone_of(4.0), Zone::Upper);
        assert_eq!(zone_of(39.9), Zone::Upper);
        assert_eq!(zone_of(40.0), Zone::Deep);
        assert_eq!(zone_of(110.0), Zone::Deep);
        assert_eq!(tier_of(2, 2), Tier::Home);
        assert_eq!(tier_of(1, 2), Tier::Near);
        assert_eq!(tier_of(3, 2), Tier::Near);
        assert_eq!(tier_of(0, 2), Tier::Far);
        assert_eq!(tier_of(5, 2), Tier::Far);
    }

    #[test]
    fn owner_rule_zones_by_tier() {
        use Tier::*;
        use Zone::*;
        let table = [(Home, [false, false, true]), (Near, [false, true, true]), (Far, [true, true, true])];
        for (tier, want) in table {
            for (z, w) in [Surface, Upper, Deep].into_iter().zip(want) {
                assert_eq!(allowed(tier, z), w, "{tier:?} {z:?}");
            }
        }
    }

    #[test]
    fn every_planet_kind_has_hazards_in_every_zone_it_can_get() {
        use PlanetKind::*;
        for pk in [Temperate, Desert, Ice, Volcanic, Barren, Toxic, Crystal] {
            for z in [Zone::Surface, Zone::Upper, Zone::Deep] {
                let k = kinds_for(pk, true, z);
                assert!(!k.is_empty(), "{pk:?} {z:?}");
                assert!(z != Zone::Surface || k.iter().all(|x| x.0 != HazKind::Rockfall), "no ceiling on the surface");
            }
            assert!(kinds_for(pk, false, Zone::Upper).iter().any(|x| x.0 == HazKind::Rockfall));
            assert!(kinds_for(pk, false, Zone::Deep).iter().all(|x| x.0 != HazKind::Radiation), "radiation only where uranite exists");
        }
        assert!(kinds_for(Volcanic, false, Zone::Surface).iter().any(|x| x.0 == HazKind::Vent));
        assert!(kinds_for(Toxic, false, Zone::Upper).iter().any(|x| x.0 == HazKind::Spores));
    }

    /// Places hazards on every tier of a real planet and checks the zone of each one, plus
    /// determinism and the landing-site exclusion.
    fn sweep(def: &PlanetDef, tier: Tier, vw: &VoxelWorld, avoid: &[(Vec3, f32)]) -> Vec<Hazard> {
        let g = crate::decor::cells_per_edge(def.radius);
        let mut all = Vec::new();
        for cell in (0..6 * g * g).step_by(7) {
            let a = cell_hazards(def, tier, vw, g, cell, avoid);
            let b = cell_hazards(def, tier, vw, g, cell, avoid);
            assert_eq!(a.len(), b.len());
            for (x, y) in a.iter().zip(&b) {
                assert_eq!((x.id, x.kind, x.pos), (y.id, y.kind, y.pos), "placement must be deterministic");
                assert_eq!(x.id / PER_CELL, cell);
            }
            all.extend(a);
        }
        for h in &all {
            let depth = vw.gen.height(h.pos.normalize()) - h.pos.length();
            assert_eq!(zone_of(depth), h.zone);
            assert!(allowed(tier, h.zone), "{:?} hazard in {:?} at depth {depth:.1}", tier, h.zone);
            assert!(avoid.iter().all(|(c, r)| c.distance(h.pos) >= *r));
        }
        all
    }

    #[test]
    fn placement_respects_zones_and_tiers_on_real_planets() {
        let sys = StarSystem::generate(9, 0);
        let def = &sys.planets[sys.home];
        let vw = VoxelWorld::new(def);
        let landing = vw.surface_point(Vec3::X).unwrap();
        let avoid = [(landing, 28.0)];
        let home = sweep(def, Tier::Home, &vw, &avoid);
        let near = sweep(def, Tier::Near, &vw, &avoid);
        let far = sweep(def, Tier::Far, &vw, &avoid);
        let count = |v: &[Hazard], z: Zone| v.iter().filter(|h| h.zone == z).count();
        eprintln!("home {} deep / near {} upper {} deep / far {} surface {} upper {} deep", count(&home, Zone::Deep), count(&near, Zone::Upper), count(&near, Zone::Deep), count(&far, Zone::Surface), count(&far, Zone::Upper), count(&far, Zone::Deep));
        assert!(count(&far, Zone::Surface) > 10, "far planets need surface hazards");
        assert!(count(&near, Zone::Upper) > 5, "near planets need upper-cave hazards");
        assert_eq!(count(&home, Zone::Upper) + count(&home, Zone::Surface), 0);
    }

    #[test]
    fn home_planet_gets_hazards_in_a_deep_pocket() {
        let sys = StarSystem::generate(9, 0);
        let def = &sys.planets[sys.home];
        let mut vw = VoxelWorld::new(def);
        let g = crate::decor::cells_per_edge(def.radius);
        // dig a big pocket 60 m down under some cell, then every try in that cell finds a floor
        let found = (0..6 * g * g).step_by(5).find_map(|cell| {
            let face = cell / (g * g);
            let (j, i) = ((cell / g) % g, cell % g);
            let dir = cube_dir(face, (i as f32 + 0.5) / g as f32 * 2.0 - 1.0, (j as f32 + 0.5) / g as f32 * 2.0 - 1.0);
            let c = dir * (vw.gen.height(dir) - 60.0);
            if c.length() < vw.gen.core + 20.0 {
                return None;
            }
            for _ in 0..4 {
                vw.edit(c, 9.0, &Brush::Dig, 2.0);
            }
            let hs = cell_hazards(def, Tier::Home, &vw, g, cell, &[]);
            (!hs.is_empty()).then_some(hs)
        });
        let hs = found.expect("a deep pocket must get a hazard");
        assert!(hs.iter().all(|h| h.zone == Zone::Deep));
    }

    #[test]
    fn storms_exist_but_do_not_cover_everything() {
        let sys = StarSystem::generate(9, 0);
        let def = &sys.planets[0];
        let mut rng = Rng::new(3);
        let n = 2000;
        let inside = (0..n).filter(|_| storm_level(def, rng.dir()) > STORM_IN).count();
        assert!(inside > n / 20 && inside < n / 2, "storm coverage {inside}/{n}");
    }

    fn sphere(r: f32) -> impl Fn(Vec3) -> f32 {
        move |p: Vec3| (r - p.length()).clamp(-1.0, 1.0)
    }

    #[test]
    fn blackout_moves_the_backpack_into_a_bundle_and_back() {
        let mut g = Game::new(5, Lang::En);
        g.inv.add(Item::IronOre, 25);
        g.inv.add(Item::Quartz, 3);
        g.soil = 42.0;
        g.data = 300;
        let id = drop_bundle(&mut g, Vec3::new(0.0, 100.0, 0.0)).unwrap();
        assert!(g.inv.slots.iter().all(|s| s.is_none()), "backpack must be empty");
        assert_eq!(g.inv.slots.len(), 16, "slot count is kept");
        assert_eq!((g.soil, g.data), (42.0, 300), "soil and data stay with the player");
        assert_eq!(g.bundles.len(), 1);
        assert_eq!(g.bundles[0].items, vec![(Item::IronOre, 25), (Item::Quartz, 3)]);
        assert_eq!(g.bundles[0].planet, g.planet);
        assert!(drop_bundle(&mut g, Vec3::ZERO).is_none(), "an empty backpack leaves no bundle");
        // a second blackout makes a second bundle
        g.inv.add(Item::Carbon, 2);
        let id2 = drop_bundle(&mut g, Vec3::new(0.0, 0.0, 100.0)).unwrap();
        assert_eq!(g.bundles.len(), 2);
        // recovery; a full backpack takes only what fits and the rest stays
        for k in 0..16 {
            g.inv.slots[k] = Some((Item::Scrap, 40));
        }
        g.inv.slots[15] = Some((Item::IronOre, 30));
        let taken = recover_bundle(&mut g, id);
        assert_eq!(taken, vec![(Item::IronOre, 10)]);
        assert_eq!(g.bundles.iter().find(|b| b.id == id).unwrap().items, vec![(Item::IronOre, 15), (Item::Quartz, 3)]);
        for k in 0..16 {
            g.inv.slots[k] = None;
        }
        recover_bundle(&mut g, id);
        assert_eq!(g.inv.count(Item::IronOre), 15);
        assert_eq!(g.inv.count(Item::Quartz), 3);
        assert!(g.bundles.iter().all(|b| b.id != id), "an emptied bundle disappears");
        recover_bundle(&mut g, id2);
        assert!(g.bundles.is_empty());
        // bundles persist in the save
        g.inv.add(Item::Iron, 1);
        drop_bundle(&mut g, Vec3::ONE);
        let bytes = bincode::serialize(&g).unwrap();
        let back: Game = bincode::deserialize(&bytes).unwrap();
        assert_eq!(back.bundles, g.bundles);
    }

    #[test]
    fn bundle_is_left_outside_the_hazard_that_knocked_you_out() {
        let r = 100.0;
        let ground = |p: Vec3| Some(p.normalize() * r);
        let pool = (Vec3::new(0.0, r, 0.0), 2.5);
        let fell = Vec3::new(0.5, r, 0.0);
        let b = bundle_spot(fell, &[pool], &ground);
        let flat = (b - pool.0).reject_from(Vec3::Y).length();
        assert!(flat > 2.5 + 0.8, "outside the pool: {flat}");
        assert!(b.x > 0.0, "on the side the player fell on");
        assert!((b.length() - r).abs() < 1e-3, "on the ground");
        let away = Vec3::new(0.0, r, 9.0);
        assert_eq!(bundle_spot(away, &[pool], &ground), away, "no hazard: right where the player fell");
    }

    #[test]
    fn base_anchor_prefers_pod_at_home_then_shuttle_then_oxygen() {
        let s = |id, kind, x: f32| Structure::new(id, kind, [x, 100.0, 0.0], [0.0, 0.0, 0.0, 1.0]);
        let landing = Vec3::new(0.0, 100.0, 0.0);
        let v = vec![s(1, SK::Smelter, 3.0), s(2, SK::Pod, 0.0), s(3, SK::Shuttle, 11.0), s(4, SK::Beacon, 20.0)];
        assert_eq!(base_anchor(&v, true, landing).0.x, 0.0);
        assert_eq!(base_anchor(&v, false, landing).0.x, 11.0);
        let v = vec![s(1, SK::Smelter, 3.0), s(4, SK::Beacon, 20.0), s(5, SK::Beacon, 60.0), s(6, SK::Tether, 1.0)];
        assert_eq!(base_anchor(&v, false, landing).0.x, 20.0);
        assert_eq!(base_anchor(&[], false, landing).0, landing);
    }

    #[test]
    fn respawn_is_on_the_ground_next_to_the_base() {
        let r = 100.0;
        let pod = Structure::new(1, SK::Pod, [0.0, r, 0.0], [0.0, 0.0, 0.0, 1.0]);
        let smelter = Structure::new(2, SK::Smelter, [3.6, r, 0.0], [0.0, 0.0, 0.0, 1.0]);
        let structs = vec![pod.clone(), smelter];
        let anchor = Vec3::from_array(pod.pos);
        let d = sphere(r);
        let p = respawn_spot(&d, anchor, 2.2, &structs);
        assert!((p.length() - r).abs() < 0.2, "on the ground, not in the air: {}", p.length());
        assert!(p.distance(anchor) < 2.2 + 4.0, "next to the pod: {}", p.distance(anchor));
        assert!(p.distance(anchor) > 2.2 * 0.8, "not inside the pod");
        assert!(p.distance(Vec3::new(3.6, r, 0.0)) > 1.4 * 0.8 + 0.5, "not inside the smelter");
        // a base in a hollow under a thick roof (terrain far above a cave): never on the roof
        let roofed = |q: Vec3| {
            let l = q.length();
            if l > r + 6.0 && l < r + 14.0 {
                1.0
            } else {
                d(q)
            }
        };
        let p = respawn_spot(&roofed, anchor, 2.2, &structs);
        assert!((p.length() - r).abs() < 0.2, "under the roof, on the floor: {}", p.length());
        // a deep pit in front of the pod on one side: still a spot at the pod's level
        let pit = |q: Vec3| if q.x < -1.0 && q.y > r - 20.0 { -1.0 } else { d(q) };
        let p = respawn_spot(&pit, anchor, 2.2, &structs);
        assert!((p.length() - r).abs() < 0.3 && p.x >= -1.0, "not dropped into the pit: {p}");
    }

    #[test]
    fn respawn_on_a_real_planet_lands_on_the_pod_terrain() {
        let sys = StarSystem::generate(4, 0);
        let def = &sys.planets[sys.home];
        let vw = VoxelWorld::new(def);
        let dir = Vec3::new(0.3, 1.0, 0.2).normalize();
        let ground = vw.surface_point(dir).unwrap();
        let pod = Structure::new(1, SK::Pod, ground.to_array(), [0.0, 0.0, 0.0, 1.0]);
        let p = respawn_spot(&|q| vw.sample(q), ground, 2.2, &[pod]);
        let up = p.normalize();
        assert!(vw.sample(p - up * 0.2) > 0.0, "standing on ground");
        assert!(vw.sample(p + up * 1.0) < 0.0, "head in the open");
        assert!(p.distance(ground) < 10.0);
    }
}
