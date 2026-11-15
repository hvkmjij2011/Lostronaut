//! Developer-only scripted input (LOSTRANOUT_BOT=...) used for automated smoke tests.
use crate::items::SK;
use crate::network::{hud::NetUi, Grid};
use crate::voxel::VoxelWorld;
use crate::player::Controls;
use crate::state::Game;
use crate::{Action, Sys, Ui};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

#[allow(clippy::too_many_arguments)]
pub fn bot(
    time: Res<Time>,
    mut c: ResMut<Controls>,
    mut ui: ResMut<Ui>,
    mut game: ResMut<Game>,
    sys: Res<Sys>,
    (grid, mut net_ui, vw): (Res<Grid>, ResMut<NetUi>, Res<VoxelWorld>),
    windows: Query<&Window, With<PrimaryWindow>>,
    mut step: Local<u32>,
    mut t0: Local<Option<f32>>,
    mut frames: Local<(u32, f32)>,
) {
    // LOSTRANOUT_TOUCH=1 forces the touch UI on desktop (to exercise the touch buttons)
    if std::env::var("LOSTRANOUT_TOUCH").is_ok() {
        c.touch = true;
    }
    let Ok(mode) = std::env::var("LOSTRANOUT_BOT") else { return };
    let Ok(w) = windows.single() else { return };
    let start = *t0.get_or_insert(time.elapsed_secs());
    let t = time.elapsed_secs() - start;
    frames.0 += 1;
    if t - frames.1 > 2.0 {
        eprintln!("[bot] t={:.1} fps={:.0} inv={:?} soil={:.0} quest={} o2={:.0}", t, frames.0 as f32 / (t - frames.1), game.inv.slots.iter().flatten().take(6).collect::<Vec<_>>(), game.soil, game.quest, game.oxygen);
        if mode == "craft" {
            let st: Vec<String> = game.structs().iter().filter(|s| s.kind.station().is_some()).map(|s| format!("{:?} job={:?} out={:?}", s.kind, s.job.as_ref().map(|j| (j.recipe, j.count)), s.out)).collect();
            eprintln!("[bot] iron={} ore={} data={} techs={:?} {:?}", game.inv.count(crate::items::Item::Iron), game.inv.count(crate::items::Item::IronOre), game.data, game.techs, st);
        }
        let posts = grid.posts.iter().filter(|p| p.live).count();
        let smelter = game.structs().iter().find(|s| s.kind == SK::Smelter).map(|s| (s.links.clone(), grid.nets.status(s.id)));
        eprintln!("[bot] net: breath={:?} posts={}/{} live, sources={:?}, smelter(links,status)={:?}, power={:?}", grid.breath, posts, grid.posts.len(), grid.sources.iter().map(|s| s.0).collect::<Vec<_>>(), smelter, grid.focus.and_then(|f| grid.nets.of(f)).map(|n| (n.supply, n.demand, n.ratio)));
        *frames = (0, t);
    }
    let centre = Vec2::new(w.width() * 0.5, w.height() * 0.5);
    // LOSTRANOUT_ZOOM=<n>: camera zoom input for the first 3 s (negative = pull back);
    // LOSTRANOUT_LOOK=<x,y>: camera turn per frame during the first 3 s
    if t < 3.0 {
        if let Some(z) = std::env::var("LOSTRANOUT_ZOOM").ok().and_then(|s| s.parse::<f32>().ok()) {
            c.zoom = z;
        }
        if let Some((x, y)) = std::env::var("LOSTRANOUT_LOOK").ok().and_then(|s| s.split_once(',').and_then(|(x, y)| Some((x.parse::<f32>().ok()?, y.parse::<f32>().ok()?)))) {
            c.look = Vec2::new(x, y);
        }
    }
    match mode.as_str() {
        "walk" => {
            if t > 1.0 {
                c.mv = Vec2::new(0.3, 1.0);
                c.jump_pressed = (t * 2.0).fract() < 0.05;
            }
        }
        "roam" => {
            // turn away from the pod, then sprint in a wide curve, hopping now and then
            if t > 0.5 && t < 1.0 {
                c.look.x = 0.09;
            }
            if t > 1.0 {
                c.mv = Vec2::new(0.0, 1.0);
                c.sprint = true;
                c.look.x = 0.0015;
                c.jump_pressed = (t * 0.5).fract() < 0.02;
            }
        }
        "shaft" => {
            // digs straight down at the astronaut's feet and keeps walking into the pit, so the
            // camera has to cope with a narrow shaft around the player
            ui.tool_mode = crate::ToolMode::Dig;
            if t > 1.0 {
                c.pointer = Some(centre + Vec2::new(0.0, 70.0));
                c.tool = true;
                c.tool_started = t < 1.1;
                c.mv = Vec2::new(0.0, if (t * 0.5).fract() < 0.3 { 0.35 } else { 0.0 });
            }
        }
        "dig" | "fill" | "flat" => {
            ui.tool_mode = match mode.as_str() {
                "dig" => crate::ToolMode::Dig,
                "fill" => crate::ToolMode::Fill,
                _ => crate::ToolMode::Flatten,
            };
            if t > 1.0 {
                c.pointer = Some(centre + Vec2::new(120.0 * (t * 0.7).sin(), 110.0));
                c.tool = true;
                c.tool_started = t < 1.1;
            }
            if once(&mut step, t, 0, 5.0) {
                ui.actions.push(Action::Save);
            }
        }
        "place" | "travel" => {
            c.pointer = Some(centre + Vec2::new(260.0, 150.0));
            if once(&mut step, t, 0, 1.0) {
                ui.placing = Some(SK::Smelter);
            }
            if once(&mut step, t, 1, 2.0) {
                ui.place_confirm = true;
            }
            if once(&mut step, t, 2, 3.0) {
                c.pointer = Some(centre + Vec2::new(-300.0, 160.0));
                ui.placing = Some(SK::Shuttle);
            }
            if *step == 3 {
                c.pointer = Some(centre + Vec2::new(-300.0, 160.0));
            }
            if once(&mut step, t, 3, 4.0) {
                ui.place_confirm = true;
            }
            if mode == "travel" && once(&mut step, t, 4, 5.5) {
                let dest = (0..sys.0.planets.len()).find(|i| *i != game.planet).unwrap();
                let dest = std::env::var("LOSTRANOUT_DEST").ok().and_then(|s| s.parse().ok()).unwrap_or(dest);
                if let Some(s) = game.structs_mut().iter_mut().find(|s| s.kind == SK::Shuttle) {
                    s.fuel = 30.0;
                    ui.travel_from = Some(s.id);
                    ui.actions.push(Action::Travel(dest));
                }
            }
        }
        "craft" => {
            // builds a smelter (it cables itself to the pod), unplugs it, queues iron and opens
            // its panel: nothing happens until the cable is plugged back in, either by a click
            // on the panel's cable button (LOSTRANOUT_CLICKS) or by the bot at 9 s.
            c.pointer = Some(centre + Vec2::new(260.0, 150.0));
            if once(&mut step, t, 0, 1.0) {
                ui.placing = Some(SK::Smelter);
            }
            if once(&mut step, t, 1, 2.0) {
                ui.place_confirm = true;
            }
            if once(&mut step, t, 2, 3.0) {
                if let Some(id) = game.structs().iter().find(|s| s.kind == SK::Smelter).map(|s| s.id) {
                    crate::network::unlink_all(game.structs_mut(), id);
                    if game.inv.remove(crate::items::Item::IronOre, 4) {
                        if let Some(s) = game.find_struct(id) {
                            s.job = Some(crate::state::Job { recipe: 0, count: 2, progress: 0.0 });
                        }
                    }
                    ui.machine = Some(id);
                }
            }
            if std::env::var("LOSTRANOUT_CLICKS").is_err() && once(&mut step, t, 3, 9.0) {
                let pod = game.structs().iter().find(|s| s.kind == SK::Pod).map(|s| s.id);
                let sm = game.structs().iter().find(|s| s.kind == SK::Smelter).map(|s| s.id);
                if let (Some(a), Some(b)) = (pod, sm) {
                    eprintln!("[bot] plugging the smelter into the pod: {:?}", crate::network::link(game.structs_mut(), a, b));
                }
            }
        }
        "cables" => {
            // a small grid around the pod: a powered pylon line with a smelter at its end, and
            // a separate pylon pair with nothing to feed it (dead cable)
            if once(&mut step, t, 0, 1.0) {
                let pod = game.structs().iter().find(|s| s.kind == SK::Pod).cloned().unwrap();
                let pp = Vec3::from_array(pod.pos);
                let add = |game: &mut Game, kind: SK, dist: f32, ang: f32| {
                    let p = crate::world::offset_on_surface(&vw, pp, dist, ang);
                    let id = game.next_id;
                    game.next_id += 1;
                    game.structs_mut().push(crate::state::Structure::new(id, kind, p.to_array(), crate::world::upright(p, pp - p).to_array()));
                    id
                };
                let ang = std::env::var("LOSTRANOUT_ANGLE").ok().and_then(|s| s.parse().ok()).unwrap_or(2.4);
                let a = add(&mut game, SK::Pylon, 9.0, ang);
                let b = add(&mut game, SK::Pylon, 24.0, ang + 0.25);
                let m = add(&mut game, SK::Smelter, 30.0, ang + 0.15);
                let d1 = add(&mut game, SK::Pylon, 14.0, ang + 0.9);
                let d2 = add(&mut game, SK::Pylon, 26.0, ang + 0.75);
                let d3 = add(&mut game, SK::Research, 8.0, ang - 0.9);
                for (x, y) in [(pod.id, a), (a, b), (b, m), (d1, d2)] {
                    eprintln!("[bot] link {x}-{y}: {:?}", crate::network::link(game.structs_mut(), x, y));
                }
                let _ = d3;
            }
        }
        "tether" => {
            // walk away from the pod dropping tethers, pick the first one up again, go past
            // the end of the chain, then break the chain in the middle
            let n: u32 = std::env::var("LOSTRANOUT_TETHERS").ok().and_then(|s| s.parse().ok()).unwrap_or(4);
            let touch = std::env::var("LOSTRANOUT_TOUCH").is_ok();
            if once(&mut step, t, 0, 1.0) {
                net_ui.drop_tether = true;
            }
            if once(&mut step, t, 1, 1.6) {
                c.interact = true; // picks up the tether that was just dropped
            }
            if t > 2.2 && *step >= 2 && *step < 2 + n {
                c.mv = Vec2::new(0.0, -1.0);
                let at = 2.2 + 2.0 * (*step - 1) as f32;
                if t > at {
                    *step += 1;
                    if !touch {
                        net_ui.drop_tether = true;
                    }
                }
            } else if *step == 2 + n {
                c.mv = Vec2::new(0.0, -1.0);
                let stop: f32 = std::env::var("LOSTRANOUT_WALK").ok().and_then(|s| s.parse().ok()).unwrap_or(2.0);
                if t > 2.2 + 2.0 * (n as f32 - 1.0) + stop {
                    *step += 1;
                }
            } else if *step == 3 + n && std::env::var("LOSTRANOUT_BREAK").is_ok() {
                let ids: Vec<u32> = game.structs().iter().filter(|s| s.kind == SK::Tether).map(|s| s.id).collect();
                if ids.len() > 2 {
                    eprintln!("[bot] breaking the chain at tether {}", ids[1]);
                    ui.actions.push(Action::Dismantle(ids[1]));
                }
                *step += 1;
            }
            let _ = game.inv.count(crate::items::Item::Organic);
        }
        "story" => {
            // drives the story hooks: a research station decoding black box fragments next to
            // the pod, a wreck site and a hazard hit reported through story::fire
            if once(&mut step, t, 0, 1.0) {
                let pod = game.structs().iter().find(|s| s.kind == SK::Pod).cloned().unwrap();
                let pp = Vec3::from_array(pod.pos);
                let p = crate::world::offset_on_surface(&vw, pp, 6.0, 1.2);
                let id = game.next_id;
                game.next_id += 1;
                let mut st = crate::state::Structure::new(id, SK::Research, p.to_array(), crate::world::upright(p, pp - p).to_array());
                let frag = crate::items::RECIPES.iter().position(|r| r.data > 0 && r.inputs[0].0 == crate::items::Item::DataFragment).unwrap();
                st.job = Some(crate::state::Job { recipe: frag as u16, count: 3, progress: 0.0 });
                game.structs_mut().push(st);
                game.built.push(SK::Research);
                eprintln!("[bot] research station {id} decoding: {:?}", crate::network::link(game.structs_mut(), pod.id, id));
            }
            if once(&mut step, t, 1, 3.0) {
                crate::story::fire(&mut game, crate::story::StoryEvent::WreckFound { id: 1 });
            }
            if once(&mut step, t, 2, 5.0) {
                crate::story::fire(&mut game, crate::story::StoryEvent::HazardHit);
            }
            if (*step == 3) && t > 6.0 {
                *step += 1;
                eprintln!("[bot] journal={:?} quest={}", game.journal, game.quest);
            }
            if t > 6.0 && (t * 0.5).fract() < 0.01 {
                eprintln!("[bot] journal={:?} fragments={} quest={}", game.journal, game.fragments, game.quest);
            }
        }
        "finale" => {
            // back away from the pod first: the shipyard needs a wide free spot
            if t < 3.0 {
                c.mv = Vec2::new(0.0, -1.0);
            }
            c.pointer = Some(centre + Vec2::new(0.0, 200.0));
            if once(&mut step, t, 0, 3.5) {
                ui.placing = Some(SK::Shipyard);
            }
            if once(&mut step, t, 1, 4.5) {
                ui.place_confirm = true;
            }
            if once(&mut step, t, 2, 5.0) {
                if let Some(s) = game.structs_mut().iter_mut().find(|s| s.kind == SK::Shipyard) {
                    s.delivered = crate::items::SHIP_PARTS.to_vec();
                }
            }
            if once(&mut step, t, 3, 8.0) {
                if let Some(id) = game.structs().iter().find(|s| s.kind == SK::Shipyard).map(|s| s.id) {
                    ui.actions.push(Action::Finale(id));
                }
            }
        }
        _ => {}
    }
}

