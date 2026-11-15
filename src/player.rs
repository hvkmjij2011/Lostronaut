//! Input gathering (keyboard / mouse / touch), the astronaut controller and the orbit camera.
use crate::items::{Tech, SK};
use crate::state::Game;
use crate::util::any_perp;
use crate::voxel::VoxelWorld;
use crate::world::{Gfx, PlanetRt, PlanetScoped, ShipVisual, StructEnt};
use crate::{Cine, CineKind, MainCam, Tab, Ui};
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::EguiContexts;

#[derive(Component)]
pub struct Player {
    pub vel: Vec3,
    pub grounded: bool,
    pub facing: Vec3,
    pub phase: f32,
    pub using_tool: bool,
}

#[derive(Component)]
pub struct Limb(pub u8);

#[derive(Resource)]
pub struct CamRig {
    pub heading: Vec3,
    pub pitch: f32,
    pub dist: f32,
}

impl Default for CamRig {
    fn default() -> Self {
        CamRig { heading: Vec3::X, pitch: 0.35, dist: 8.0 }
    }
}

#[derive(Resource, Default)]
pub struct Controls {
    pub mv: Vec2,
    pub look: Vec2,
    pub zoom: f32,
    pub jump: bool,
    pub jump_pressed: bool,
    pub sprint: bool,
    /// tool trigger held on the world (not on UI)
    pub tool: bool,
    pub tool_started: bool,
    /// screen position used for aiming
    pub pointer: Option<Vec2>,
    pub interact: bool,
    pub touch: bool,
    pub joy_origin: Option<Vec2>,
    pub joy_pos: Vec2,
}

