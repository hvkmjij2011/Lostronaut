//! Gameplay systems: terrain tool, harvesting, building, interaction, machine/power/oxygen
//! simulation, quests, travel cinematics.
use crate::items::{Item, Lang, Tech, RECIPES, SK};
use crate::network::{self, Grid, PlanetNets};
use crate::player::{Controls, Player};
use crate::state::{save_game, Game, Structure};
use crate::story;
use crate::voxel::{Brush, VoxelWorld};
use crate::world::{stash, upright, DecorEnt, DecorKind, Gfx, PlanetRt, PlanetScoped, ShipVisual, Sky, StructEnt};
use crate::{Action, AppState, Cine, CineKind, MainCam, Sys, ToolMode, Ui};
use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;

#[derive(Component)]
pub struct BrushInd;
#[derive(Component)]
pub struct Ghost(pub SK);
#[derive(Component)]
pub struct Flame;

/// Power readout for the HUD: the network of the structure the player is dealing with
/// (`focus`), not the whole planet. `focus == None` means no network nearby.
#[derive(Resource, Default, Clone, Copy)]
pub struct PowerStat {
    pub supply: f32,
    pub demand: f32,
    pub ratio: f32,
    pub battery: f32,
    pub capacity: f32,
    pub focus: Option<u32>,
    pub members: u32,
}

#[derive(Resource, Default)]
pub struct PendingTravel(pub Option<usize>);

#[derive(Clone, Copy, PartialEq)]
pub enum Target {
    Struct(u32),
    Decor(Entity),
}

#[derive(Resource, Default)]
pub struct Hover(pub Option<Target>);

const ORE_UNIT: f32 = 4.0;
pub const BATTERY_CAP: f32 = 300.0;
pub const BIO_BURN: f32 = 45.0;

fn t(l: Lang, tr: &str, en: &str) -> String {
    match l {
        Lang::Tr => tr.to_string(),
        Lang::En => en.to_string(),
    }
}

/// Adds items to the backpack, tracking discoveries. Returns how many fit.
pub fn give(game: &mut Game, ui: &mut Ui, item: Item, n: u32) -> u32 {
    let left = game.inv.add(item, n);
    let added = n - left;
    let l = game.lang;
    if added > 0 {
        match game.collected.iter_mut().find(|c| c.0 == item) {
            Some(c) => c.1 += added,
            None => game.collected.push((item, added)),
        }
        ui.pickup(item, added);
        if !game.discovered.contains(&item) {
            game.discovered.push(item);
            game.data += 30;
            ui.toast(format!("{}: {} (+30 {})", t(l, "Yeni keşif", "Discovered"), item.name(l), t(l, "Veri", "Data")), [0.5, 0.9, 1.0]);
            if item == Item::Heliosite && story::unlock(game, story::J_HELIO) {
                ui.journal_toast(l);
            }
        }
    }
    if left > 0 && ui.full_cd <= 0.0 {
        ui.full_cd = 3.0;
        ui.toast(t(l, "Sırt çantası dolu!", "Backpack is full!"), [1.0, 0.5, 0.3]);
    }
    added
}

fn harvest(game: &mut Game, ui: &mut Ui, rt: &PlanetRt, d: &DecorEnt) -> bool {
    let l = game.lang;
    let added = match d.kind {
        DecorKind::Plant(s) => {
            let sp = &rt.def.flora[s as usize];
            let a = give(game, ui, Item::Organic, sp.organic);
            if a > 0 && !game.species_seen.contains(&sp.name) {
                game.species_seen.push(sp.name.clone());
                game.data += 20;
                let word = crate::items::tx(l, sp.arch.word());
                ui.toast(format!("{}: {} {} (+20 {})", t(l, "Yeni tür", "New species"), sp.name, word, t(l, "Veri", "Data")), [0.6, 1.0, 0.6]);
            }
            a
        }
        DecorKind::Crystal(item) => give(game, ui, item, 4),
        DecorKind::Sample => give(game, ui, Item::Sample, 1),
        DecorKind::Fragment => give(game, ui, Item::DataFragment, 1),
        DecorKind::Scrap => give(game, ui, Item::Scrap, 2),
        DecorKind::Glow(_) => give(game, ui, Item::Organic, 1),
    };
    if added == 0 {
        return false;
    }
    let p = game.planet;
    game.planets[p].removed.push(d.id);
    true
}

fn pointer_hit(c: &Controls, cam: &Query<(&Camera, &GlobalTransform), With<MainCam>>, vw: &VoxelWorld) -> Option<Vec3> {
    let (camera, gt) = cam.single().ok()?;
    let ray = camera.viewport_to_world(gt, c.pointer?).ok()?;
    vw.raycast(ray.origin, *ray.direction, 80.0)
}