/// LOSTRANOUT_PLANET=<kind> (temperate, desert, ice, volcanic, barren, toxic, crystal): a new
/// game starts on the first planet of that kind instead of the home planet (visual checks).
pub fn start_planet(game: &mut Game, sys: &crate::galaxy::StarSystem) {
    // LOSTRANOUT_PLANET=<index> (a pod lands on that planet) or =<kind> (first planet of that kind)
    if let Ok(want) = std::env::var("LOSTRANOUT_PLANET") {
        let kinds: Vec<String> = sys.planets.iter().map(|p| format!("{:?}", p.kind).to_lowercase()).collect();
        let by_index = want.parse::<usize>().ok().filter(|i| *i < sys.planets.len());
        match by_index.or_else(|| kinds.iter().position(|k| *k == want.to_lowercase())) {
            Some(i) => game.planet = i,
            None => eprintln!("[bot] no {want} planet in this system: {kinds:?}"),
        }
    }
    // LOSTRANOUT_HOUR=<0..24>: local solar time at the landing site (12 = noon, ~18.5 = dusk)
    if let Some(h) = std::env::var("LOSTRANOUT_HOUR").ok().and_then(|s| s.parse::<f32>().ok()) {
        let day = sys.planets[game.planet].day_len;
        game.time = (((h - 12.0) / 24.0).rem_euclid(1.0) + 0.75 / std::f32::consts::TAU) as f64 * day as f64;
        game.seen_night = true;
    }
}

