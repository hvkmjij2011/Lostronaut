//! World visuals for cables, tether chains, the backpack oxygen line and machine status lights.
//!
//! Cost model: the wanted set of wires is recomputed every frame from save data (cheap), but a
//! wire mesh is only built when the wire appears, its ends move, or it comes near the player for
//! the first time (to lay it over the terrain). Power / live changes only swap the material.
//! The backpack line and the placement preview are one unit tube each, moved by its transform.
use super::{Breath, Grid, Status, STRETCH, TETHER_BREATH};
use crate::gameplay::Ghost;
use crate::items::SK;
use crate::player::Player;
use crate::state::{Game, Structure};
use crate::util::any_perp;
use bevy::render::mesh::MeshAabb;
use crate::voxel::VoxelWorld;
use crate::world::PlanetScoped;
use bevy::asset::RenderAssetUsages;
use bevy::pbr::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use std::collections::HashMap;

/// Terrain is consulted only this close to the player (it may not exist further away).
const LIFT_RANGE: f32 = 70.0;
const CABLE_R: f32 = 0.035;
const TETHER_R: f32 = 0.025;

#[derive(Resource)]
pub struct NetGfx {
    cable_live: Handle<StandardMaterial>,
    cable_dead: Handle<StandardMaterial>,
    tether_live: Handle<StandardMaterial>,
    tether_dead: Handle<StandardMaterial>,
    tether_warn: Handle<StandardMaterial>,
    lamp_ok: Handle<StandardMaterial>,
    lamp_low: Handle<StandardMaterial>,
    lamp_off: Handle<StandardMaterial>,
    unit_tube: Handle<Mesh>,
    bulb: Handle<Mesh>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum WireKind {
    Cable,
    Tether,
}

#[derive(Component)]
pub struct Wire {
    key: (WireKind, u32, u32),
    ends: (Vec3, Vec3),
    lifted: bool,
    live: bool,
}

#[derive(Component)]
pub struct Lamp(u32, Status);

#[derive(Component)]
pub struct Backline;

#[derive(Component)]
pub struct Preview;

/// Height above the base where cables attach. Tuned to the placeholder meshes.
pub fn anchor_height(k: SK) -> f32 {
    match k {
        SK::Pylon => 2.3,
        SK::Tether => 1.45,
        SK::Pod => 0.45,
        SK::Beacon => 0.6,
        _ => 0.3,
    }
}

/// Height of the status light above a machine: just over the top of its model (read from the
/// mesh bounds once per kind, so it follows model changes), 2.5 m when the mesh is unknown.
fn lamp_height(k: SK, cache: &mut HashMap<SK, f32>, gfx: &crate::world::Gfx, meshes: &Assets<Mesh>) -> f32 {
    *cache.entry(k).or_insert_with(|| {
        gfx.struct_mesh.get(&k).and_then(|h| meshes.get(h)).and_then(|m| m.compute_aabb()).map(|a| a.max().y + 0.35).unwrap_or(2.5)
    })
}

fn anchor(s: &Structure) -> Vec3 {
    let p = Vec3::from_array(s.pos);
    p + p.normalize() * anchor_height(s.kind)
}

pub fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>) {
    let mut lamp = |c: Color| mats.add(StandardMaterial { base_color: c, unlit: true, ..default() });
    let (lamp_ok, lamp_low, lamp_off) = (lamp(Color::srgb(0.2, 1.0, 0.3)), lamp(Color::srgb(1.0, 0.65, 0.1)), lamp(Color::srgb(1.0, 0.12, 0.08)));
    let mut m = |c: Color, emissive: LinearRgba| {
        mats.add(StandardMaterial { base_color: c, emissive, perceptual_roughness: 0.6, cull_mode: None, ..default() })
    };
    let gfx = NetGfx {
        cable_live: m(Color::srgb(0.95, 0.6, 0.15), LinearRgba::rgb(1.6, 0.7, 0.1)),
        cable_dead: m(Color::srgb(0.12, 0.12, 0.13), LinearRgba::BLACK),
        tether_live: m(Color::srgb(0.6, 0.95, 1.0), LinearRgba::rgb(0.5, 1.4, 2.0)),
        tether_dead: m(Color::srgb(0.45, 0.2, 0.2), LinearRgba::rgb(0.25, 0.02, 0.02)),
        tether_warn: m(Color::srgb(1.0, 0.65, 0.2), LinearRgba::rgb(2.0, 0.8, 0.1)),
        lamp_ok,
        lamp_low,
        lamp_off,
        unit_tube: meshes.add(tube(&[Vec3::ZERO, Vec3::Y], 1.0, Vec3::Z)),
        bulb: meshes.add(Sphere::new(0.2).mesh().ico(2).unwrap()),
    };
    commands.insert_resource(gfx);
}