#[derive(Default)]
pub struct TouchState {
    joy: Option<u64>,
    look: Option<u64>,
    was_tool: bool,
    dbg_prev: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn gather_input(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    touches: Res<Touches>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut contexts: EguiContexts,
    mut c: ResMut<Controls>,
    mut ui: ResMut<Ui>,
    mut ts: Local<TouchState>,
) {
    let Ok(window) = windows.single() else { return };
    let Some(ctx) = contexts.try_ctx_mut() else { return };
    // egui always registers a full-screen background layer; only real areas count as UI.
    let over_ui = |p: Vec2| ctx.layer_id_at(bevy_egui::egui::pos2(p.x, p.y)).is_some_and(|l| l.order != bevy_egui::egui::Order::Background);
    let typing = ctx.wants_keyboard_input();
    let (w, h) = (window.width(), window.height());

    let was_tool = ts.was_tool;
    c.mv = Vec2::ZERO;
    c.look = Vec2::ZERO;
    c.zoom = 0.0;
    c.tool = false;
    c.interact = false;
    c.jump_pressed = false;
    c.jump = false;
    c.sprint = false;
    if mobile_or_touch(&touches) {
        c.touch = true;
    }

    // ---- keyboard
    if !typing {
        let k = |a: KeyCode, b: KeyCode| keys.pressed(a) || keys.pressed(b);
        if k(KeyCode::KeyW, KeyCode::ArrowUp) {
            c.mv.y += 1.0;
        }
        if k(KeyCode::KeyS, KeyCode::ArrowDown) {
            c.mv.y -= 1.0;
        }
        if k(KeyCode::KeyD, KeyCode::ArrowRight) {
            c.mv.x += 1.0;
        }
        if k(KeyCode::KeyA, KeyCode::ArrowLeft) {
            c.mv.x -= 1.0;
        }
        c.jump = keys.pressed(KeyCode::Space);
        c.jump_pressed = keys.just_pressed(KeyCode::Space);
        c.sprint = keys.pressed(KeyCode::ShiftLeft);
        c.interact = keys.just_pressed(KeyCode::KeyE);
        if keys.just_pressed(KeyCode::Digit1) {
            ui.tool_mode = crate::ToolMode::Dig;
        }
        if keys.just_pressed(KeyCode::Digit2) {
            ui.tool_mode = crate::ToolMode::Fill;
        }
        if keys.just_pressed(KeyCode::Digit3) {
            ui.tool_mode = crate::ToolMode::Flatten;
        }
        if keys.just_pressed(KeyCode::KeyZ) {
            ui.brush = (ui.brush - 0.5).max(1.5);
        }
        if keys.just_pressed(KeyCode::KeyX) {
            ui.brush = (ui.brush + 0.5).min(ui.max_brush);
        }
        for (key, tab) in [(KeyCode::Tab, Tab::Inventory), (KeyCode::KeyI, Tab::Inventory), (KeyCode::KeyB, Tab::Build), (KeyCode::KeyR, Tab::Research), (KeyCode::KeyM, Tab::Map), (KeyCode::KeyJ, Tab::Journal)] {
            if keys.just_pressed(key) {
                if ui.open && ui.tab == tab {
                    ui.open = false;
                } else {
                    ui.open = true;
                    ui.tab = tab;
                    ui.machine = None;
                }
            }
        }
        if keys.just_pressed(KeyCode::Escape) {
            if ui.placing.is_some() {
                ui.placing = None;
            } else if ui.open || ui.machine.is_some() {
                ui.open = false;
                ui.machine = None;
                ui.travel_from = None;
            } else {
                ui.paused = !ui.paused;
            }
        }
    }

    // ---- mouse
    let cursor = ui.dbg_mouse.or(window.cursor_position());
    let cursor_on_ui = cursor.map(|p| over_ui(p)).unwrap_or(true);
    let lmb = buttons.pressed(MouseButton::Left) || ui.dbg_mouse.is_some();
    let lmb_just = buttons.just_pressed(MouseButton::Left) || (ui.dbg_mouse.is_some() && !ts.dbg_prev);
    ts.dbg_prev = ui.dbg_mouse.is_some();
    if buttons.pressed(MouseButton::Right) || buttons.pressed(MouseButton::Middle) {
        c.look += motion.delta * 0.0045;
    }
    if !cursor_on_ui {
        c.zoom += scroll.delta.y * if scroll.delta.y.abs() > 3.0 { 0.02 } else { 1.0 };
    }
    if !c.touch || touches.iter().count() == 0 {
        if let Some(p) = cursor {
            c.pointer = Some(p);
        }
        if lmb && !cursor_on_ui && (was_tool || lmb_just) {
            c.tool = true;
        }
    }

    // ---- touch
    for t in touches.iter_just_pressed() {
        let p = t.position();
        if over_ui(p) {
            continue;
        }
        if p.x < w * 0.4 && p.y > h * 0.4 && ts.joy.is_none() {
            ts.joy = Some(t.id());
            c.joy_origin = Some(p);
            c.joy_pos = p;
        } else if ts.look.is_none() {
            ts.look = Some(t.id());
        }
    }
    if let Some(id) = ts.joy {
        match touches.get_pressed(id) {
            Some(t) => {
                let o = c.joy_origin.unwrap_or(t.position());
                let d = (t.position() - o) / (h * 0.11);
                let d = if d.length() > 1.0 { d.normalize() } else { d };
                c.joy_pos = o + d * h * 0.11;
                c.mv = Vec2::new(d.x, -d.y);
                c.sprint = d.length() > 0.95;
            }
            None => {
                ts.joy = None;
                c.joy_origin = None;
            }
        }
    }
    if let Some(id) = ts.look {
        match touches.get_pressed(id) {
            Some(t) => {
                if ui.tool_armed || ui.placing.is_some() {
                    c.pointer = Some(t.position());
                    c.tool = ui.tool_armed && ui.placing.is_none();
                } else {
                    c.look += t.delta() * 0.006;
                }
            }
            None => ts.look = None,
        }
    }
    // two-finger pinch is not needed: zoom buttons live in the HUD
    c.zoom += ui.v_zoom;
    ui.v_zoom = 0.0;
    if ui.v_jump {
        c.jump = true;
        if !ui.v_jump_prev {
            c.jump_pressed = true;
        }
    }
    ui.v_jump_prev = ui.v_jump;
    if ui.v_interact {
        c.interact = true;
        ui.v_interact = false;
    }
    if c.mv.length() > 1.0 {
        c.mv = c.mv.normalize();
    }
    c.tool_started = c.tool && !was_tool;
    ts.was_tool = c.tool;
}

fn mobile_or_touch(touches: &Touches) -> bool {
    cfg!(target_os = "android") || touches.iter().count() > 0
}

pub fn spawn_player(commands: &mut Commands, gfx: &Gfx, pos: Vec3) {
    let up = pos.normalize();
    let facing = any_perp(up);
    let root = commands
        .spawn((
            Transform::from_translation(pos).looking_to(facing, up),
            Visibility::Visible,
            Player { vel: Vec3::ZERO, grounded: false, facing, phase: 0.0, using_tool: false },
            PlanetScoped,
        ))
        .with_children(|p| {
            p.spawn((
                SpotLight { intensity: 900_000.0, range: 30.0, outer_angle: 0.75, inner_angle: 0.4, shadows_enabled: false, ..default() },
                Transform::from_xyz(0.0, 1.75, -0.2).looking_to(Vec3::new(0.0, -0.25, -1.0), Vec3::Y),
            ));
            p.spawn((PointLight { intensity: 40_000.0, range: 9.0, shadows_enabled: false, ..default() }, Transform::from_xyz(0.0, 2.3, 0.3)));
        })
        .id();
    // Blender-made astronaut (tools/blender/astronaut.py); limbs swing about their pivots.
    let ents = gfx.models.spawn(commands, root, "astronaut", &gfx.vc, &gfx.glow, None);
    for (part, e) in gfx.models.get("astronaut").parts.iter().zip(ents) {
        let limb = match part.name.as_str() {
            "arm_l" => Some(0),
            "arm_r" => Some(1),
            "leg_l" => Some(2),
            "leg_r" => Some(3),
            _ => None,
        };
        if let Some(l) = limb {
            commands.entity(e).insert(Limb(l));
        }
    }
}

fn collide(vw: &VoxelWorld, pos: &mut Vec3, vel: &mut Vec3) {
    let up = pos.normalize();
    let u = any_perp(up);
    let v = up.cross(u);
    for _ in 0..2 {
        let mut probes = [Vec3::ZERO; 10];
        probes[0] = Vec3::ZERO;
        probes[1] = up * 1.8;
        for k in 0..8 {
            let a = k as f32 * std::f32::consts::FRAC_PI_4;
            probes[2 + k] = up * if k % 2 == 0 { 0.75 } else { 1.35 } + (u * a.cos() + v * a.sin()) * 0.36;
        }
        for o in probes {
            let p = *pos + o;
            let d = vw.sample(p);
            if d > 0.0 {
                let n = vw.normal(p);
                *pos += n * d.min(0.6);
                let vn = vel.dot(n);
                if vn < 0.0 {
                    *vel -= n * vn;
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn player_move(
    time: Res<Time>,
    c: Res<Controls>,
    vw: Res<VoxelWorld>,
    rt: Res<PlanetRt>,
    mut game: ResMut<Game>,
    rig: Res<CamRig>,
    ui: Res<Ui>,
    cine: Res<Cine>,
    mut q: Query<(&mut Transform, &mut Player, &mut Visibility)>,
    mut limbs: Query<(&mut Transform, &Limb), Without<Player>>,
) {
    let Ok((mut tf, mut pl, mut vis)) = q.single_mut() else { return };
    if cine.0.is_some() {
        *vis = Visibility::Hidden;
        return;
    }
    *vis = Visibility::Visible;
    if ui.paused {
        return;
    }
    let dt = time.delta_secs().min(0.05);
    let mut pos = tf.translation;
    let up = pos.normalize();
    let heading = (rig.heading - up * rig.heading.dot(up)).normalize_or(any_perp(up));
    let right = heading.cross(up);
    let blocked = ui.open || ui.machine.is_some();
    let mv = if blocked { Vec2::ZERO } else { c.mv };
    let wish = heading * mv.y + right * mv.x;
    let speed = if c.sprint { 8.0 } else { 5.2 };
    let g = rt.def.gravity;

    let mut vel = pl.vel;
    let mut vn = vel.dot(up);
    let mut vt = vel - up * vn;
    let k = if pl.grounded { 12.0 } else { 2.5 };
    vt = vt.lerp(wish * speed, 1.0 - (-dt * k).exp());
    vn -= g * dt;
    if pl.grounded && c.jump_pressed && !blocked {
        vn = 6.4;
        pl.grounded = false;
    } else if !pl.grounded && c.jump && !blocked && game.has_tech(Tech::Jetpack) && game.jet > 0.0 {
        vn = (vn + (g + 16.0) * dt).min(7.0);
        game.jet = (game.jet - 24.0 * dt).max(0.0);
    }
    if pl.grounded {
        game.jet = (game.jet + 35.0 * dt).min(100.0);
    }
    vel = vt + up * vn;
    let fall_speed = -vn;

    let steps = ((vel.length() * dt / 0.3).ceil() as i32).clamp(1, 8);
    for _ in 0..steps {
        pos += vel * dt / steps as f32;
        collide(&vw, &mut pos, &mut vel);
    }
    // structures are simple upright cylinders
    for s in game.structs() {
        let sp = Vec3::from_array(s.pos);
        let d = pos - sp;
        let sup = sp.normalize();
        let hgt = d.dot(sup);
        let flat = d - sup * hgt;
        let r = s.kind.def().radius * if s.kind == SK::Shipyard { 0.0 } else { 0.8 } + 0.3;
        if hgt > -1.0 && hgt < 2.5 && flat.length() < r && flat.length() > 1e-3 && s.kind != SK::Shipyard {
            pos += flat.normalize() * (r - flat.length());
        }
    }
    let up = pos.normalize();
    let was_grounded = pl.grounded;
    pl.grounded = vw.sample(pos - up * 0.2) > 0.0;
    if !pl.grounded && was_grounded && vel.dot(up) <= 0.5 {
        if let Some(hit) = vw.raycast(pos, -up, 0.6) {
            pos = hit;
            pl.grounded = true;
        }
    }
    if pl.grounded && !was_grounded && fall_speed > 15.0 {
        game.health -= (fall_speed - 15.0) * 4.0;
    }
    if pl.grounded && wish.length() < 0.05 {
        vel *= 0.5;
    }
    // keep inside the playable shell
    let lim = (vw.half * crate::voxel::CS) as f32 - 6.0;
    if pos.length() > lim {
        pos = pos.normalize() * lim;
        let vn = vel.dot(up);
        if vn > 0.0 {
            vel -= up * vn;
        }
    }
    if pos.length() < vw.gen.core {
        pos = pos.normalize_or(Vec3::Y) * (vw.gen.core + 1.0);
    }
    pl.vel = vel;

    let mut facing = pl.facing - up * pl.facing.dot(up);
    if wish.length() > 0.1 {
        facing = facing.normalize_or(wish).lerp(wish.normalize(), (dt * 10.0).min(1.0));
    }
    pl.facing = facing.normalize_or(any_perp(up));
    *tf = Transform::from_translation(pos).looking_to(pl.facing, up);

    // procedural walk cycle
    let hs = (vel - up * vel.dot(up)).length();
    if pl.grounded {
        pl.phase += hs * dt * 2.2;
    }
    let swing = if pl.grounded { (hs / 5.0).min(1.3) * 0.8 } else { 0.3 };
    for (mut lt, limb) in &mut limbs {
        let s = pl.phase.sin() * swing;
        let a = match limb.0 {
            0 => s,
            1 => {
                if pl.using_tool {
                    1.35
                } else {
                    -s
                }
            }
            2 => -s,
            _ => s,
        };
        lt.rotation = Quat::from_rotation_x(a);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn camera_follow(
    c: Res<Controls>,
    mut rig: ResMut<CamRig>,
    time: Res<Time>,
    vw: Res<VoxelWorld>,
    cine: Res<Cine>,
    ui: Res<Ui>,
    player: Query<&Transform, (With<Player>, Without<MainCam>)>,
    structs: Query<(&GlobalTransform, &StructEnt)>,
    ships: Query<(&GlobalTransform, &ShipVisual)>,
    mut cam: Query<&mut Transform, With<MainCam>>,
) {
    let (Ok(pt), Ok(mut ct)) = (player.single(), cam.single_mut()) else { return };
    let mut focus = pt.translation;
    let mut dist_mul = 1.0;
    let mut lift = 1.5;
    if let Some((kind, _, id)) = cine.0 {
        if kind == CineKind::Finale {
            if let Some((g, _)) = ships.iter().find(|(_, s)| s.0 == id) {
                focus = g.translation();
                dist_mul = 3.4;
                lift = 6.0;
            }
        } else if let Some((g, _)) = structs.iter().find(|(_, s)| s.0 == id) {
            focus = g.translation();
            dist_mul = 2.2;
            lift = 3.0;
        }
    }
    let up = pt.translation.normalize();
    let mut heading = (rig.heading - up * rig.heading.dot(up)).normalize_or(any_perp(up));
    if !ui.paused {
        heading = Quat::from_axis_angle(up, -c.look.x) * heading;
        rig.pitch = (rig.pitch + c.look.y).clamp(-0.45, 1.45);
        rig.dist = (rig.dist - c.zoom * 0.9).clamp(3.0, 22.0);
    }
    rig.heading = heading;
    // The look-at point must be in open air (the player may stand in a fresh, tight hole).
    let mut target = focus + up * lift;
    for _ in 0..5 {
        if vw.sample(target) <= -0.05 {
            break;
        }
        target -= up * 0.3;
    }
    let want = rig.dist * dist_mul;
    // Free length from the target along `dir`, keeping 0.5 m of clearance from the ground.
    let free = |dir: Vec3| -> f32 {
        if vw.sample(target) > 0.0 {
            return 0.0;
        }
        match vw.raycast(target, dir, want + 0.6) {
            Some(h) => ((h - target).length() - 0.5).max(0.0),
            None => want,
        }
    };
    // The player's own pitch first; when that is blocked (digging, tunnels, walls) try steeper
    // views over the rim, straight down last. The camera never goes into the ground: it either
    // finds open air with a clear line of sight to the target or comes close to the target.
    let comfy = (want * 0.6).min(3.0);
    let mut best = (-1.0f32, -heading * rig.pitch.cos() + up * rig.pitch.sin());
    for p in [rig.pitch, rig.pitch + 0.35, rig.pitch + 0.7, rig.pitch + 1.05, 1.5] {
        let p = p.min(1.5);
        let dir = (-heading * p.cos() + up * p.sin()).normalize();
        let d = free(dir).min(want);
        if d >= comfy {
            best = (d, dir);
            break;
        }
        if d > best.0 + 0.3 {
            best = (d, dir);
        }
    }
    let (d, dir) = (best.0.max(0.2), best.1);
    let pos = target + dir * d;
    // Smooth the motion, but only along paths that stay in the open: the smoothed point must
    // see the target and must not sit in (or right against) solid ground, otherwise snap.
    let k = 1.0 - (-time.delta_secs() * 12.0).exp();
    let cand = ct.translation.lerp(pos, k);
    let to = cand - target;
    let len = to.length();
    let clear = len < 1e-3 || (vw.sample(cand) < -0.3 && vw.raycast(target, to / len, len).is_none());
    ct.translation = if clear && cand.distance(pos) < 25.0 { cand } else { pos };
    ct.look_at(target, up);
}