/// LOSTRANOUT_ORES=<dist>: one deposit of every ore type in a row across the view, `dist` m
/// ahead of the player (spawned through the normal decor path). LOSTRANOUT_FLORA=<dist>: the
/// same with two variants of every plant species of the planet. For judging the visuals.
#[allow(clippy::too_many_arguments)]
pub fn ore_showcase(
    mut commands: Commands,
    time: Res<Time>,
    gfx: Res<crate::world::Gfx>,
    rt: Res<crate::world::PlanetRt>,
    vw: Res<VoxelWorld>,
    rig: Res<crate::player::CamRig>,
    ds: Option<Res<crate::decor::DecorStream>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut player: Query<&mut Transform, With<crate::player::Player>>,
    mut t0: Local<Option<f32>>,
    mut done: Local<bool>,
) {
    let ores = std::env::var("LOSTRANOUT_ORES").ok();
    let flora = std::env::var("LOSTRANOUT_FLORA").ok();
    if *done || (ores.is_none() && flora.is_none()) {
        return;
    }
    let start = *t0.get_or_insert(time.elapsed_secs());
    let Ok(mut pt) = player.single_mut() else { return };
    if time.elapsed_secs() - start < 3.3 {
        return;
    }
    let Some(ds) = ds else { return };
    *done = true;
    let p = pt.translation;
    let up = p.normalize();
    let fwd = (rig.heading - up * rig.heading.dot(up)).normalize_or_zero();
    let side = up.cross(fwd);
    // step 16 m sideways first, away from the pod, so nothing hides the row
    let Some(p) = vw.surface_point((p + side * 16.0).normalize()) else { return };
    pt.translation = p + p.normalize() * 0.3;
    let up = p.normalize();
    let fwd = (fwd - up * fwd.dot(up)).normalize_or_zero();
    let side = up.cross(fwd);
    let dist: f32 = ores.as_ref().or(flora.as_ref()).and_then(|s| s.parse().ok()).filter(|d: &f32| *d > 1.5).unwrap_or(6.0);
    let kinds: Vec<(crate::world::DecorKind, u32)> = if ores.is_some() {
        crate::items::ORES.iter().map(|o| (crate::world::DecorKind::Crystal(o.0), 0)).collect()
    } else {
        (0..rt.def.flora.len()).flat_map(|s| (0..2).map(move |v| (crate::world::DecorKind::Plant(s as u8), v))).collect()
    };
    let n = kinds.len();
    let gap = if ores.is_some() { 1.6 } else { 2.2 };
    for (k, (kind, variant)) in kinds.into_iter().enumerate() {
        // leave a gap in the middle, where the astronaut stands in the camera view
        let c = k as f32 - (n - 1) as f32 * 0.5;
        let x = c * gap + c.signum() * 1.0;
        let ahead = if c == 0.0 { dist + 2.5 } else { dist };
        let dir = (p + fwd * ahead + side * x).normalize();
        let Some(pos) = vw.surface_point(dir) else { continue };
        if let crate::world::DecorKind::Plant(s) = kind {
            let up = pos.normalize();
            let mesh = meshes.add(crate::meshgen::plant_mesh(&rt.def.flora[s as usize], variant));
            let rot = Quat::from_rotation_arc(Vec3::Y, up) * Quat::from_rotation_y(k as f32 * 1.3);
            commands.spawn((Mesh3d(mesh), MeshMaterial3d(gfx.flora.clone()), Transform::from_translation(pos - up * 0.1).with_rotation(rot), crate::world::PlanetScoped));
            continue;
        }
        let d = crate::decor::Decor { id: crate::decor::FORCED_ID + 0x7000 + k as u32, kind, pos, up: pos.normalize(), yaw: k as f32 * 1.7, scale: 1.1, light: false };
        crate::decor::spawn_decor(&mut commands, &gfx, &ds, &rt.def, &d, crate::decor::FORCED_ID + 0x7000);
    }
}