/// Square tube along a polyline (world space). `up` orients the cross-section.
fn tube(points: &[Vec3], r: f32, up: Vec3) -> Mesh {
    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut nor: Vec<[f32; 3]> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    let n = points.len();
    for i in 0..n {
        let t = if i + 1 < n { points[i + 1] - points[i] } else { points[i] - points[i - 1] }.normalize_or(Vec3::X);
        let local_up = if up == Vec3::Z { up } else { points[i].normalize_or(up) };
        let side = t.cross(local_up).try_normalize().unwrap_or_else(|| any_perp(t));
        let n2 = side.cross(t).normalize();
        for d in [side, n2, -side, -n2] {
            pos.push((points[i] + d * r).to_array());
            nor.push(d.to_array());
        }
    }
    for i in 0..n.saturating_sub(1) as u32 {
        for k in 0..4u32 {
            let k1 = (k + 1) % 4;
            let (a, b, c, d) = (i * 4 + k, i * 4 + k1, (i + 1) * 4 + k1, (i + 1) * 4 + k);
            idx.extend_from_slice(&[a, b, c, a, c, d]);
        }
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
    mesh.insert_indices(Indices::U32(idx));
    mesh
}

/// Points of a wire from `a` to `b`: a slight catenary that is pushed out of the ground where
/// terrain data is available (near the player); elsewhere a straight segment.
fn wire_points(a: Vec3, b: Vec3, vw: Option<&VoxelWorld>, near: Vec3, slack: f32) -> (Vec<Vec3>, bool) {
    let len = a.distance(b);
    let n = ((len / 0.7) as usize).clamp(4, 48);
    let lift = vw.is_some() && (a.distance(near) < LIFT_RANGE || b.distance(near) < LIFT_RANGE);
    let sag = if lift { (len * slack).min(2.0) } else { 0.0 };
    let mut pts = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let t = i as f32 / n as f32;
        let mut p = a.lerp(b, t);
        let up = p.normalize();
        p -= up * sag * 4.0 * t * (1.0 - t);
        if let (true, Some(vw)) = (lift && i > 0 && i < n, vw) {
            if p.distance(near) < LIFT_RANGE + 20.0 {
                if let Some(hit) = vw.raycast(p + up * 3.0, -up, 3.5) {
                    let floor = hit.length() + 0.05;
                    if p.length() < floor {
                        p = up * floor;
                    }
                }
            }
        }
        pts.push(p);
    }
    (pts, lift)
}

