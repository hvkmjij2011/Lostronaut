//! Lostranout: crash on a procedural voxel planet, rebuild, and reach the stars again.
pub mod debug;
pub mod decor;
pub mod galaxy;
pub mod gameplay;
pub mod hazards;
pub mod items;
pub mod meshgen;
pub mod models;
pub mod network;
pub mod names;
pub mod nature;
pub mod player;
pub mod props;
pub mod sky;
pub mod state;
pub mod story;
pub mod terrain;
pub mod ui;
pub mod util;
pub mod voxel;
pub mod world;

use bevy::prelude::*;
use bevy_egui::{EguiContextPass, EguiPlugin};
use items::{Item, Lang, SK};

#[derive(States, Default, Clone, Copy, Eq, PartialEq, Debug, Hash)]
pub enum AppState {
    #[default]
    Menu,
    Intro,
    Loading,
    Playing,
    Ending,
}

#[derive(Component)]
pub struct MainCam;

#[derive(Resource)]
pub struct Sys(pub galaxy::StarSystem);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CineKind {
    Launch,
    Land,
    Finale,
}

/// Active cinematic: (kind, elapsed seconds, structure id)
#[derive(Resource, Default)]
pub struct Cine(pub Option<(CineKind, f32, u32)>);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToolMode {
    Dig,
    Fill,
    Flatten,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Inventory,
    Build,
    Research,
    Map,
    Journal,
}

pub enum Action {
    Travel(usize),
    Finale(u32),
    Dismantle(u32),
    Save,
    ToMenu,
}

#[derive(Resource)]
pub struct Ui {
    pub lang: Lang,
    pub open: bool,
    pub tab: Tab,
    pub machine: Option<u32>,
    pub paused: bool,
    pub toasts: Vec<(String, f32, [f32; 3])>,
    pub pickups: Vec<(Item, u32, f32)>,
    pub placing: Option<SK>,
    pub place_confirm: bool,
    pub place_problem: Option<String>,
    pub tool_mode: ToolMode,
    pub brush: f32,
    pub max_brush: f32,
    pub tool_armed: bool,
    pub v_jump: bool,
    pub v_jump_prev: bool,
    pub v_interact: bool,
    pub v_zoom: f32,
    pub travel_from: Option<u32>,
    pub map_sel: Option<usize>,
    pub journal_sel: Option<u16>,
    pub inv_sel: Option<usize>,
    pub intro_page: usize,
    pub actions: Vec<Action>,
    pub hover_text: Option<String>,
    pub full_cd: f32,
    pub in_oxygen: bool,
    pub confirm_new: bool,
    pub shot: Option<String>,
    /// synthetic cursor with the left button held (automated tests only)
    pub dbg_mouse: Option<Vec2>,
}

impl Default for Ui {
    fn default() -> Self {
        Ui {
            lang: Lang::Tr,
            open: false,
            tab: Tab::Inventory,
            machine: None,
            paused: false,
            toasts: Vec::new(),
            pickups: Vec::new(),
            placing: None,
            place_confirm: false,
            place_problem: None,
            tool_mode: ToolMode::Dig,
            brush: 2.0,
            max_brush: 2.5,
            tool_armed: false,
            v_jump: false,
            v_jump_prev: false,
            v_interact: false,
            v_zoom: 0.0,
            travel_from: None,
            map_sel: None,
            journal_sel: None,
            inv_sel: None,
            intro_page: 0,
            actions: Vec::new(),
            hover_text: None,
            full_cd: 0.0,
            in_oxygen: true,
            confirm_new: false,
            shot: None,
            dbg_mouse: None,
        }
    }
}

impl Ui {
    pub fn toast(&mut self, text: String, color: [f32; 3]) {
        if self.toasts.len() > 5 {
            self.toasts.remove(0);
        }
        self.toasts.push((text, 5.0, color));
    }
    pub fn journal_toast(&mut self, l: Lang) {
        self.toast(
            match l {
                Lang::Tr => "MİRA: Yeni günlük kaydı (J)".to_string(),
                Lang::En => "MIRA: New journal entry (J)".to_string(),
            },
            [1.0, 0.8, 0.5],
        );
    }
    pub fn pickup(&mut self, item: Item, n: u32) {
        if let Some(p) = self.pickups.iter_mut().find(|p| p.0 == item) {
            p.1 += n;
            p.2 = 3.0;
        } else {
            self.pickups.push((item, n, 3.0));
        }
    }
    pub fn reset_play(&mut self) {
        self.open = false;
        self.machine = None;
        self.paused = false;
        self.placing = None;
        self.travel_from = None;
        self.place_confirm = false;
        self.hover_text = None;
    }
}

