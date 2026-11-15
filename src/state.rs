//! The whole persistent game state + save / load.
use crate::galaxy::StarSystem;
use crate::items::{Inventory, Item, Lang, Tech, SHIP_PARTS, SK};
use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Bump whenever planets or the save layout change: a save made for other terrain would put
/// the player and the base inside the rock of the new planet. 5: streamed 220-380 m planets.
/// 6: layered caves, new decor ids, story state, hazards and blackout bundles.
pub const SAVE_VERSION: u32 = 6;

#[derive(Clone, Serialize, Deserialize)]
pub struct Job {
    pub recipe: u16,
    pub count: u32,
    pub progress: f32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Structure {
    pub id: u32,
    pub kind: SK,
    pub pos: [f32; 3],
    pub rot: [f32; 4],
    pub job: Option<Job>,
    pub out: Vec<(Item, u32)>,
    pub store: Inventory,
    pub fuel: f32,
    pub charge: f32,
    pub delivered: Vec<(Item, u32)>,
    /// ids of structures this one is cabled / tethered to (kept symmetric)
    pub links: Vec<u32>,
}

impl Structure {
    pub fn new(id: u32, kind: SK, pos: [f32; 3], rot: [f32; 4]) -> Self {
        Structure {
            id,
            kind,
            pos,
            rot,
            job: None,
            out: Vec::new(),
            store: Inventory::new(if kind == SK::Storage { 24 } else { 0 }),
            fuel: 0.0,
            charge: 0.0,
            delivered: Vec::new(),
            links: Vec::new(),
        }
    }
    pub fn delivered_count(&self, item: Item) -> u32 {
        self.delivered.iter().find(|d| d.0 == item).map(|d| d.1).unwrap_or(0)
    }
    pub fn ship_complete(&self) -> bool {
        SHIP_PARTS.iter().all(|&(i, n)| self.delivered_count(i) >= n)
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct PlanetSave {
    pub visited: bool,
    pub edits: Vec<([i32; 3], Vec<u8>)>,
    pub removed: Vec<u32>,
    pub structs: Vec<Structure>,
    pub landing: Option<[f32; 3]>,
    /// stable ids of hazards dug out on this planet (see `hazards`)
    pub hazards_cleared: Vec<u32>,
}

/// Backpack contents left behind where the player blacked out, until picked up again.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Bundle {
    pub id: u32,
    pub planet: usize,
    pub pos: [f32; 3],
    pub items: Vec<(Item, u32)>,
}

#[derive(Resource, Clone, Serialize, Deserialize)]
pub struct Game {
    pub version: u32,
    pub seed: u64,
    pub system_index: u32,
    pub lang: Lang,
    pub planet: usize,
    pub inv: Inventory,
    pub soil: f32,
    pub health: f32,
    pub oxygen: f32,
    pub jet: f32,
    pub player_pos: Option<[f32; 3]>,
    pub data: u32,
    pub techs: Vec<Tech>,
    pub discovered: Vec<Item>,
    pub species_seen: Vec<String>,
    pub collected: Vec<(Item, u32)>,
    pub built: Vec<SK>,
    pub researched: u32,
    pub quest: usize,
    pub journal: Vec<u16>,
    pub journal_unread: bool,
    pub fragments: u32,
    pub planets: Vec<PlanetSave>,
    pub time: f64,
    pub travels: u32,
    pub next_id: u32,
    pub finished: bool,
    pub ore_frac: Vec<(Item, f32)>,
    pub carried_shuttle: Option<Structure>,
    pub deaths: u32,
    pub seen_night: bool,
    /// story memory: chapter flags, wrecks seen, remarks said (see `story::StoryState`)
    pub story: crate::story::StoryState,
    /// one bundle per blackout, on any planet, kept until recovered
    pub bundles: Vec<Bundle>,
    /// hazard kinds MIRA has already warned about
    pub hazards_seen: Vec<crate::hazards::HazKind>,
}

impl Game {
    pub fn new(seed: u64, lang: Lang) -> Game {
        let sys = StarSystem::generate(seed, 0);
        Game {
            version: SAVE_VERSION,
            seed,
            system_index: 0,
            lang,
            planet: sys.home,
            inv: Inventory::new(16),
            soil: 10.0,
            health: 100.0,
            oxygen: 100.0,
            jet: 100.0,
            player_pos: None,
            data: 0,
            techs: Vec::new(),
            discovered: Vec::new(),
            species_seen: Vec::new(),
            collected: Vec::new(),
            built: Vec::new(),
            researched: 0,
            quest: 0,
            journal: Vec::new(),
            journal_unread: false,
            fragments: 0,
            planets: vec![PlanetSave::default(); sys.planets.len()],
            time: 0.0,
            travels: 0,
            next_id: 1,
            finished: false,
            ore_frac: Vec::new(),
            carried_shuttle: None,
            deaths: 0,
            seen_night: false,
            story: Default::default(),
            bundles: Vec::new(),
            hazards_seen: Vec::new(),
        }
    }