#[allow(clippy::too_many_arguments)]
pub fn sync_wires(
    mut commands: Commands,
    game: Res<Game>,
    grid: Res<Grid>,
    gfx: Res<NetGfx>,
    vw: Option<Res<VoxelWorld>>,
    time: Res<Time>,
    world_gfx: Res<crate::world::Gfx>,
    mut heights: Local<HashMap<SK, f32>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut wires: Query<(Entity, &mut Wire, &Mesh3d, &mut MeshMaterial3d<StandardMaterial>)>,
    mut lamps: Query<(Entity, &mut Lamp, &mut Transform, &mut MeshMaterial3d<StandardMaterial>), Without<Wire>>,
    player: Query<&Transform, (With<Player>, Without<Lamp>)>,
) {
    let structs = game.structs();
    let near = player.single().map(|t| t.translation).unwrap_or(Vec3::ZERO);
    let by_id: HashMap<u32, &Structure> = structs.iter().map(|s| (s.id, s)).collect();

    // ---- wanted wires
    let mut want: HashMap<(WireKind, u32, u32), (Vec3, Vec3, bool)> = HashMap::new();
    for s in structs {
        for &l in &s.links {
            if s.id < l {
                if let Some(o) = by_id.get(&l) {
                    let live = grid.nets.of(s.id).map(|n| n.powered()).unwrap_or(false);
                    want.insert((WireKind::Cable, s.id, l), (anchor(s), anchor(o), live));
                }
            }
        }
    }
    for p in &grid.posts {
        let Some(parent) = p.parent else { continue };
        let Some(o) = by_id.get(&parent) else { continue };
        let Some(me) = by_id.get(&p.id) else { continue };
        // dead posts may point at each other: draw each pair once
        let key = if o.kind == SK::Tether && !p.live { (WireKind::Tether, p.id.min(parent), p.id.max(parent)) } else { (WireKind::Tether, p.id, parent) };
        let end = if o.kind == SK::Tether { anchor(o) } else { Vec3::from_array(o.pos) + Vec3::from_array(o.pos).normalize() * 1.0 };
        want.insert(key, (anchor(me), end, p.live));
    }

    // ---- update / remove existing ones; at most one mesh rebuild per frame
    let mut rebuilt = false;
    for (e, mut w, mesh, mut mat) in &mut wires {
        let Some(&(a, b, live)) = want.get(&w.key) else {
            meshes.remove(&mesh.0);
            commands.entity(e).despawn();
            continue;
        };
        want.remove(&w.key);
        let moved = w.ends.0.distance(a) > 0.01 || w.ends.1.distance(b) > 0.01;
        let relay = !w.lifted && !rebuilt && vw.is_some() && (a.distance(near) < LIFT_RANGE * 0.8 || b.distance(near) < LIFT_RANGE * 0.8);
        if moved || relay {
            rebuilt = true;
            let (pts, lifted) = wire_points(a, b, vw.as_deref(), near, if w.key.0 == WireKind::Cable { 0.06 } else { 0.03 });
            meshes.remove(&mesh.0);
            let r = if w.key.0 == WireKind::Cable { CABLE_R } else { TETHER_R };
            commands.entity(e).insert(Mesh3d(meshes.add(tube(&pts, r, Vec3::Y)))).remove::<bevy::render::primitives::Aabb>();
            w.ends = (a, b);
            w.lifted = lifted;
        }
        if w.live != live || moved {
            w.live = live;
            mat.0 = wire_mat(&gfx, w.key.0, live);
        }
    }
    for (key, (a, b, live)) in want {
        let (pts, lifted) = wire_points(a, b, vw.as_deref(), near, if key.0 == WireKind::Cable { 0.06 } else { 0.03 });
        let r = if key.0 == WireKind::Cable { CABLE_R } else { TETHER_R };
        commands.spawn((
            Mesh3d(meshes.add(tube(&pts, r, Vec3::Y))),
            MeshMaterial3d(wire_mat(&gfx, key.0, live)),
            Transform::IDENTITY,
            NotShadowCaster,
            NotShadowReceiver,
            Wire { key, ends: (a, b), lifted, live },
            PlanetScoped,
        ));
    }

    // ---- status lights on machines
    let blink = (time.elapsed_secs() * 3.0).sin() > -0.2;
    let mut have: Vec<u32> = Vec::new();
    for (e, mut lamp, mut tf, mut mat) in &mut lamps {
        let Some(s) = by_id.get(&lamp.0) else {
            commands.entity(e).despawn();
            continue;
        };
        have.push(lamp.0);
        let st = grid.nets.status(s.id);
        if st != lamp.1 {
            lamp.1 = st;
            mat.0 = lamp_mat(&gfx, st);
        }
        let p = Vec3::from_array(s.pos);
        tf.translation = p + p.normalize() * lamp_height(s.kind, &mut heights, &world_gfx, &meshes);
        tf.scale = Vec3::splat(if st == Status::NoPower && !blink { 0.55 } else { 1.0 });
    }
    for s in structs.iter().filter(|s| super::is_consumer(s.kind) && !have.contains(&s.id)) {
        let st = grid.nets.status(s.id);
        let p = Vec3::from_array(s.pos);
        commands.spawn((
            Mesh3d(gfx.bulb.clone()),
            MeshMaterial3d(lamp_mat(&gfx, st)),
            Transform::from_translation(p + p.normalize() * lamp_height(s.kind, &mut heights, &world_gfx, &meshes)),
            NotShadowCaster,
            Lamp(s.id, st),
            PlanetScoped,
        ));
    }
}