fn once(step: &mut u32, t: f32, n: u32, at: f32) -> bool {
    if *step == n && t > at {
        *step += 1;
        true
    } else {
        false
    }
}

/// LOSTRANOUT_BOT=mouse: feeds a synthetic cursor + held left button into `gather_input`
/// (through `Ui::dbg_mouse`, so the real desktop pointer is never touched). The cursor is
/// parked low on the screen, left of the toolbar, where HUD hit-testing used to swallow clicks.
pub fn mouse_bot(time: Res<Time>, windows: Query<&Window, With<PrimaryWindow>>, mut ui: ResMut<Ui>, mut t0: Local<Option<f32>>) {
    if std::env::var("LOSTRANOUT_BOT").as_deref() != Ok("mouse") {
        return;
    }
    let Ok(w) = windows.single() else { return };
    let start = *t0.get_or_insert(time.elapsed_secs());
    if time.elapsed_secs() - start < 1.0 {
        return;
    }
    ui.dbg_mouse = Some(Vec2::new(w.width() * 0.3, w.height() - 150.0));
}

/// LOSTRANOUT_CLICKS="4.0:640,300;9.5:700,250": real egui clicks (press + release) at the given
/// times and logical positions, injected as egui events. Exercises the UI code exactly like a
/// user would, without touching the desktop pointer.
pub fn click_bot(time: Res<Time>, window: Query<Entity, With<PrimaryWindow>>, mut ev: EventWriter<bevy_egui::input::EguiInputEvent>, mut state: Local<(Option<f32>, usize, u8)>) {
    use bevy_egui::egui;
    let Ok(spec) = std::env::var("LOSTRANOUT_CLICKS") else { return };
    let Ok(context) = window.single() else { return };
    let start = *state.0.get_or_insert(time.elapsed_secs());
    let t = time.elapsed_secs() - start;
    let clicks: Vec<(f32, f32, f32)> = spec
        .split(';')
        .filter_map(|c| {
            let (at, pos) = c.split_once(':')?;
            let (x, y) = pos.split_once(',')?;
            Some((at.trim().parse().ok()?, x.trim().parse().ok()?, y.trim().parse().ok()?))
        })
        .collect();
    let Some(&(at, x, y)) = clicks.get(state.1) else { return };
    if t < at {
        return;
    }
    let pos = egui::pos2(x, y);
    let event = match state.2 {
        0 => egui::Event::PointerMoved(pos),
        1 => egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
        2 => egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE },
        _ => egui::Event::PointerGone,
    };
    ev.write(bevy_egui::input::EguiInputEvent { context, event });
    state.2 += 1;
    if state.2 > 3 {
        state.2 = 0;
        state.1 += 1;
        eprintln!("[bot] clicked {x},{y} at t={t:.1}");
    }
}