fn start_from_env(mut commands: Commands, mut next: ResMut<NextState<AppState>>, mut ui: ResMut<Ui>) {
    ui.lang = state::load_lang();
    // Developer helpers: LOSTRANOUT_AUTOSTART=<seed> jumps straight into a new game,
    // LOSTRANOUT_SHOT=<file.png> saves a screenshot after a few seconds and quits.
    if let Ok(seed) = std::env::var("LOSTRANOUT_AUTOSTART") {
        let seed = seed.parse().unwrap_or(1);
        let mut game = state::Game::new(seed, ui.lang);
        if let Some(t) = std::env::var("LOSTRANOUT_TIME").ok().and_then(|s| s.parse::<f64>().ok()) {
            game.time = t;
            game.seen_night = true;
        }
        if std::env::var("LOSTRANOUT_CHEAT").is_ok() {
            for item in Item::ALL {
                game.inv.slots.push(Some((item, 40)));
            }
            game.data = 9000;
        }
        // LOSTRANOUT_SYSTEM=<n>: start in the n-th star system; LOSTRANOUT_INTRO=1: show its intro first
        for _ in 0..std::env::var("LOSTRANOUT_SYSTEM").ok().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0) {
            game.advance_system();
        }
        // LOSTRANOUT_PLANET=<index or kind>, LOSTRANOUT_HOUR=<0..24>
        let sys = galaxy::StarSystem::generate(game.seed, game.system_index);
        debug::start_planet(&mut game, &sys);
        commands.insert_resource(Sys(sys));
        commands.insert_resource(game);
        next.set(if std::env::var("LOSTRANOUT_INTRO").is_ok() { AppState::Intro } else { AppState::Loading });
    }
    if std::env::var("LOSTRANOUT_CONTINUE").is_ok() {
        if let Some(game) = state::load_game() {
            commands.insert_resource(Sys(galaxy::StarSystem::generate(game.seed, game.system_index)));
            commands.insert_resource(game);
            next.set(AppState::Loading);
        }
    }
    ui.shot = std::env::var("LOSTRANOUT_SHOT").ok();
}

fn auto_shot(mut commands: Commands, time: Res<Time>, mut ui: ResMut<Ui>, mut done: Local<u8>, mut opened: Local<bool>, mut exit: EventWriter<AppExit>) {
    use bevy::render::view::screenshot::{save_to_disk, Screenshot};
    if !*opened && time.elapsed_secs() > 4.0 {
        *opened = true;
        match std::env::var("LOSTRANOUT_UI").as_deref() {
            Ok("inv") => (ui.open, ui.tab) = (true, Tab::Inventory),
            Ok("build") => (ui.open, ui.tab) = (true, Tab::Build),
            Ok("research") => (ui.open, ui.tab) = (true, Tab::Research),
            Ok("map") => (ui.open, ui.tab) = (true, Tab::Map),
            Ok("journal") => (ui.open, ui.tab) = (true, Tab::Journal),
            Ok("pause") => ui.paused = true,
            Ok(m) if m.starts_with("machine") => ui.machine = m[7..].parse().ok(),
            _ => {}
        }
    }
    let Some(path) = &ui.shot else { return };
    let delay: f32 = std::env::var("LOSTRANOUT_SHOT_DELAY").ok().and_then(|s| s.parse().ok()).unwrap_or(6.0);
    let t = time.elapsed_secs();
    if *done == 0 && t > delay {
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path.clone()));
        *done = 1;
    } else if *done == 1 && t > delay + 1.5 {
        exit.write(AppExit::Success);
    }
}