fn wire_mat(gfx: &NetGfx, k: WireKind, live: bool) -> Handle<StandardMaterial> {
    match (k, live) {
        (WireKind::Cable, true) => gfx.cable_live.clone(),
        (WireKind::Cable, false) => gfx.cable_dead.clone(),
        (WireKind::Tether, true) => gfx.tether_live.clone(),
        (WireKind::Tether, false) => gfx.tether_dead.clone(),
    }
}

fn lamp_mat(gfx: &NetGfx, st: Status) -> Handle<StandardMaterial> {
    match st {
        Status::Ok => gfx.lamp_ok.clone(),
        Status::Low => gfx.lamp_low.clone(),
        Status::NoPower => gfx.lamp_off.clone(),
    }
}

fn stretch(tf: &mut Transform, a: Vec3, b: Vec3, r: f32) {
    let d = b - a;
    tf.translation = a;
    tf.rotation = Quat::from_rotation_arc(Vec3::Y, d.normalize_or(Vec3::Y));
    tf.scale = Vec3::new(r, d.length().max(0.01), r);
}

/// The line from the tether post you breathe from to your backpack, and the cable preview
/// while placing a structure.
#[allow(clippy::too_many_arguments)]
pub fn backpack_line(
    mut commands: Commands,
    game: Res<Game>,
    grid: Res<Grid>,
    gfx: Res<NetGfx>,
    ui: Res<crate::Ui>,
    player: Query<(&Transform, &Player), (Without<Backline>, Without<Preview>)>,
    ghost: Query<(&Transform, &Ghost), (Without<Backline>, Without<Preview>)>,
    mut line: Query<(&mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>), (With<Backline>, Without<Preview>)>,
    mut preview: Query<(&mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>), (With<Preview>, Without<Backline>)>,
) {
    let (Ok((mut tf, mut vis, mut mat)), Ok((mut ptf, mut pvis, mut pmat))) = (line.single_mut(), preview.single_mut()) else {
        if line.is_empty() {
            commands.spawn((Mesh3d(gfx.unit_tube.clone()), MeshMaterial3d(gfx.tether_live.clone()), Transform::default(), Visibility::Hidden, NotShadowCaster, Backline, PlanetScoped));
        }
        if preview.is_empty() {
            commands.spawn((Mesh3d(gfx.unit_tube.clone()), MeshMaterial3d(gfx.cable_live.clone()), Transform::default(), Visibility::Hidden, NotShadowCaster, Preview, PlanetScoped));
        }
        return;
    };
    *vis = Visibility::Hidden;
    *pvis = Visibility::Hidden;
    let Ok((ptr, pl)) = player.single() else { return };
    if let Some(Breath::Post(id, d)) = grid.breath {
        if let Some(post) = game.structs().iter().find(|s| s.id == id) {
            let up = ptr.translation.normalize();
            let pack = ptr.translation + up * 1.25 - pl.facing * 0.28;
            stretch(&mut tf, anchor(post), pack, TETHER_R);
            *vis = Visibility::Visible;
            let want = if d > TETHER_BREATH * STRETCH { &gfx.tether_warn } else { &gfx.tether_live };
            if mat.0 != *want {
                mat.0 = want.clone();
            }
        }
    }
    if ui.placing.is_some() {
        if let Ok((gtf, g)) = ghost.single() {
            if let Some((id, _)) = super::auto_link_at(game.structs(), g.0, gtf.translation) {
                if let Some(o) = game.structs().iter().find(|s| s.id == id) {
                    let a = gtf.translation + gtf.translation.normalize() * anchor_height(g.0);
                    stretch(&mut ptf, a, anchor(o), CABLE_R * 1.3);
                    *pvis = Visibility::Visible;
                    let live = grid.nets.of(id).map(|n| n.powered()).unwrap_or(false);
                    let want = if live { &gfx.cable_live } else { &gfx.cable_dead };
                    if pmat.0 != *want {
                        pmat.0 = want.clone();
                    }
                }
            }
        }
    }
}