#[allow(clippy::too_many_arguments)]
pub fn tool_use(
    mut commands: Commands,
    time: Res<Time>,
    c: Res<Controls>,
    mut vw: ResMut<VoxelWorld>,
    mut game: ResMut<Game>,
    mut ui: ResMut<Ui>,
    rt: Res<PlanetRt>,
    gfx: Res<Gfx>,
    cine: Res<Cine>,
    cam: Query<(&Camera, &GlobalTransform), With<MainCam>>,
    mut player: Query<(&Transform, &mut Player)>,
    decor: Query<(Entity, &DecorEnt, &Transform), Without<Player>>,
    mut ind: Query<(&mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>), (With<BrushInd>, Without<Player>, Without<DecorEnt>)>,
    mut anchor: Local<Option<(Vec3, Vec3)>>,
) {
    let Ok((ptf, mut pl)) = player.single_mut() else { return };
    pl.using_tool = false;
    ui.full_cd -= time.delta_secs();
    if ind.is_empty() {
        commands.spawn((Mesh3d(gfx.sphere.clone()), MeshMaterial3d(gfx.brush_idle.clone()), Transform::default(), Visibility::Hidden, NotShadowCaster, BrushInd, PlanetScoped));
        return;
    }
    let Ok((mut itf, mut ivis, mut imat)) = ind.single_mut() else { return };
    *ivis = Visibility::Hidden;
    if cine.0.is_some() || ui.paused || ui.placing.is_some() || ui.open || ui.machine.is_some() {
        return;
    }
    ui.max_brush = if game.has_tech(Tech::WideTool) { 4.0 } else { 2.5 };
    ui.brush = ui.brush.min(ui.max_brush);
    if !c.tool {
        *anchor = None;
    }
    let Some(hit) = pointer_hit(&c, &cam, &vw) else { return };
    let ppos = ptf.translation;
    let up = ppos.normalize();
    if hit.distance(ppos + up) > 11.0 {
        return;
    }
    let radius = ui.brush;
    // preview as a translucent skin hugging the ground instead of a floating ball
    let n0 = vw.normal(hit);
    *itf = Transform::from_translation(hit + n0 * radius * 0.02)
        .with_rotation(Quat::from_rotation_arc(Vec3::Y, n0))
        .with_scale(Vec3::new(radius, radius * 0.06, radius));
    if !c.touch || c.tool {
        *ivis = Visibility::Visible;
    }
    let want = if c.tool { &gfx.brush } else { &gfx.brush_idle };
    if imat.0 != *want {
        imat.0 = want.clone();
    }
    if !c.tool {
        return;
    }
    pl.using_tool = true;
    if let Some(f) = (hit - ppos).reject_from(up).try_normalize() {
        pl.facing = f;
    }
    let n = vw.normal(hit);
    let boost = if game.has_tech(Tech::WideTool) { 1.6 } else { 1.0 };
    let dt = time.delta_secs().min(0.05);
    let out = match ui.tool_mode {
        // the brush sits mostly above the surface so each pass only shaves a shallow bowl
        ToolMode::Dig => vw.edit(hit + n * radius * 0.4, radius, &Brush::Dig, 0.9 * boost * dt),
        ToolMode::Fill => {
            if game.soil <= 0.5 {
                if ui.full_cd <= 0.0 {
                    ui.full_cd = 3.0;
                    ui.toast(t(game.lang, "Toprak deposu boş: önce kaz", "Soil canister empty: dig first"), [1.0, 0.6, 0.3]);
                }
                return;
            }
            vw.edit(hit - n * radius * 0.4, radius, &Brush::Fill, 0.8 * boost * dt)
        }
        ToolMode::Flatten => {
            let (a, an) = *anchor.get_or_insert((hit, hit.normalize()));
            vw.edit(hit, radius + 0.5, &Brush::Flatten { anchor: a, normal: an }, 1.1 * boost * dt)
        }
    };
    let max_soil = game.max_soil();
    game.soil = (game.soil + out.soil).clamp(0.0, max_soil);
    for (item, vol) in out.ores {
        let mut whole = 0;
        match game.ore_frac.iter_mut().find(|o| o.0 == item) {
            Some(o) => {
                o.1 += vol;
                while o.1 >= ORE_UNIT {
                    o.1 -= ORE_UNIT;
                    whole += 1;
                }
            }
            None => game.ore_frac.push((item, vol)),
        }
        if whole > 0 {
            give(&mut game, &mut ui, item, whole);
        }
    }
    if ui.tool_mode == ToolMode::Dig {
        for (e, d, dtf) in &decor {
            if dtf.translation.distance(hit) < radius * 0.8 + d.radius && harvest(&mut game, &mut ui, &rt, d) {
                commands.entity(e).despawn();
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn placement(
    mut commands: Commands,
    c: Res<Controls>,
    mut vw: ResMut<VoxelWorld>,
    mut game: ResMut<Game>,
    mut ui: ResMut<Ui>,
    gfx: Res<Gfx>,
    cam: Query<(&Camera, &GlobalTransform), With<MainCam>>,
    player: Query<&Transform, With<Player>>,
    mut ghost: Query<(Entity, &mut Transform, &mut MeshMaterial3d<StandardMaterial>, &mut Mesh3d, &mut Ghost), Without<Player>>,
    mut last: Local<Option<Vec3>>,
) {
    let Some(kind) = ui.placing else {
        for (e, ..) in &ghost {
            commands.entity(e).despawn();
        }
        *last = None;
        ui.place_confirm = false;
        return;
    };
    let Ok(ptf) = player.single() else { return };
    let ppos = ptf.translation;
    if let Some(hit) = pointer_hit(&c, &cam, &vw) {
        *last = Some(hit);
    }
    let pos = last.unwrap_or_else(|| crate::world::offset_on_surface(&vw, ppos, 4.0, 0.0));
    let up = pos.normalize();
    let def = kind.def();
    let l = game.lang;
    let mut problem = None;
    if pos.distance(ppos) > 16.0 {
        problem = Some(t(l, "Çok uzak", "Too far"));
    } else if vw.normal(pos).dot(up) < 0.6 {
        problem = Some(t(l, "Zemin çok dik", "Ground too steep"));
    } else if game.structs().iter().any(|s| Vec3::from_array(s.pos).distance(pos) < s.kind.def().radius + def.radius + 0.4) {
        problem = Some(t(l, "Başka bir yapıya çok yakın", "Too close to another structure"));
    } else if !game.inv.has_all(def.cost) {
        problem = Some(t(l, "Malzeme yetersiz", "Not enough materials"));
    }
    let rot = upright(pos, ppos - pos);
    let mat = if problem.is_none() { gfx.ghost_ok.clone() } else { gfx.ghost_bad.clone() };
    match ghost.single_mut() {
        Ok((ge, mut tf, mut m, mut mesh, mut g)) => {
            *tf = Transform::from_translation(pos).with_rotation(rot);
            if m.0 != mat {
                m.0 = mat;
            }
            if g.0 != kind {
                g.0 = kind;
                mesh.0 = gfx.struct_mesh[&kind].clone();
                // Bevy computes an entity's bounds only once; a new mesh needs new bounds
                commands.entity(ge).remove::<bevy::render::primitives::Aabb>();
            }
        }
        Err(_) => {
            commands.spawn((Mesh3d(gfx.struct_mesh[&kind].clone()), MeshMaterial3d(mat), Transform::from_translation(pos).with_rotation(rot), NotShadowCaster, Ghost(kind), PlanetScoped));
        }
    }
    ui.place_problem = problem.clone();
    let confirm = ui.place_confirm || (c.tool_started && !c.touch);
    ui.place_confirm = false;
    if !confirm {
        return;
    }
    if let Some(p) = problem {
        ui.toast(p, [1.0, 0.5, 0.3]);
        return;
    }
    game.inv.remove_all(def.cost);
    for _ in 0..3 {
        vw.edit(pos, def.radius + 1.3, &Brush::Flatten { anchor: pos, normal: up }, 1.0);
    }
    let id = game.next_id;
    game.next_id += 1;
    game.structs_mut().push(Structure::new(id, kind, (pos - up * 0.05).to_array(), rot.to_array()));
    let first = !game.built.contains(&kind);
    if first {
        game.built.push(kind);
    }
    ui.toast(format!("{}: {}", t(l, "İnşa edildi", "Built"), kind.name(l)), [0.6, 1.0, 0.6]);
    // the common case is effortless: a new structure close to the base cables itself
    if let Some(other) = network::auto_link_target(game.structs(), id) {
        if network::link(game.structs_mut(), id, other).is_ok() {
            let name = game.structs().iter().find(|s| s.id == other).map(|s| s.kind.name(l)).unwrap_or("");
            ui.toast(format!("{} → {}", t(l, "Kablo bağlandı", "Cabled to"), name), [0.6, 0.9, 1.0]);
        }
    } else if network::carries_power(kind) {
        ui.toast(t(l, "Kablo menzilinde güç yok: Güç Direği kur ya da yakına taşı", "No power within cable reach: add a Power Pylon or build closer"), [1.0, 0.75, 0.35]);
    }
    let entry = match kind {
        SK::Smelter => Some(story::J_SMELTER),
        SK::Research => Some(story::J_RESEARCH),
        SK::ChemLab => Some(story::J_CHEM),
        SK::Shuttle => Some(story::J_SHUTTLE),
        SK::Shipyard => Some(story::J_YARD),
        _ => None,
    };
    if let Some(e) = entry {
        if story::unlock(&mut game, e) {
            ui.journal_toast(l);
        }
    }
    ui.placing = None;
}

#[allow(clippy::too_many_arguments)]
pub fn interact(
    mut commands: Commands,
    c: Res<Controls>,
    mut ui: ResMut<Ui>,
    mut game: ResMut<Game>,
    rt: Res<PlanetRt>,
    cine: Res<Cine>,
    mut hover: ResMut<Hover>,
    grid: Res<Grid>,
    player: Query<&Transform, With<Player>>,
    decor: Query<(Entity, &DecorEnt, &Transform)>,
) {
    hover.0 = None;
    ui.hover_text = None;
    let Ok(ptf) = player.single() else { return };
    if cine.0.is_some() || ui.placing.is_some() {
        return;
    }
    let p = ptf.translation;
    let l = game.lang;
    let mut best = f32::MAX;
    for s in game.structs() {
        let d = Vec3::from_array(s.pos).distance(p) - s.kind.def().radius;
        if d < 2.6 && d < best {
            best = d;
            hover.0 = Some(Target::Struct(s.id));
            ui.hover_text = Some(if s.kind == SK::Tether {
                format!("{}: {}", t(l, "Topla", "Pick up"), s.kind.name(l))
            } else {
                format!("{}{}", s.kind.name(l), network::hud::hover_suffix(&grid, s.kind, s.id, l))
            });
        }
    }
    for (e, d, tf) in &decor {
        if matches!(d.kind, DecorKind::Sample | DecorKind::Fragment) {
            let dist = tf.translation.distance(p) - 0.5;
            if dist < 2.4 && dist < best {
                best = dist;
                hover.0 = Some(Target::Decor(e));
                ui.hover_text = Some(if d.kind == DecorKind::Sample { Item::Sample.name(l) } else { Item::DataFragment.name(l) }.to_string());
            }
        }
    }
    if !c.interact || ui.open || ui.paused {
        return;
    }
    if ui.machine.is_some() {
        ui.machine = None;
        return;
    }
    match hover.0 {
        Some(Target::Struct(id)) if game.structs().iter().any(|s| s.id == id && s.kind == SK::Tether) => {
            // tethers have nothing to configure: interacting picks them up
            ui.actions.push(Action::Dismantle(id));
        }
        Some(Target::Struct(id)) => {
            ui.machine = Some(id);
            ui.open = false;
        }
        Some(Target::Decor(e)) => {
            if let Ok((_, d, _)) = decor.get(e) {
                if harvest(&mut game, &mut ui, &rt, d) {
                    commands.entity(e).despawn();
                }
            }
        }
        None => {}
    }
}

/// Drops an oxygen tether post behind the astronaut: one key (T) or one touch button.
#[allow(clippy::too_many_arguments)]
pub fn drop_tether(
    keys: Res<ButtonInput<KeyCode>>,
    mut net_ui: ResMut<network::hud::NetUi>,
    mut game: ResMut<Game>,
    mut ui: ResMut<Ui>,
    grid: Res<Grid>,
    cine: Res<Cine>,
    vw: Res<VoxelWorld>,
    player: Query<(&Transform, &Player)>,
) {
    let want = std::mem::take(&mut net_ui.drop_tether) || keys.just_pressed(KeyCode::KeyT);
    if !want || ui.paused || ui.open || ui.machine.is_some() || ui.placing.is_some() || cine.0.is_some() {
        return;
    }
    let Ok((ptf, pl)) = player.single() else { return };
    let l = game.lang;
    let cost = SK::Tether.def().cost;
    if !game.inv.has_all(cost) {
        ui.toast(t(l, "Halat için 1 Organik gerekli", "A relay needs 1 Organic"), [1.0, 0.5, 0.3]);
        return;
    }
    let ppos = ptf.translation;
    let up = ppos.normalize();
    let behind = ppos - pl.facing * 0.9;
    let pos = vw.raycast(behind + up * 1.5, -up, 4.0).unwrap_or(ppos);
    if game.structs().iter().any(|s| Vec3::from_array(s.pos).distance(pos) < s.kind.def().radius + 0.6) {
        ui.toast(t(l, "Başka bir yapıya çok yakın", "Too close to another structure"), [1.0, 0.5, 0.3]);
        return;
    }
    game.inv.remove_all(cost);
    let id = game.next_id;
    game.next_id += 1;
    let rot = upright(pos, pl.facing);
    game.structs_mut().push(Structure::new(id, SK::Tether, (pos - up * 0.05).to_array(), rot.to_array()));
    if !game.built.contains(&SK::Tether) {
        game.built.push(SK::Tether);
    }
    let live = network::tethers(game.structs(), &grid.sources).iter().any(|p| p.id == id && p.live);
    if live {
        ui.toast(t(l, "Halat kuruldu: zincir canlı", "Relay placed: chain is live"), [0.6, 0.9, 1.0]);
    } else {
        ui.toast(t(l, "Halat bağlı değil! Zincire ya da bir oksijen kaynağına daha yakın kur", "Relay not connected! Place it closer to the chain or an oxygen source"), [1.0, 0.6, 0.3]);
    }
}

#[derive(Default)]
struct TickOut {
    data: u32,
    researched: u32,
    fragments: u32,
}

/// Runs every machine of one planet for `dt` seconds. Power is shared only inside each cabled
/// network (see `network`); works from save data alone so other planets keep running.
fn tick_structs(structs: &mut [Structure], sun: &dyn Fn(Vec3) -> f32, dt: f32, out: &mut TickOut) -> PlanetNets {
    let (net, count) = network::components(structs);
    let stats = network::balance(structs, &net, count, sun, dt);
    for (i, s) in structs.iter_mut().enumerate() {
        let n = &stats[net[i]];
        // energy into (+) or out of (-) this network's batteries
        let mut delta = (n.supply - n.demand) * dt;
        if delta < 0.0 {
            delta = -(-delta).min(n.battery);
        }
        let busy = n.demand > 0.0 || n.battery < n.capacity - 0.5;
        match s.kind {
            SK::Battery => {
                let share = if delta > 0.0 { delta / (n.capacity / BATTERY_CAP) } else if n.battery > 0.0 { delta * s.charge / n.battery } else { 0.0 };
                s.charge = (s.charge + share).clamp(0.0, BATTERY_CAP);
            }
            SK::BioGen => {
                if busy && s.fuel > 0.0 {
                    s.fuel = (s.fuel - dt).max(0.0);
                }
            }
            _ => {}
        }
        let ratio = n.ratio;
        if let Some(job) = &mut s.job {
            let r = &RECIPES[job.recipe as usize];
            job.progress += dt * ratio / r.time;
            if job.progress >= 1.0 {
                if r.data > 0 {
                    out.data += r.data;
                    out.researched += 1;
                    if r.inputs[0].0 == Item::DataFragment {
                        out.fragments += 1;
                    }
                } else {
                    match s.out.iter_mut().find(|o| o.0 == r.output.0) {
                        Some(o) => o.1 += r.output.1,
                        None => s.out.push(r.output),
                    }
                }
                job.count -= 1;
                job.progress = 0.0;
                if job.count == 0 {
                    s.job = None;
                }
            }
        }
    }
    PlanetNets { ids: structs.iter().map(|s| s.id).collect(), net, stats }
}

/// Oxygen tank drain per second outside any oxygen supply (a full tank lasts ~2-3 minutes).
pub fn o2_drain(atmosphere: f32) -> f32 {
    100.0 / 150.0 * (0.8 + 0.3 * atmosphere)
}

/// Picks the structure whose network the HUD power readout should describe: the open panel,
/// else the hovered structure, else the nearest cable-capable structure close by.
fn focus_struct(structs: &[Structure], machine: Option<u32>, hover: Option<u32>, p: Vec3) -> Option<u32> {
    let powered = |id: u32| structs.iter().any(|s| s.id == id && network::carries_power(s.kind));
    machine.filter(|&id| powered(id)).or(hover.filter(|&id| powered(id))).or_else(|| {
        structs
            .iter()
            .filter(|s| network::carries_power(s.kind))
            .map(|s| (s.id, Vec3::from_array(s.pos).distance(p)))
            .filter(|(_, d)| *d < 16.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    })
}

#[allow(clippy::too_many_arguments)]
pub fn simulate(
    time: Res<Time>,
    mut game: ResMut<Game>,
    sky: Res<Sky>,
    rt: Res<PlanetRt>,
    vw: Res<VoxelWorld>,
    cine: Res<Cine>,
    mut ui: ResMut<Ui>,
    mut power: ResMut<PowerStat>,
    mut grid: ResMut<Grid>,
    hover: Res<Hover>,
    decor: Option<Res<crate::decor::DecorStream>>,
    mut player: Query<(&mut Transform, &mut Player)>,
    mut autosave: Local<f32>,
    mut checked: Local<bool>,
) {
    if !*checked {
        // saves written by other builds may carry stale cable ids
        *checked = true;
        for p in game.planets.iter_mut() {
            network::sanitize(&mut p.structs);
        }
    }
    if ui.paused {
        return;
    }
    let dt = time.delta_secs().min(0.1);
    game.time += dt as f64;
    let l = game.lang;

    // machines on every planet keep running, each network on its own
    let mut out = TickOut::default();
    let current = game.planet;
    let sun_dir = sky.sun_dir;
    for pi in 0..game.planets.len() {
        if pi == current {
            grid.nets = tick_structs(&mut game.planets[pi].structs, &|p: Vec3| if p.normalize().dot(sun_dir) > 0.05 { 1.0 } else { 0.0 }, dt, &mut out);
        } else {
            tick_structs(&mut game.planets[pi].structs, &|_| 0.5, dt, &mut out);
        }
    }
    grid.sources = network::sources(game.structs(), &grid.nets);
    grid.posts = network::tethers(game.structs(), &grid.sources);
    if out.data > 0 {
        game.data += out.data;
        game.researched += out.researched;
        ui.toast(format!("+{} {}", out.data, t(l, "Veri", "Data")), [0.5, 0.9, 1.0]);
    }
    for _ in 0..out.fragments {
        if let Some(id) = story::decode_fragment(&mut game) {
            ui.toast(crate::items::tx(l, story::decode_toast(id)).to_string(), [1.0, 0.7, 0.4]);
        }
    }

    // oxygen & health
    let Ok((ptf, _)) = player.single_mut() else { return };
    let ppos = ptf.translation;
    grid.focus = focus_struct(game.structs(), ui.machine, hover.0.and_then(|t| if let Target::Struct(id) = t { Some(id) } else { None }), ppos);
    let fs = grid.focus.and_then(|id| grid.nets.of(id)).unwrap_or_default();
    *power = PowerStat { supply: fs.supply, demand: fs.demand, ratio: fs.ratio, battery: fs.battery, capacity: fs.capacity, focus: grid.focus, members: fs.members };
    if cine.0.is_none() {
        let before = grid.breath;
        grid.breath = network::breath(ppos, &grid.sources, &grid.posts);
        let in_zone = grid.breath.is_some();
        ui.in_oxygen = in_zone;
        let drain = o2_drain(rt.def.atmosphere);
        if in_zone {
            game.oxygen = (game.oxygen + 30.0 * dt).min(100.0);
            game.health = (game.health + 4.0 * dt).min(100.0);
        } else {
            game.oxygen = (game.oxygen - drain * dt).max(0.0);
            if game.oxygen <= 0.0 {
                game.health -= 9.0 * dt;
            }
        }
        if before.is_some() && !in_zone {
            let secs = game.oxygen / drain;
            ui.toast(format!("{} ({:.0} s)", t(l, "⚠ Oksijen bağlantısı koptu! Tüp boşalıyor", "⚠ Oxygen link lost! Tank draining"), secs), [1.0, 0.45, 0.35]);
        }
        // health at zero: see `blackout`
    }

    if sky.day < 0.05 && !game.seen_night && game.time > 20.0 {
        game.seen_night = true;
        if story::unlock(&mut game, story::J_NIGHT) {
            ui.journal_toast(l);
        }
    }

    // story: MIRA reacts to what happens (every trigger lives in story::check)
    if let Some(decor) = &decor {
        // a wreck site counts as found within 25 m; its index on this planet is its stable id
        if let Some(id) = decor.wrecks.iter().position(|w| w.centre.distance(ppos) < 25.0) {
            story::fire(&mut game, story::StoryEvent::WreckFound { id: id as u32 });
        }
    }
    let sctx = story::StoryCtx::observe(&vw, ppos, &rt.def, sky.day, matches!(grid.breath, Some(network::Breath::Post(..))), grid.breath.is_some());
    for note in story::check(&mut game, &sctx) {
        note.show(&mut ui, l);
    }

    // quests
    while game.quest < story::QUESTS.len() && (story::QUESTS[game.quest].done)(&game) {
        game.quest += 1;
        game.data += 40;
        ui.toast(format!("{} (+40 {})", t(l, "Görev tamamlandı", "Objective complete"), t(l, "Veri", "Data")), [1.0, 0.9, 0.4]);
        if game.quest == story::Q_LAUNCH && story::unlock(&mut game, story::J_SHIP) {
            ui.journal_toast(l);
        }
    }

    *autosave += dt;
    if *autosave > 75.0 && cine.0.is_none() {
        *autosave = 0.0;
        stash(&mut game, &vw, Some(ppos));
        let _ = save_game(&game);
    }
}

/// Blacking out: MIRA leaves the whole backpack behind as a bundle where the player fell and
/// carries them straight to the base, onto the ground next to its main structure (the pod at
/// home, otherwise the shuttle or the base's oxygen source). Soil and Data are kept.
#[allow(clippy::too_many_arguments)]
pub fn blackout(
    mut game: ResMut<Game>,
    mut ui: ResMut<Ui>,
    rt: Res<PlanetRt>,
    sys: Res<Sys>,
    vw: Res<VoxelWorld>,
    cine: Res<Cine>,
    mut fx: ResMut<crate::hazards::HazFx>,
    mut rig: ResMut<crate::player::CamRig>,
    mut player: Query<(&mut Transform, &mut Player)>,
    hazards: Query<&crate::hazards::HazEnt>,
) {
    if game.health > 0.0 || cine.0.is_some() {
        return;
    }
    let Ok((mut ptf, mut pl)) = player.single_mut() else { return };
    let l = game.lang;
    let around: Vec<(Vec3, f32)> = hazards.iter().map(|h| (h.h.pos, h.h.reach())).collect();
    let fell = crate::hazards::bundle_spot(ptf.translation, &around, &|p| {
        let u = p.normalize();
        vw.raycast(p + u * 2.0, -u, 6.0)
    });
    let cause = crate::hazards::cause_text(&fx, &game, rt.def.kind);
    let bundle = crate::hazards::drop_bundle(&mut game, fell);
    let home = game.planet == sys.0.home;
    let (anchor, radius) = crate::hazards::base_anchor(game.structs(), home, rt.landing);
    let spot = crate::hazards::respawn_spot(&|p| vw.sample(p), anchor, radius, game.structs());
    let up = spot.normalize();
    *ptf = Transform::from_translation(spot);
    pl.vel = Vec3::ZERO;
    pl.grounded = true;
    let to_base = (anchor - spot).reject_from(up).normalize_or(crate::util::any_perp(up));
    pl.facing = to_base;
    rig.heading = to_base;
    rig.pitch = 0.35;
    game.health = 100.0;
    game.oxygen = 100.0;
    game.deaths += 1;
    crate::story::fire(&mut game, crate::story::StoryEvent::Blackout);
    ui.machine = None;
    ui.placing = None;
    ui.open = false;
    fx.revive();
    let line = t(l, crate::hazards::CARRIED_HOME.0, crate::hazards::CARRIED_HOME.1);
    fx.fade = 2.6;
    fx.fade_text = line.clone();
    fx.fade_sub = cause.clone();
    ui.toast(line, [1.0, 0.55, 0.35]);
    let gear = if bundle.is_some() { t(l, " • Eşyaların düştüğün yerde (turuncu işaret)", " • Your gear is where you fell (orange marker)") } else { String::new() };
    ui.toast(format!("{cause}{gear}"), [1.0, 0.75, 0.5]);
    eprintln!("[blackout] fell {:.1} m from base, carried to {:.1} m from it (height {:+.2} m), bundle {:?}", fell.distance(anchor), spot.distance(anchor), (spot - anchor).dot(up), bundle);
    stash(&mut game, &vw, Some(spot));
    let _ = save_game(&game);
}

#[allow(clippy::too_many_arguments)]
pub fn apply_actions(
    mut ui: ResMut<Ui>,
    mut game: ResMut<Game>,
    vw: Res<VoxelWorld>,
    sys: Res<Sys>,
    mut cine: ResMut<Cine>,
    mut pending: ResMut<PendingTravel>,
    mut next: ResMut<NextState<AppState>>,
    player: Query<&Transform, With<Player>>,
) {
    let actions: Vec<Action> = ui.actions.drain(..).collect();
    let ppos = player.single().ok().map(|t| t.translation);
    let l = game.lang;
    for a in actions {
        match a {
            Action::Travel(dest) => {
                let Some(id) = ui.travel_from else { continue };
                let cost = sys.0.fuel_cost(game.planet, dest) as f32;
                let Some(s) = game.find_struct(id) else { continue };
                if s.fuel + 0.01 < cost || cine.0.is_some() {
                    continue;
                }
                s.fuel -= cost;
                pending.0 = Some(dest);
                cine.0 = Some((CineKind::Launch, 0.0, id));
                ui.reset_play();
            }
            Action::Finale(id) => {
                if cine.0.is_none() {
                    cine.0 = Some((CineKind::Finale, 0.0, id));
                    ui.reset_play();
                }
            }
            Action::Dismantle(id) => {
                let Some(i) = game.structs().iter().position(|s| s.id == id) else { continue };
                network::unlink_all(game.structs_mut(), id);
                let s = game.structs_mut().remove(i);
                for &(item, n) in s.kind.def().cost {
                    game.inv.add(item, n);
                }
                for (item, n) in s.out {
                    game.inv.add(item, n);
                }
                ui.machine = None;
                let verb = if s.kind == SK::Tether { t(l, "Toplandı", "Picked up") } else { t(l, "Söküldü", "Dismantled") };
                ui.toast(format!("{}: {}", verb, s.kind.name(l)), [0.9, 0.9, 0.6]);
            }
            Action::Save => {
                stash(&mut game, &vw, ppos);
                match save_game(&game) {
                    Ok(()) => ui.toast(t(l, "Oyun kaydedildi", "Game saved"), [0.6, 1.0, 0.6]),
                    Err(e) => ui.toast(format!("{}: {e}", t(l, "Kayıt hatası", "Save failed")), [1.0, 0.4, 0.3]),
                }
            }
            Action::ToMenu => {
                stash(&mut game, &vw, ppos);
                let _ = save_game(&game);
                ui.reset_play();
                next.set(AppState::Menu);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn cine_update(
    mut commands: Commands,
    time: Res<Time>,
    mut cine: ResMut<Cine>,
    mut game: ResMut<Game>,
    vw: Res<VoxelWorld>,
    gfx: Res<Gfx>,
    mut pending: ResMut<PendingTravel>,
    mut next: ResMut<NextState<AppState>>,
    mut structs: Query<(Entity, &mut Transform, &StructEnt), Without<ShipVisual>>,
    mut ships: Query<(Entity, &mut Transform, &ShipVisual)>,
    flames: Query<Entity, With<Flame>>,
    player: Query<&Transform, (With<Player>, Without<StructEnt>, Without<ShipVisual>)>,
) {
    let Some((kind, t0, id)) = cine.0 else {
        for e in &flames {
            commands.entity(e).despawn();
        }
        return;
    };
    let tt = t0 + time.delta_secs();
    cine.0 = Some((kind, tt, id));
    let spawn_flame = |commands: &mut Commands, parent: Entity, y: f32, scale: f32| {
        commands.entity(parent).with_children(|p| {
            p.spawn((Mesh3d(gfx.flame.clone()), MeshMaterial3d(gfx.glow.clone()), Transform::from_xyz(0.0, y, 0.0).with_scale(Vec3::splat(scale)), NotShadowCaster, Flame));
        });
    };
    match kind {
        CineKind::Launch | CineKind::Land => {
            let Some(base) = game.structs().iter().find(|s| s.id == id).map(|s| Vec3::from_array(s.pos)) else {
                cine.0 = None;
                return;
            };
            let up = base.normalize();
            let Some((e, mut tf, _)) = structs.iter_mut().find(|(_, _, s)| s.0 == id) else { return };
            if flames.is_empty() {
                spawn_flame(&mut commands, e, 0.7, 1.0);
            }
            if kind == CineKind::Launch {
                let warm = (tt - 1.0).max(0.0);
                tf.translation = base + up * 3.0 * warm * warm;
                if tt > 5.0 {
                    let dest = pending.0.take().unwrap_or(game.planet);
                    stash(&mut game, &vw, None);
                    if let Some(i) = game.structs().iter().position(|s| s.id == id) {
                        // the shuttle leaves its cables behind
                        network::unlink_all(game.structs_mut(), id);
                        let mut s = game.structs_mut().remove(i);
                        s.links.clear();
                        game.carried_shuttle = Some(s);
                    }
                    game.planet = dest;
                    game.travels += 1;
                    game.oxygen = 100.0;
                    cine.0 = None;
                    next.set(AppState::Loading);
                }
            } else {
                let total = 4.5;
                let rem = (total - tt).max(0.0);
                tf.translation = base + up * 3.0 * rem * rem;
                if tt >= total {
                    tf.translation = base;
                    cine.0 = None;
                }
            }
        }
        CineKind::Finale => {
            let Some((e, mut tf, _)) = ships.iter_mut().find(|(_, _, s)| s.0 == id) else {
                cine.0 = None;
                return;
            };
            if flames.is_empty() {
                spawn_flame(&mut commands, e, 0.6, 2.6);
            }
            let warm = (tt - 2.0).max(0.0);
            tf.translation.y = 0.5 + 2.2 * warm * warm;
            if tt > 9.0 {
                game.finished = true;
                if game.quest < story::QUESTS.len() {
                    game.quest = story::QUESTS.len();
                }
                stash(&mut game, &vw, player.single().ok().map(|t| t.translation));
                let _ = save_game(&game);
                cine.0 = None;
                next.set(AppState::Ending);
            }
        }
    }
}

/// Never lose progress when the app is sent to the background (Android) or the window is closed.
pub fn save_on_suspend(
    mut lifecycle: EventReader<bevy::window::AppLifecycle>,
    mut close: EventReader<bevy::window::WindowCloseRequested>,
    mut game: ResMut<Game>,
    vw: Res<VoxelWorld>,
    cine: Res<Cine>,
    player: Query<&Transform, With<Player>>,
) {
    use bevy::window::AppLifecycle;
    let suspend = lifecycle.read().any(|e| matches!(e, AppLifecycle::WillSuspend | AppLifecycle::Suspended));
    let closing = close.read().count() > 0;
    if (suspend || closing) && cine.0.is_none() {
        stash(&mut game, &vw, player.single().ok().map(|t| t.translation));
        let _ = save_game(&game);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Job;

    fn st(id: u32, kind: SK) -> Structure {
        Structure::new(id, kind, [id as f32, 70.0, 0.0], [0.0, 0.0, 0.0, 1.0])
    }

    /// cables every structure of the list to the first one (a small star-shaped base)
    fn cabled(mut v: Vec<Structure>) -> Vec<Structure> {
        let hub = v[0].id;
        let ids: Vec<u32> = v.iter().skip(1).map(|s| s.id).collect();
        for id in ids {
            network::link(&mut v, hub, id).unwrap();
        }
        v
    }

    fn run(structs: &mut [Structure], secs: f32) -> (PlanetNets, TickOut) {
        let mut out = TickOut::default();
        let mut nets = PlanetNets::default();
        let steps = (secs / 0.1) as usize;
        for _ in 0..steps {
            nets = tick_structs(structs, &|_| 1.0, 0.1, &mut out);
        }
        (nets, out)
    }

    #[test]
    fn smelter_runs_on_pod_power() {
        let mut v = cabled(vec![st(1, SK::Pod), st(2, SK::Smelter)]);
        v[1].job = Some(Job { recipe: 0, count: 2, progress: 0.0 }); // 2x iron, 4 s each
        let (nets, _) = run(&mut v, 4.5);
        assert!((nets.of(2).unwrap().ratio - 1.0).abs() < 1e-3);
        assert_eq!(v[1].out, vec![(Item::Iron, 1)]);
        run(&mut v, 4.5);
        assert_eq!(v[1].out, vec![(Item::Iron, 2)]);
        assert!(v[1].job.is_none());
    }

    #[test]
    fn smelter_only_works_once_cabled() {
        let mut v = vec![st(1, SK::Pod), st(2, SK::Smelter)];
        v[1].job = Some(Job { recipe: 0, count: 1, progress: 0.0 });
        let (nets, _) = run(&mut v, 10.0);
        assert!(v[1].out.is_empty(), "the pod must not power a smelter it is not cabled to");
        assert_eq!(nets.status(2), network::Status::NoPower);
        assert_eq!(nets.status(1), network::Status::Ok);
        network::link(&mut v, 1, 2).unwrap();
        let (nets, _) = run(&mut v, 4.5);
        assert_eq!(v[1].out, vec![(Item::Iron, 1)]);
        assert_eq!(nets.index_of(1), nets.index_of(2));
    }

    #[test]
    fn no_power_no_progress_and_overload_slows_down() {
        let mut v = vec![st(2, SK::Smelter)];
        v[0].job = Some(Job { recipe: 0, count: 1, progress: 0.0 });
        run(&mut v, 30.0);
        assert!(v[0].out.is_empty(), "a smelter without any generator must not work");

        // pod gives 4, smelter + fabricator need 8 -> half speed
        let mut v = cabled(vec![st(1, SK::Pod), st(2, SK::Smelter), st(3, SK::Fabricator)]);
        v[1].job = Some(Job { recipe: 0, count: 1, progress: 0.0 });
        v[2].job = Some(Job { recipe: 8, count: 1, progress: 0.0 });
        let (nets, _) = run(&mut v, 5.0);
        assert!((nets.of(1).unwrap().ratio - 0.5).abs() < 1e-3);
        assert_eq!(nets.status(2), network::Status::Low);
        assert!(v[1].out.is_empty(), "4 s recipe must not be done after 5 s at half power");
        run(&mut v, 4.0);
        assert_eq!(v[1].out, vec![(Item::Iron, 1)]);
    }

    #[test]
    fn separate_networks_do_not_share_load() {
        // two pods, each with its own smelter: both run at full speed
        let mut v = vec![st(1, SK::Pod), st(2, SK::Smelter), st(3, SK::Pod), st(4, SK::Smelter)];
        network::link(&mut v, 1, 2).unwrap();
        network::link(&mut v, 3, 4).unwrap();
        v[1].job = Some(Job { recipe: 0, count: 1, progress: 0.0 });
        v[3].job = Some(Job { recipe: 0, count: 1, progress: 0.0 });
        let (nets, _) = run(&mut v, 4.5);
        assert_ne!(nets.index_of(2), nets.index_of(4));
        assert_eq!(v[1].out, vec![(Item::Iron, 1)]);
        assert_eq!(v[3].out, vec![(Item::Iron, 1)]);
    }

    #[test]
    fn battery_charges_from_surplus_and_covers_deficit() {
        let mut v = cabled(vec![st(1, SK::Pod), st(2, SK::Battery)]);
        run(&mut v, 10.0);
        assert!((v[1].charge - 40.0).abs() < 0.5, "4 power for 10 s = 40, got {}", v[1].charge);
        // now a chem lab (5) on a pod (4): the battery covers the missing 1/s
        v.push(st(3, SK::ChemLab));
        network::link(&mut v, 1, 3).unwrap();
        v[2].job = Some(Job { recipe: 14, count: 1, progress: 0.0 });
        let (nets, _) = run(&mut v, 2.0);
        assert!((nets.of(3).unwrap().ratio - 1.0).abs() < 1e-3);
        assert!(v[1].charge < 40.0);
        // an uncabled battery does not charge
        let mut lone = vec![st(1, SK::Pod), st(2, SK::Battery)];
        run(&mut lone, 5.0);
        assert_eq!(lone[1].charge, 0.0);
    }

    #[test]
    fn biogen_burns_only_when_needed() {
        let mut v = vec![st(1, SK::BioGen)];
        v[0].fuel = 20.0;
        run(&mut v, 5.0);
        assert_eq!(v[0].fuel, 20.0, "idle generator must keep its fuel");
        v.push(st(2, SK::Smelter));
        network::link(&mut v, 1, 2).unwrap();
        v[1].job = Some(Job { recipe: 0, count: 1, progress: 0.0 });
        run(&mut v, 3.0);
        assert!(v[0].fuel < 17.5);
    }

    #[test]
    fn research_gives_data_and_decodes_fragments() {
        let sample = RECIPES.iter().position(|r| r.data > 0 && r.inputs[0].0 == Item::Sample).unwrap();
        let frag = RECIPES.iter().position(|r| r.data > 0 && r.inputs[0].0 == Item::DataFragment).unwrap();
        let mut v = cabled(vec![st(1, SK::Pod), st(2, SK::Research)]);
        v[1].job = Some(Job { recipe: sample as u16, count: 1, progress: 0.0 });
        let (_, out) = run(&mut v, 9.0);
        assert_eq!((out.data, out.researched, out.fragments), (50, 1, 0));
        v[1].job = Some(Job { recipe: frag as u16, count: 1, progress: 0.0 });
        let (_, out) = run(&mut v, 13.0);
        assert_eq!((out.data, out.researched, out.fragments), (120, 1, 1));
    }

    #[test]
    fn hud_focus_prefers_panel_then_hover_then_nearby() {
        let v = vec![st(1, SK::Pod), st(2, SK::Smelter), st(3, SK::Storage)];
        let p = Vec3::new(1.0, 70.0, 0.0);
        assert_eq!(focus_struct(&v, Some(2), Some(1), p), Some(2));
        assert_eq!(focus_struct(&v, None, Some(1), p), Some(1));
        assert_eq!(focus_struct(&v, Some(3), None, p), Some(1), "a crate has no network; fall back to the nearest");
        assert_eq!(focus_struct(&v, None, None, Vec3::new(500.0, 70.0, 0.0)), None);
    }
}
