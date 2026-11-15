//! Scene management: shared render assets, planet loading, terrain chunks, decor, sky.
use crate::galaxy::{PlanetDef, StarSystem};
use crate::items::{Item, ORES, SK};
use crate::meshgen;
use crate::player::{spawn_player, CamRig, Player};
use crate::state::{Game, Structure};
use crate::story;
use crate::util::{any_perp, smoothstep, Rng};
use crate::voxel::{Brush, TerrainGen, VoxelWorld};
use crate::{AppState, Cine, CineKind, MainCam, Sys, Ui};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::pbr::{CascadeShadowConfigBuilder, DistanceFog, FogFalloff, NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use std::collections::HashMap;
use std::f32::consts::TAU;

#[derive(Component)]
pub struct PlanetScoped;
#[derive(Component)]
pub struct MenuScoped;
#[derive(Component)]
pub struct ChunkEnt;
#[derive(Component)]
pub struct StructEnt(pub u32);
#[derive(Component)]
pub struct ShipVisual(pub u32, pub u64);
#[derive(Component)]
pub struct SkyRoot;
#[derive(Component)]
pub struct SunLight;
#[derive(Component)]
pub struct Spin(pub f32);

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum DecorKind {
    Plant(u8),
    Crystal(Item),
    Sample,
    Fragment,
    /// wreckage of the Öncü (gives `Item::Scrap`)
    Scrap,
    /// light-giving cave growth (variant)
    Glow(u8),
}

#[derive(Component)]
pub struct DecorEnt {
    pub id: u32,
    pub kind: DecorKind,
    pub radius: f32,
}

#[derive(Resource)]
pub struct Gfx {
    pub vc: Handle<StandardMaterial>,
    /// vertex colours + per-vertex glow (meshes must carry UV0, as every `MB` mesh does)
    pub flora: Handle<StandardMaterial>,
    pub terrain: Handle<StandardMaterial>,
    pub shiny: Handle<StandardMaterial>,
    pub glow: Handle<StandardMaterial>,
    pub sky: Handle<StandardMaterial>,
    pub stars: Handle<StandardMaterial>,
    pub sun: Handle<StandardMaterial>,
    /// sky gradient dome (unlit vertex colours, rewritten by `update_sky`)
    pub sky_dome: Handle<StandardMaterial>,
    pub halo: Handle<StandardMaterial>,
    pub moon: Handle<StandardMaterial>,
    /// hemisphere ambient cubemap used by the camera's `EnvironmentMapLight`
    pub env: Handle<Image>,
    pub dome: Handle<Mesh>,
    pub dome_dirs: Vec<Vec3>,
    pub halo_mesh: Handle<Mesh>,
    pub ghost_ok: Handle<StandardMaterial>,
    pub ghost_bad: Handle<StandardMaterial>,
    pub brush: Handle<StandardMaterial>,
    pub brush_idle: Handle<StandardMaterial>,
    pub struct_mesh: HashMap<SK, Handle<Mesh>>,
    pub crystal: HashMap<Item, Vec<Handle<Mesh>>>,
    pub sample: Handle<Mesh>,
    pub fragment: Handle<Mesh>,
    pub flame: Handle<Mesh>,
    pub sphere: Handle<Mesh>,
    pub star_dome: Handle<Mesh>,
    pub models: crate::models::ModelLib,
}

#[derive(Resource)]
pub struct PlanetRt {
    pub def: PlanetDef,
    pub landing: Vec3,
    pub sun_a0: f32,
}

#[derive(Resource, Default)]
pub struct Sky {
    pub sun_dir: Vec3,
    pub day: f32,
}

/// Edge length of each face of the ambient cubemap.
const ENV_SIZE: u32 = 12;

#[derive(Component)]
pub struct SkyDome;
#[derive(Component)]
pub struct MoonDisc;

/// Phone quality tier. On desktop `LOSTRANOUT_MOBILE=1` previews it (for checks only).
pub fn mobile() -> bool {
    static PREVIEW: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    cfg!(target_os = "android") || *PREVIEW.get_or_init(|| std::env::var("LOSTRANOUT_MOBILE").is_ok())
}

pub fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>, mut images: ResMut<Assets<Image>>) {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension};
    // Hemisphere ambient: a tiny generated cubemap (sky above, ground bounce below) rewritten
    // by `update_sky` and turned to the player's up with `EnvironmentMapLight::rotation`.
    let mut env = Image::new(
        Extent3d { width: ENV_SIZE, height: ENV_SIZE, depth_or_array_layers: 6 },
        TextureDimension::D2,
        vec![90u8; (ENV_SIZE * ENV_SIZE * 6 * 4) as usize],
        TextureFormat::Rgba8Unorm,
        bevy::asset::RenderAssetUsages::all(),
    );
    env.texture_view_descriptor = Some(TextureViewDescriptor { dimension: Some(TextureViewDimension::Cube), ..default() });
    let env = images.add(env);
    // Quality tier: the phone renders without HDR/bloom, MSAA and shadows; the desktop gets
    // HDR with a soft bloom (glowing ores, plants and the sun), 4x MSAA and soft shadows.
    let cam = commands
        .spawn((
            Camera3d::default(),
            Camera { hdr: !mobile(), ..default() },
            Projection::Perspective(PerspectiveProjection { fov: 1.05, near: 0.06, far: 3000.0, ..default() }),
            Tonemapping::AcesFitted,
            if mobile() { Msaa::Off } else { Msaa::Sample4 },
            DistanceFog { color: Color::BLACK, falloff: FogFalloff::Linear { start: 2000.0, end: 2500.0 }, ..default() },
            EnvironmentMapLight { diffuse_map: env.clone(), specular_map: env.clone(), intensity: 300.0, rotation: Quat::IDENTITY, affects_lightmapped_mesh_diffuse: true },
            Transform::from_xyz(0.0, 0.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
            MainCam,
        ))
        .id();
    if !mobile() {
        commands.entity(cam).insert((
            bevy::core_pipeline::bloom::Bloom { intensity: 0.12, low_frequency_boost: 0.5, ..bevy::core_pipeline::bloom::Bloom::NATURAL },
            bevy::pbr::ShadowFilteringMethod::Gaussian,
        ));
    }

    let unlit = |c: Color, alpha: bool| StandardMaterial {
        base_color: c,
        unlit: true,
        fog_enabled: false,
        alpha_mode: if alpha { AlphaMode::Blend } else { AlphaMode::Opaque },
        ..default()
    };
    let ghost = |c: Color| StandardMaterial { base_color: c, alpha_mode: AlphaMode::Blend, unlit: true, ..default() };
    let models = crate::models::ModelLib::load(&mut meshes);
    let mut struct_mesh = HashMap::new();
    for k in [SK::Pod].into_iter().chain(SK::BUILDABLE) {
        let h = match models.models.get(&format!("{k:?}")) {
            Some(m) => m.merged.clone(),
            None => meshes.add(meshgen::structure_mesh(k)),
        };
        struct_mesh.insert(k, h);
    }
    let mut crystal = HashMap::new();
    for (item, _, _) in ORES {
        crystal.insert(item, (0..3).map(|v| meshes.add(meshgen::crystal_mesh(item, v))).collect());
    }
    // per-vertex glow lookup (see meshgen::glow_uv); emission of 1.0 = full white before
    // tonemapping, alpha 0 keeps it out of the camera exposure
    let mut lut = Image::new(
        bevy::render::render_resource::Extent3d { width: meshgen::GLOW_W, height: meshgen::GLOW_H, depth_or_array_layers: 1 },
        bevy::render::render_resource::TextureDimension::D2,
        meshgen::glow_lut(),
        bevy::render::render_resource::TextureFormat::Rgba8Unorm,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    lut.sampler = bevy::image::ImageSampler::nearest();
    let lut = images.add(lut);
    let glow_k = if mobile() { 1.6 } else { 2.4 };
    let emit = LinearRgba::new(glow_k, glow_k, glow_k, 0.0);
    let (dome_mesh, dome_dirs) = meshgen::sky_dome(if mobile() { 24 } else { 40 });
    commands.insert_resource(Gfx {
        vc: mats.add(StandardMaterial { perceptual_roughness: 0.92, reflectance: 0.2, ..default() }),
        flora: mats.add(StandardMaterial { perceptual_roughness: 0.85, reflectance: 0.25, emissive: emit, emissive_texture: Some(lut.clone()), ..default() }),
        terrain: mats.add(StandardMaterial { perceptual_roughness: 0.95, reflectance: 0.08, cull_mode: None, double_sided: true, ..default() }),
        shiny: mats.add(StandardMaterial { perceptual_roughness: 0.22, reflectance: 0.5, emissive: emit, emissive_texture: Some(lut.clone()), ..default() }),
        glow: mats.add(StandardMaterial { unlit: true, ..default() }),
        sky: mats.add(StandardMaterial { fog_enabled: false, perceptual_roughness: 1.0, ..default() }),
        stars: mats.add(unlit(Color::WHITE, true)),
        sun: mats.add(unlit(Color::srgb(1.0, 0.97, 0.85), false)),
        sky_dome: mats.add(StandardMaterial { unlit: true, fog_enabled: false, cull_mode: None, ..default() }),
        halo: mats.add(StandardMaterial { unlit: true, fog_enabled: false, cull_mode: None, alpha_mode: AlphaMode::Add, ..default() }),
        moon: mats.add(unlit(Color::srgb(0.8, 0.84, 0.95), false)),
        env,
        dome: meshes.add(dome_mesh),
        dome_dirs,
        halo_mesh: meshes.add(meshgen::halo_disc(24)),
        ghost_ok: mats.add(ghost(Color::srgba(0.3, 1.0, 0.5, 0.45))),
        ghost_bad: mats.add(ghost(Color::srgba(1.0, 0.25, 0.2, 0.45))),
        brush: mats.add(ghost(Color::srgba(0.4, 0.9, 1.0, 0.16))),
        brush_idle: mats.add(ghost(Color::srgba(0.4, 0.9, 1.0, 0.06))),
        struct_mesh,
        crystal,
        sample: meshes.add(meshgen::sample_mesh()),
        fragment: meshes.add(meshgen::fragment_mesh()),
        flame: meshes.add(meshgen::flame_mesh()),
        sphere: meshes.add(meshgen::simple_sphere(16)),
        star_dome: meshes.add(meshgen::star_dome(7, 1400)),
        models,
    });
}

// ---------------------------------------------------------------- menu scene

pub fn enter_menu(
    mut commands: Commands,
    gfx: Res<Gfx>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut cam: Query<(&mut Transform, &mut DistanceFog, Option<&mut EnvironmentMapLight>), With<MainCam>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut clear: ResMut<ClearColor>,
    mut ambient: ResMut<AmbientLight>,
    time: Res<Time>,
    existing: Query<(), With<MenuScoped>>,
) {
    if !existing.is_empty() {
        return;
    }
    let seed = 1000 + (time.elapsed_secs_f64() * 1000.0) as u64 % 50;
    let sys = StarSystem::generate(seed, 0);
    let def = &sys.planets[sys.home];
    let gen = TerrainGen::new(def);
    clear.0 = Color::srgb(0.005, 0.006, 0.015);
    ambient.brightness = 40.0;
    ambient.color = Color::WHITE;
    if let Ok((mut t, mut fog, env)) = cam.single_mut() {
        *t = Transform::from_xyz(0.0, 0.3, 4.2).looking_at(Vec3::new(-1.3, 0.0, 0.0), Vec3::Y);
        fog.falloff = FogFalloff::Linear { start: 2000.0, end: 2500.0 };
        // space: no sky light, only a faint fill (the planet's day sky may still be in the map)
        if let Some(mut env) = env {
            env.intensity = 40.0;
        }
    }
    // the stars may have been faded out by a planet's daylight
    if let Some(m) = mats.get_mut(&gfx.stars) {
        m.base_color = Color::WHITE;
    }
    commands.spawn((
        Mesh3d(meshes.add(meshgen::planet_ball(&gen, 64))),
        MeshMaterial3d(gfx.sky.clone()),
        Transform::from_xyz(0.9, -0.1, 0.0).with_scale(Vec3::splat(1.5)),
        Spin(0.05),
        MenuScoped,
    ));
    commands.spawn((
        Mesh3d(gfx.star_dome.clone()),
        MeshMaterial3d(gfx.stars.clone()),
        Transform::from_scale(Vec3::splat(900.0)),
        NotShadowCaster,
        MenuScoped,
    ));
    commands.spawn((
        DirectionalLight { illuminance: 9000.0, shadows_enabled: false, ..default() },
        Transform::from_xyz(-4.0, 2.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
        MenuScoped,
    ));
}

pub fn spin(time: Res<Time>, mut q: Query<(&mut Transform, &Spin)>) {
    for (mut t, s) in &mut q {
        t.rotate_y(s.0 * time.delta_secs());
    }
}

pub fn despawn_scoped<T: Component>(mut commands: Commands, q: Query<Entity, With<T>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

// ---------------------------------------------------------------- planet loading

fn pick_landing(_def: &PlanetDef, gen: &TerrainGen) -> Vec3 {
    // chosen by the generator, which keeps cave openings away from it
    gen.landing
}

/// Point on the surface `dist` metres from `from`, in the tangent direction given by `angle`.
pub fn offset_on_surface(vw: &VoxelWorld, from: Vec3, dist: f32, angle: f32) -> Vec3 {
    let up = from.normalize();
    let u = any_perp(up);
    let v = up.cross(u);
    let dir = (from + (u * angle.cos() + v * angle.sin()) * dist).normalize();
    vw.surface_point(dir).unwrap_or(dir * from.length())
}

pub fn upright(pos: Vec3, face: Vec3) -> Quat {
    let up = pos.normalize();
    let mut f = face - up * face.dot(up);
    if f.length_squared() < 1e-6 {
        f = any_perp(up);
    }
    Transform::IDENTITY.looking_to(f.normalize(), up).rotation
}

pub fn spawn_structure(commands: &mut Commands, gfx: &Gfx, s: &Structure) -> Entity {
    let pos = Vec3::from_array(s.pos);
    let name = format!("{:?}", s.kind);
    let root = commands.spawn((Transform::from_translation(pos).with_rotation(Quat::from_array(s.rot)), Visibility::default(), StructEnt(s.id), PlanetScoped)).id();
    if gfx.models.models.contains_key(&name) {
        gfx.models.spawn(commands, root, &name, &gfx.vc, &gfx.glow, Some(s.id));
    } else {
        commands.entity(root).insert((Mesh3d(gfx.struct_mesh[&s.kind].clone()), MeshMaterial3d(gfx.vc.clone())));
    }
    let mut e = commands.entity(root);
    let light = match s.kind {
        SK::Pod => Some((Color::srgb(1.0, 0.85, 0.7), 3.2)),
        SK::Beacon => Some((Color::srgb(0.5, 0.95, 1.0), 2.6)),
        SK::Smelter => Some((Color::srgb(1.0, 0.6, 0.2), 1.6)),
        SK::Research => Some((Color::srgb(0.5, 0.8, 1.0), 1.6)),
        SK::Shipyard => Some((Color::srgb(1.0, 0.9, 0.8), 5.0)),
        _ => None,
    };
    if let (Some((color, h)), false) = (light, mobile()) {
        e.with_children(|p| {
            p.spawn((PointLight { color, intensity: 220_000.0, range: 16.0, shadows_enabled: false, ..default() }, Transform::from_xyz(0.0, h, -0.5)));
        });
    }
    if s.kind == SK::Shipyard {
        let id = s.id;
        e.with_children(|p| {
            p.spawn((Transform::from_xyz(0.0, 0.5, 0.0), Visibility::Inherited, ShipVisual(id, u64::MAX)));
        });
    }
    e.id()
}

#[allow(clippy::too_many_arguments)]
pub fn load_planet(
    mut commands: Commands,
    mut game: ResMut<Game>,
    sys: Res<Sys>,
    gfx: Res<Gfx>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut next: ResMut<NextState<AppState>>,
    mut frames: Local<u32>,
    mut cine: ResMut<Cine>,
    mut ui: ResMut<Ui>,
    mut rig: ResMut<CamRig>,
    mut ambient: ResMut<AmbientLight>,
) {
    *frames += 1;
    if *frames < 3 {
        return;
    }
    *frames = 0;
    let pi = game.planet;
    let def = sys.0.planets[pi].clone();
    let home = pi == sys.0.home;
    let t_load = std::time::Instant::now();
    let mut vw = VoxelWorld::new(&def);
    vw.import_edits(&game.planets[pi].edits);

    // landing site (flattened once, then persisted through the edit list)
    let first_visit = !game.planets[pi].visited;
    let landing = match game.planets[pi].landing {
        Some(l) => Vec3::from_array(l),
        None => {
            let dir = pick_landing(&def, &vw.gen);
            let p = vw.surface_point(dir).unwrap_or(dir * def.radius);
            for _ in 0..4 {
                vw.edit(p, 9.0, &Brush::Flatten { anchor: p, normal: dir }, 1.0);
            }
            let pad = offset_on_surface(&vw, p, 11.0, 0.0);
            let pad = pad.normalize() * pad.length().max(p.length() - 1.5).min(p.length() + 1.5);
            for _ in 0..4 {
                vw.edit(pad, 5.5, &Brush::Flatten { anchor: pad, normal: pad.normalize() }, 1.0);
            }
            game.planets[pi].landing = Some(p.to_array());
            p
        }
    };
    let up = landing.normalize();
    let pad = offset_on_surface(&vw, landing, 11.0, 0.0);

    // arriving structures
    let mut player_pos = game.player_pos.map(Vec3::from_array);
    let mut land_cine = None;
    if let Some(mut shuttle) = game.carried_shuttle.take() {
        shuttle.pos = pad.to_array();
        shuttle.rot = upright(pad, landing - pad).to_array();
        land_cine = Some(shuttle.id);
        game.planets[pi].structs.push(shuttle);
        player_pos = Some(offset_on_surface(&vw, pad, 3.2, 1.0));
    } else if game.planets[pi].structs.is_empty() && player_pos.is_none() {
        let id = game.next_id;
        game.next_id += 1;
        let tangent = any_perp(up);
        game.planets[pi].structs.push(Structure::new(id, SK::Pod, (landing - up * 0.15).to_array(), upright(landing, tangent).to_array()));
        player_pos = Some(offset_on_surface(&vw, landing, 4.5, 2.0));
    }
    let mut ppos = player_pos.unwrap_or(landing + up * 2.0);
    if vw.sample(ppos + ppos.normalize() * 0.5) > 0.0 {
        ppos = vw.surface_point(ppos.normalize()).unwrap_or(ppos);
    }
    // developer switch LOSTRANOUT_GOTO: start in a cave hall, at an entrance, a wreck...
    let dbg_spot = crate::decor::debug_spot(&def, &vw, landing);
    if let Some((p, _)) = dbg_spot {
        ppos = p;
    }
    if first_visit {
        game.planets[pi].visited = true;
        if game.travels > 0 || sys.0.index > 0 {
            game.data += 150;
            ui.toast(match game.lang {
                crate::items::Lang::Tr => format!("Yeni dünya: {} (+150 Veri)", def.name),
                crate::items::Lang::En => format!("New world: {} (+150 Data)", def.name),
            }, [0.5, 0.9, 1.0]);
        }
        if game.travels > 0 && story::unlock(&mut game, story::J_TRAVEL) {
            ui.journal_toast(game.lang);
        }
        if sys.0.index > 0 && game.travels == 0 {
            story::unlock(&mut game, story::J_NEXT);
        }
        if game.journal.is_empty() {
            story::unlock(&mut game, story::J_AWAKE);
            story::unlock(&mut game, story::J_TOOL);
        }
    }

    // terrain around the player (the rest streams in), far shell, decor
    let streamer = crate::terrain::Streamer::prime(&mut vw, ppos, &mut commands, &mut meshes, &mut mats, &gfx);
    let decor = crate::decor::DecorStream::new(&def, &vw, landing, home, &mut meshes);

    // structures
    for s in &game.planets[pi].structs {
        spawn_structure(&mut commands, &gfx, s);
    }

    // player
    spawn_player(&mut commands, &gfx, ppos);
    let pup = ppos.normalize();
    rig.heading = any_perp(pup);
    if let Some(t) = (landing - ppos).try_normalize() {
        let t = t - pup * t.dot(pup);
        if t.length() > 0.1 {
            rig.heading = t.normalize();
        }
    }
    rig.pitch = 0.35;
    rig.dist = 8.0;
    if let Some((p, look)) = dbg_spot {
        if let Some(t) = (look - p).reject_from(p.normalize()).try_normalize() {
            rig.heading = t;
        }
    }

    // sun + sky
    let sun_a0 = landing.z.atan2(landing.x) - 0.75;
    let sc = sys.0.star_color;
    commands.spawn((
        DirectionalLight { color: Color::srgb(sc[0], sc[1], sc[2]), illuminance: 9000.0, shadows_enabled: !mobile(), ..default() },
        CascadeShadowConfigBuilder { num_cascades: 3, maximum_distance: 110.0, first_cascade_far_bound: 14.0, ..default() }.build(),
        Transform::default(),
        SunLight,
        PlanetScoped,
    ));
    ambient.color = Color::WHITE;
    let moon_halo = mats.add(StandardMaterial { base_color: Color::srgb(0.16, 0.2, 0.3), unlit: true, fog_enabled: false, cull_mode: None, alpha_mode: AlphaMode::Add, ..default() });
    commands.spawn((Mesh3d(gfx.dome.clone()), MeshMaterial3d(gfx.sky_dome.clone()), Transform::from_scale(Vec3::splat(2100.0)), NotShadowCaster, NotShadowReceiver, SkyDome, PlanetScoped));
    commands
        .spawn((Transform::default(), Visibility::Visible, SkyRoot, PlanetScoped))
        .with_children(|p| {
            p.spawn((Mesh3d(gfx.star_dome.clone()), MeshMaterial3d(gfx.stars.clone()), Transform::from_scale(Vec3::splat(1900.0)), NotShadowCaster, NotShadowReceiver));
            let sun_local = Vec3::new(1.0, 0.25, 0.0).normalize();
            // airless worlds show a small hard sun, thick air a larger softened one
            let sun_size = 38.0 + 22.0 * def.atmosphere.min(1.2);
            p.spawn((Mesh3d(gfx.sphere.clone()), MeshMaterial3d(gfx.sun.clone()), Transform::from_translation(sun_local * 1700.0).with_scale(Vec3::splat(sun_size)), NotShadowCaster, NotShadowReceiver));
            let face = |d: Vec3| Quat::from_rotation_arc(Vec3::Z, -d);
            p.spawn((Mesh3d(gfx.halo_mesh.clone()), MeshMaterial3d(gfx.halo.clone()), Transform::from_translation(sun_local * 1650.0).with_rotation(face(sun_local)).with_scale(Vec3::splat(sun_size * (4.0 + 5.0 * def.atmosphere.min(1.4)))), NotShadowCaster, NotShadowReceiver));
            // the moon sits opposite the sun, where the night light comes from
            let moon_local = -sun_local;
            p.spawn((Mesh3d(gfx.sphere.clone()), MeshMaterial3d(gfx.moon.clone()), Transform::from_translation(moon_local * 1720.0).with_scale(Vec3::splat(34.0)), NotShadowCaster, NotShadowReceiver, MoonDisc));
            p.spawn((Mesh3d(gfx.halo_mesh.clone()), MeshMaterial3d(moon_halo.clone()), Transform::from_translation(moon_local * 1660.0).with_rotation(face(moon_local)).with_scale(Vec3::splat(150.0)), NotShadowCaster, NotShadowReceiver, MoonDisc));
            let mut rng = Rng::new(sys.0.seed ^ (pi as u64 * 977));
            for (i, other) in sys.0.planets.iter().enumerate() {
                if i == pi {
                    continue;
                }
                let mut d = rng.dir();
                d.y = d.y * 0.5 + 0.2;
                let d = d.normalize();
                let size = 22.0 + other.radius * 0.5 - (i as f32 - pi as f32).abs() * 6.0;
                p.spawn((
                    Mesh3d(meshes.add(meshgen::planet_ball(&TerrainGen::new(other), 28))),
                    MeshMaterial3d(gfx.sky.clone()),
                    Transform::from_translation(d * 1500.0).with_scale(Vec3::splat(size.max(14.0))),
                    NotShadowCaster,
                    NotShadowReceiver,
                ));
            }
        });

    eprintln!("[load] planet {} r={} loaded in {:.2}s ({} chunks, {} meshed)", def.name, def.radius, t_load.elapsed().as_secs_f32(), vw.chunks.len(), streamer.meshed.len());
    commands.insert_resource(PlanetRt { def, landing, sun_a0 });
    commands.insert_resource(vw);
    commands.insert_resource(streamer);
    commands.insert_resource(decor);
    commands.insert_resource(Sky::default());
    cine.0 = land_cine.map(|id| (CineKind::Land, 0.0, id));
    game.player_pos = Some(ppos.to_array());
    ui.reset_play();
    next.set(AppState::Playing);
}

pub fn leave_planet(mut commands: Commands) {
    commands.remove_resource::<crate::terrain::Streamer>();
    commands.remove_resource::<crate::decor::DecorStream>();
    commands.remove_resource::<VoxelWorld>();
    commands.remove_resource::<PlanetRt>();
}

/// Copies the live planet state (terrain edits, player position) into the save structure.
pub fn stash(game: &mut Game, vw: &VoxelWorld, player: Option<Vec3>) {
    let pi = game.planet;
    game.planets[pi].edits = vw.export_edits();
    game.player_pos = player.map(|p| p.to_array());
}

// ---------------------------------------------------------------- per-frame world systems

/// Keeps structure entities in sync with the save data and rebuilds the growing starship.
pub fn sync_structs(
    mut commands: Commands,
    game: Res<Game>,
    gfx: Res<Gfx>,
    q: Query<(Entity, &StructEnt)>,
    mut ships: Query<(Entity, &mut ShipVisual)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let structs = game.structs();
    for (e, s) in &q {
        if !structs.iter().any(|x| x.id == s.0) {
            commands.entity(e).despawn();
        }
    }
    for s in structs {
        if !q.iter().any(|(_, e)| e.0 == s.id) {
            spawn_structure(&mut commands, &gfx, s);
        }
    }
    for (e, mut sv) in &mut ships {
        let Some(s) = structs.iter().find(|x| x.id == sv.0) else { continue };
        let hull = s.delivered_count(Item::HullSection);
        let (core, nav, cryo) = (s.delivered_count(Item::FusionCore) > 0, s.delivered_count(Item::NavComputer) > 0, s.delivered_count(Item::CryoPod) > 0);
        let sig = hull as u64 | (core as u64) << 8 | (nav as u64) << 9 | (cryo as u64) << 10;
        if sig != sv.1 {
            sv.1 = sig;
            match meshgen::starship_mesh(hull, core, nav, cryo) {
                Some(m) => {
                    // the growing ship needs fresh bounds or its new sections get culled
                    commands.entity(e).insert((Mesh3d(meshes.add(m)), MeshMaterial3d(gfx.vc.clone()))).remove::<bevy::render::primitives::Aabb>();
                }
                None => {
                    commands.entity(e).remove::<Mesh3d>();
                }
            }
        }
    }
}


/// Day cycle, sky, sun, moon, fog and ambient. Runs last so the dome follows the camera.
#[allow(clippy::too_many_arguments)]
pub fn update_sky(
    game: Res<Game>,
    sys: Res<Sys>,
    rt: Res<PlanetRt>,
    gfx: Res<Gfx>,
    mut sky: ResMut<Sky>,
    (mut clear, mut ambient): (ResMut<ClearColor>, ResMut<AmbientLight>),
    (mut mats, mut meshes, mut images): (ResMut<Assets<StandardMaterial>>, ResMut<Assets<Mesh>>, ResMut<Assets<Image>>),
    mut root: Query<&mut Transform, (With<SkyRoot>, Without<MainCam>, Without<SunLight>, Without<SkyDome>)>,
    mut dome: Query<&mut Transform, (With<SkyDome>, Without<MainCam>, Without<SunLight>)>,
    mut moon: Query<&mut Visibility, With<MoonDisc>>,
    mut sun: Query<(&mut Transform, &mut DirectionalLight), (With<SunLight>, Without<MainCam>)>,
    mut cam: Query<(&Transform, &mut DistanceFog, Option<&mut EnvironmentMapLight>), With<MainCam>>,
    player: Query<&Transform, (With<Player>, Without<MainCam>, Without<SunLight>, Without<SkyRoot>, Without<SkyDome>)>,
    vw: Res<VoxelWorld>,
    mut last: Local<(f32, u32, u64, Vec<u8>, f32)>,
) {
    let def = &rt.def;
    let a = rt.sun_a0 + (game.time as f32 / def.day_len) * TAU;
    let rot = Quat::from_rotation_y(-a);
    let sun_dir = rot * Vec3::new(1.0, 0.25, 0.0).normalize();
    sky.sun_dir = sun_dir;
    let Ok((cam_t, mut fog, env_light)) = cam.single_mut() else { return };
    if let Ok(mut r) = root.single_mut() {
        r.translation = cam_t.translation;
        r.rotation = rot;
    }
    let up = player.single().map(|t| t.translation.normalize()).unwrap_or(cam_t.translation.normalize());
    let se = up.dot(sun_dir).clamp(-1.0, 1.0);
    let day = crate::sky::Atmos::day(se);
    sky.day = day;
    let atmos = crate::sky::Atmos::new(def, sys.0.star_color);
    // local sky frame: +Y up, the sun in the X-Y plane on the +X side
    let hx = (sun_dir - up * se).normalize_or(any_perp(up));
    let frame = Quat::from_mat3(&Mat3::from_cols(hx, up, hx.cross(up)));
    let sun_local = Vec3::new((1.0 - se * se).max(0.0).sqrt(), se, 0.0);
    if let Ok(mut t) = dome.single_mut() {
        t.translation = cam_t.translation;
        t.rotation = frame;
    }

    // dome colours and the ambient cubemap only change with the sun elevation
    last.1 += 1;
    if (se - last.0).abs() > 0.002 || last.1 > 90 || last.2 != def.seed {
        *last = (se, 0, def.seed, std::mem::take(&mut last.3), last.4);
        if let Some(mesh) = meshes.get_mut(&gfx.dome) {
            let cols: Vec<[f32; 4]> = gfx.dome_dirs.iter().map(|d| {
                let c = atmos.sky(*d, sun_local);
                [c[0], c[1], c[2], 1.0]
            }).collect();
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, cols);
        }
        let mut buf = std::mem::take(&mut last.3);
        last.4 = crate::sky::fill_ambient_cube(&atmos, sun_local, ENV_SIZE, &mut buf);
        if let Some(img) = images.get_mut(&gfx.env) {
            img.data = Some(buf.clone());
        }
        last.3 = buf;
    }
    // below ground the open sky is mostly hidden: dim the sky light in pits and caves
    let ppos = player.single().map(|t| t.translation).unwrap_or(cam_t.translation);
    let under = smoothstep(3.0, 14.0, vw.gen.height(up) - ppos.length());
    if let Some(mut env) = env_light {
        env.rotation = frame;
        env.intensity = 1000.0 * last.4 * (1.0 - 0.65 * under);
    }
    ambient.brightness = 15.0;
    ambient.color = Color::WHITE;

    let (fog_c, scatter) = atmos.fog(sun_local);
    let col = Color::linear_rgb(fog_c[0], fog_c[1], fog_c[2]);
    clear.0 = col;
    fog.color = col;
    fog.directional_light_color = Color::linear_rgb(scatter[0], scatter[1], scatter[2]);
    fog.directional_light_exponent = 6.0 + 24.0 * (1.0 - atmos.air);
    fog.falloff = FogFalloff::from_visibility_squared(atmos.visibility());

    if let Ok((mut t, mut light)) = sun.single_mut() {
        // by day the star lights the scene; at night a dim, bluish moonlight comes from the
        // opposite side so the surface never turns pitch black
        let (sun_c, sun_i) = atmos.sun(se);
        let (moon_c, moon_i) = crate::sky::Atmos::moon(se);
        if sun_i >= moon_i {
            *t = Transform::default().looking_to(-sun_dir, Vec3::Y);
            light.illuminance = sun_i + 30.0;
            light.color = Color::linear_rgb(sun_c[0], sun_c[1], sun_c[2]);
        } else {
            *t = Transform::default().looking_to(sun_dir, Vec3::Y);
            light.illuminance = moon_i;
            light.color = Color::linear_rgb(moon_c[0], moon_c[1], moon_c[2]);
        }
    }
    // sun disc and halo: brighter than white on HDR so they bloom; reddened when low
    let (sun_c, _) = atmos.sun(se.max(0.0));
    let hot = if mobile() { 1.0 } else { 5.0 };
    let vis = smoothstep(-0.12, 0.02, se);
    if let Some(m) = mats.get_mut(&gfx.sun) {
        m.base_color = Color::linear_rgb(sun_c[0] * hot + 0.1, sun_c[1] * hot + 0.1, sun_c[2] * hot + 0.1);
    }
    if let Some(m) = mats.get_mut(&gfx.halo) {
        let k = (0.25 + 0.55 * atmos.air) * vis;
        m.base_color = Color::linear_rgb(sun_c[0] * k, sun_c[1] * k, sun_c[2] * k);
    }
    let show_moon = if day < 0.8 { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in &mut moon {
        v.set_if_neq(show_moon);
    }
    if let Some(m) = mats.get_mut(&gfx.moon) {
        let k = 1.0 - day * 0.8;
        m.base_color = Color::srgb(0.82 * k, 0.86 * k, 0.95 * k);
    }
    if let Some(m) = mats.get_mut(&gfx.stars) {
        let alpha = (1.0 - day * (atmos.air * 1.3).min(1.0)).clamp(0.0, 1.0).powi(2);
        if (m.base_color.alpha() - alpha).abs() > 0.01 {
            m.base_color = Color::srgba(1.0, 1.0, 1.0, alpha);
        }
    }
}