    pub fn has_tech(&self, t: Tech) -> bool {
        self.techs.contains(&t)
    }
    pub fn tech_ok(&self, t: Option<Tech>) -> bool {
        t.map(|t| self.has_tech(t)).unwrap_or(true)
    }
    pub fn collected_count(&self, item: Item) -> u32 {
        self.collected.iter().find(|c| c.0 == item).map(|c| c.1).unwrap_or(0)
    }
    pub fn structs(&self) -> &Vec<Structure> {
        &self.planets[self.planet].structs
    }
    pub fn structs_mut(&mut self) -> &mut Vec<Structure> {
        let p = self.planet;
        &mut self.planets[p].structs
    }
    pub fn find_struct(&mut self, id: u32) -> Option<&mut Structure> {
        self.structs_mut().iter_mut().find(|s| s.id == id)
    }
    pub fn max_soil(&self) -> f32 {
        120.0
    }

    /// Start over in the next star system, keeping knowledge and a few supplies.
    pub fn advance_system(&mut self) {
        self.system_index += 1;
        let sys = StarSystem::generate(self.seed, self.system_index);
        self.planet = sys.home;
        self.planets = vec![PlanetSave::default(); sys.planets.len()];
        self.player_pos = None;
        self.finished = false;
        self.carried_shuttle = None;
        self.bundles.clear();
        self.quest = 0;
        self.travels = 0;
        self.built.clear();
        self.collected.clear();
        self.researched = 0;
        self.health = 100.0;
        self.oxygen = 100.0;
    }
}

pub fn save_dir() -> PathBuf {
    #[cfg(target_os = "android")]
    {
        if let Some(app) = bevy::window::ANDROID_APP.get() {
            if let Some(p) = app.internal_data_path() {
                return p;
            }
        }
        return PathBuf::from("/data/local/tmp");
    }
    #[cfg(not(target_os = "android"))]
    {
        if let Ok(p) = std::env::var("LOSTRANOUT_HOME") {
            return PathBuf::from(p);
        }
        #[cfg(target_os = "windows")]
        let base = std::env::var("APPDATA").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."));
        #[cfg(not(target_os = "windows"))]
        let base = std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .unwrap_or_else(|_| PathBuf::from("."));
        base.join("lostranout")
    }
}

fn save_path() -> PathBuf {
    save_dir().join("save.bin")
}

/// True only for a save this version can load (the version is the first field, a little-endian
/// u32), so "Continue" is never offered for an outdated save.
pub fn save_exists() -> bool {
    use std::io::Read;
    let mut head = [0u8; 4];
    std::fs::File::open(save_path()).and_then(|mut f| f.read_exact(&mut head)).is_ok() && u32::from_le_bytes(head) == SAVE_VERSION
}

pub fn save_game(game: &Game) -> Result<(), String> {
    let dir = save_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let bytes = bincode::serialize(game).map_err(|e| e.to_string())?;
    let tmp = dir.join("save.tmp");
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, save_path()).map_err(|e| e.to_string())
}

pub fn load_game() -> Option<Game> {
    let bytes = std::fs::read(save_path()).ok()?;
    let game: Game = bincode::deserialize(&bytes).ok()?;
    (game.version == SAVE_VERSION).then_some(game)
}

#[derive(Serialize, Deserialize)]
struct Prefs {
    lang: Lang,
}

pub fn load_lang() -> Lang {
    if let Ok(b) = std::fs::read(save_dir().join("prefs.bin")) {
        if let Ok(p) = bincode::deserialize::<Prefs>(&b) {
            return p.lang;
        }
    }
    match std::env::var("LANG") {
        Ok(l) if l.starts_with("en") => Lang::En,
        _ => Lang::Tr,
    }
}

pub fn save_lang(lang: Lang) {
    let dir = save_dir();
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(b) = bincode::serialize(&Prefs { lang }) {
        let _ = std::fs::write(dir.join("prefs.bin"), b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continue_is_offered_only_for_current_saves() {
        let dir = std::env::temp_dir().join(format!("lostranout-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("LOSTRANOUT_HOME", &dir);
        let _ = std::fs::remove_file(save_path());
        assert!(!save_exists());
        let mut g = Game::new(42, Lang::Tr);
        save_game(&g).unwrap();
        assert!(save_exists());
        assert!(load_game().is_some());
        g.version = SAVE_VERSION - 1;
        save_game(&g).unwrap();
        assert!(!save_exists(), "an outdated save must not enable Continue");
        assert!(load_game().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