pub fn run() {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    // automated test runs must not steal keyboard focus from the desktop
                    focused: std::env::var("LOSTRANOUT_AUTOSTART").is_err() && std::env::var("LOSTRANOUT_CONTINUE").is_err(),
                    title: std::env::var("LOSTRANOUT_TITLE").unwrap_or_else(|_| "Lostranout".into()),
                    // LOSTRANOUT_SIZE=832x384 opens a phone-sized window (logical px) for UI checks
                    resolution: std::env::var("LOSTRANOUT_SIZE").ok().and_then(|v| v.split_once('x').and_then(|(w, h)| Some((w.parse::<f32>().ok()?, h.parse::<f32>().ok()?)))).unwrap_or((1280.0, 720.0)).into(),
                    ..default()
                }),
                ..default()
            })
            .set(bevy::log::LogPlugin { level: bevy::log::Level::WARN, ..default() }),
    )
    .add_plugins(EguiPlugin { enable_multipass_for_primary_context: true })
    .init_state::<AppState>()
    .insert_resource(ClearColor(Color::BLACK))
    .insert_resource(AmbientLight { brightness: 200.0, ..default() })
    .init_resource::<Ui>()
    .init_resource::<Cine>()
    .init_resource::<player::Controls>()
    .init_resource::<player::CamRig>()
    .init_resource::<gameplay::PowerStat>()
    .init_resource::<network::Grid>()
    .init_resource::<network::hud::NetUi>()
    .init_resource::<gameplay::PendingTravel>()
    .init_resource::<gameplay::Hover>()
    .add_systems(Startup, (world::setup, network::vis::setup, start_from_env))
    .add_systems(Update, world::enter_menu.run_if(in_state(AppState::Menu)))
    .add_systems(OnExit(AppState::Menu), world::despawn_scoped::<world::MenuScoped>)
    .add_systems(OnExit(AppState::Playing), (world::despawn_scoped::<world::PlanetScoped>, world::leave_planet))
    .add_systems(Update, (world::spin, auto_shot, debug::click_bot))
    .add_systems(Update, world::load_planet.run_if(in_state(AppState::Loading)))
    .add_systems(Update, gameplay::save_on_suspend.run_if(in_state(AppState::Playing)))
    .add_systems(Update, debug::ore_showcase.run_if(in_state(AppState::Playing)))
    .add_systems(
        Update,
        (
            debug::mouse_bot,
            player::gather_input,
            debug::bot,
            gameplay::apply_actions,
            player::player_move,
            gameplay::interact,
            gameplay::drop_tether,
            gameplay::tool_use,
            gameplay::placement,
            gameplay::simulate,
            gameplay::cine_update,
            world::sync_structs,
            models::animate_parts,
            crate::terrain::stream,
            crate::decor::stream_decor,
            network::vis::sync_wires,
            network::vis::backpack_line,
            player::camera_follow,
            world::update_sky,
        )
            .chain()
            .run_if(in_state(AppState::Playing)),
    )
    // hazards: placement / effects between movement and the oxygen simulation, blackout right
    // after it, HUD on top of the main HUD
    .init_resource::<hazards::HazFx>()
    .add_systems(Startup, hazards::setup)
    .add_systems(OnExit(AppState::Playing), hazards::reset)
    .add_systems(Update, (hazards::debug_bot, hazards::slow_input).chain().after(debug::bot).before(player::player_move).run_if(in_state(AppState::Playing)))
    .add_systems(Update, (hazards::stream_hazards, hazards::update_hazards, hazards::storm).chain().after(gameplay::tool_use).before(gameplay::simulate).run_if(in_state(AppState::Playing)))
    .add_systems(Update, (gameplay::blackout, hazards::bundles).chain().after(gameplay::simulate).before(gameplay::cine_update).run_if(in_state(AppState::Playing)))
    .add_systems(EguiContextPass, hazards::hud.after(ui::play_ui).run_if(in_state(AppState::Playing)))
    .add_systems(EguiContextPass, (ui::style, ui::menu_ui.run_if(in_state(AppState::Menu)), ui::intro_ui.run_if(in_state(AppState::Intro)), ui::loading_ui.run_if(in_state(AppState::Loading)), ui::ending_ui.run_if(in_state(AppState::Ending)), ui::play_ui.run_if(in_state(AppState::Playing))).chain());
    app.run();
}

#[cfg(target_os = "android")]
#[bevy_main]
fn main() {
    run();
}
