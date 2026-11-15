//! Procedural star systems: star, planets, their resources, palettes and flora.
//! Everything derives from (seed, system index) so nothing here needs to be saved.
use crate::items::{Item, L2};
use crate::names;
use crate::util::{mix64, Rng};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlanetKind {
    Temperate,
    Desert,
    Ice,
    Volcanic,
    Barren,
    Toxic,
    Crystal,
}

impl PlanetKind {
    pub fn name(self) -> L2 {
        match self {
            PlanetKind::Temperate => ("Ilıman Dünya", "Temperate World"),
            PlanetKind::Desert => ("Çöl Gezegeni", "Desert Planet"),
            PlanetKind::Ice => ("Buz Gezegeni", "Ice Planet"),
            PlanetKind::Volcanic => ("Volkanik Gezegen", "Volcanic Planet"),
            PlanetKind::Barren => ("Çorak Kaya", "Barren Rock"),
            PlanetKind::Toxic => ("Zehirli Bataklık", "Toxic Marsh"),
            PlanetKind::Crystal => ("Kristal Gezegen", "Crystal Planet"),
        }
    }
    /// How this kind of world is hollowed out (see `voxel::TerrainGen::caves`).
    pub fn cave_style(self) -> CaveStyle {
        use PlanetKind::*;
        let (tunnel, chambers, size, shafts, mouths, rough) = match self {
            // balanced: winding tunnels, a fair number of halls, a few ways in
            Temperate => (1.0, 1.0, 1.0, 1.0, 1.0, 1.0),
            // wind-cut: wide, low passages, many openings, few halls
            Desert => (1.2, 0.6, 0.8, 0.8, 1.5, 0.8),
            // ice caverns: few tunnels, very large halls
            Ice => (0.85, 1.5, 1.35, 0.8, 0.8, 0.6),
            // lava tubes: broad tubes and many vertical vents
            Volcanic => (1.45, 0.7, 0.9, 1.6, 1.0, 1.3),
            // dead rock: sparse, narrow, rough
            Barren => (0.8, 0.7, 0.85, 0.7, 0.7, 1.5),
            // eaten-out marsh rock: many medium halls
            Toxic => (1.0, 1.4, 0.9, 1.0, 1.2, 1.1),
            // geodes: tall round halls
            Crystal => (0.9, 1.2, 1.4, 1.1, 0.9, 0.7),
        };
        CaveStyle { tunnel, chambers, size, shafts, mouths, rough }
    }
    fn heat(self) -> f32 {
        match self {
            PlanetKind::Volcanic => 0.0,
            PlanetKind::Desert => 1.0,
            PlanetKind::Toxic => 2.0,
            PlanetKind::Temperate => 3.0,
            PlanetKind::Crystal => 4.0,
            PlanetKind::Barren => 5.0,
            PlanetKind::Ice => 6.0,
        }
    }
}

/// Per planet-kind cave layout multipliers (1.0 = the temperate default).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CaveStyle {
    /// tunnel width
    pub tunnel: f32,
    /// how much of a layer is open halls
    pub chambers: f32,
    /// hall height
    pub size: f32,
    /// how many tunnel junctions get a shaft to the deep layer
    pub shafts: f32,
    /// how many surface entrances (ramps, sinkholes)
    pub mouths: f32,
    /// wall roughness
    pub rough: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Arch {
    Tree,
    Pine,
    Bush,
    Fern,
    Cactus,
    Mushroom,
    Bulb,
    Coral,
}

impl Arch {
    pub fn word(self) -> L2 {
        match self {
            Arch::Tree => ("ağacı", "tree"),
            Arch::Pine => ("çamı", "pine"),
            Arch::Bush => ("çalısı", "bush"),
            Arch::Fern => ("eğreltisi", "fern"),
            Arch::Cactus => ("kaktüsü", "cactus"),
            Arch::Mushroom => ("mantarı", "shroom"),
            Arch::Bulb => ("feneri", "lantern"),
            Arch::Coral => ("mercanı", "coral"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Species {
    pub name: String,
    pub arch: Arch,
    pub leaf: [f32; 3],
    pub stem: [f32; 3],
    pub size: f32,
    pub organic: u32,
    pub seed: u64,
}

#[derive(Clone, Debug)]
pub struct PlanetDef {
    pub seed: u64,
    pub name: String,
    pub kind: PlanetKind,
    pub radius: f32,
    pub gravity: f32,
    pub day_len: f32,
    pub orbit: f32,
    pub phase: f32,
    pub resources: Vec<(Item, f32)>,
    pub top: [f32; 3],
    pub soil: [f32; 3],
    pub rock: [f32; 3],
    pub deep: [f32; 3],
    pub sky: [f32; 3],
    pub atmosphere: f32,
    pub amp: f32,
    pub mount: f32,
    pub terrace: f32,
    pub craters: f32,
    pub cave: f32,
    pub flora: Vec<Species>,
    pub flora_density: f32,
}

#[derive(Clone, Debug)]
pub struct StarSystem {
    pub seed: u64,
    pub index: u32,
    pub name: String,
    pub class: L2,
    pub star_color: [f32; 3],
    pub planets: Vec<PlanetDef>,
    pub home: usize,
}

fn jitter(rng: &mut Rng, c: [f32; 3], amt: f32) -> [f32; 3] {
    [
        (c[0] * rng.range(1.0 - amt, 1.0 + amt)).clamp(0.0, 1.0),
        (c[1] * rng.range(1.0 - amt, 1.0 + amt)).clamp(0.0, 1.0),
        (c[2] * rng.range(1.0 - amt, 1.0 + amt)).clamp(0.0, 1.0),
    ]
}

fn make_flora(rng: &mut Rng, kind: PlanetKind) -> (Vec<Species>, f32) {
    use Arch::*;
    let (pool, count, density): (&[Arch], usize, f32) = match kind {
        PlanetKind::Temperate => (&[Tree, Pine, Bush, Fern, Mushroom, Bulb, Tree, Bush], 6, 1.0),
        PlanetKind::Toxic => (&[Mushroom, Bulb, Coral, Fern, Bush, Mushroom], 5, 0.8),
        PlanetKind::Desert => (&[Cactus, Bush, Coral, Cactus], 3, 0.3),
        PlanetKind::Crystal => (&[Coral, Bulb, Mushroom], 3, 0.35),
        PlanetKind::Ice => (&[Pine, Coral, Bulb], 2, 0.2),
        PlanetKind::Volcanic => (&[Coral, Cactus], 1, 0.1),
        PlanetKind::Barren => (&[], 0, 0.0),
    };
    let leaf_base: [f32; 3] = match kind {
        PlanetKind::Temperate => [0.25, 0.6, 0.22],
        PlanetKind::Toxic => [0.65, 0.3, 0.7],
        PlanetKind::Desert => [0.45, 0.6, 0.3],
        PlanetKind::Crystal => [0.4, 0.8, 0.9],
        PlanetKind::Ice => [0.5, 0.75, 0.8],
        _ => [0.8, 0.4, 0.2],
    };
    let mut out = Vec::new();
    let mut archs: Vec<Arch> = pool.to_vec();
    rng.shuffle(&mut archs);
    for i in 0..count.min(archs.len()) {
        let arch = archs[i];
        let mut leaf = jitter(rng, leaf_base, 0.35);
        if rng.chance(0.25) {
            leaf = [leaf[2], leaf[0], leaf[1]]; // alien tint
        }
        let size = match arch {
            Tree | Pine => rng.range(1.0, 1.6),
            Cactus | Coral => rng.range(0.7, 1.2),
            _ => rng.range(0.6, 1.0),
        };
        out.push(Species {
            name: names::species_name(rng),
            arch,
            leaf,
            stem: jitter(rng, [0.4, 0.28, 0.18], 0.3),
            size,
            organic: match arch {
                Tree | Pine => 3,
                Cactus | Coral | Mushroom => 2,
                _ => 1,
            },
            seed: rng.next_u64(),
        });
    }
    (out, density)
}

fn make_planet(rng: &mut Rng, kind: PlanetKind, name: String, home: bool) -> PlanetDef {
    use PlanetKind::*;
    // top, soil, rock, deep, sky, atmosphere, amp, mount, terrace, craters, cave
    let (top, soil, rock, deep, sky, atmosphere, amp, mount, terrace, craters, cave): (
        [f32; 3], [f32; 3], [f32; 3], [f32; 3], [f32; 3], f32, f32, f32, f32, f32, f32,
    ) = match kind {
        Temperate => ([0.34, 0.5, 0.25], [0.45, 0.33, 0.2], [0.5, 0.5, 0.52], [0.3, 0.28, 0.33], [0.42, 0.68, 0.95], 1.0, 8.0, 0.5, 0.0, 0.0, 0.016),
        Desert => ([0.86, 0.68, 0.4], [0.75, 0.5, 0.3], [0.6, 0.4, 0.3], [0.4, 0.26, 0.22], [0.92, 0.74, 0.5], 0.8, 6.5, 0.35, 0.7, 0.0, 0.012),
        Ice => ([0.9, 0.95, 1.0], [0.7, 0.8, 0.92], [0.45, 0.55, 0.72], [0.25, 0.32, 0.5], [0.68, 0.85, 1.0], 0.7, 7.0, 0.7, 0.0, 0.0, 0.014),
        Volcanic => ([0.2, 0.18, 0.18], [0.32, 0.15, 0.1], [0.15, 0.13, 0.14], [0.55, 0.16, 0.05], [0.42, 0.18, 0.1], 0.9, 11.0, 1.0, 0.0, 0.0, 0.02),
        Barren => ([0.6, 0.6, 0.62], [0.5, 0.5, 0.52], [0.4, 0.4, 0.43], [0.25, 0.25, 0.28], [0.02, 0.02, 0.04], 0.05, 4.5, 0.2, 0.0, 1.0, 0.01),
        Toxic => ([0.5, 0.6, 0.15], [0.35, 0.3, 0.4], [0.3, 0.36, 0.3], [0.2, 0.22, 0.25], [0.6, 0.7, 0.28], 1.4, 5.0, 0.2, 0.0, 0.0, 0.02),
        Crystal => ([0.56, 0.36, 0.72], [0.32, 0.2, 0.46], [0.25, 0.25, 0.42], [0.15, 0.12, 0.3], [0.42, 0.26, 0.62], 0.8, 9.0, 0.8, 0.3, 0.0, 0.016),
    };
    let (flora, flora_density) = make_flora(rng, kind);
    let radius = if home { rng.range(260.0, 300.0) } else { rng.range(220.0, 380.0) };
    let gravity = match kind {
        Barren => rng.range(4.0, 6.5),
        _ => rng.range(8.0, 12.0) * (0.85 + radius / 2000.0),
    };
    PlanetDef {
        seed: rng.next_u64(),
        name,
        kind,
        radius: radius.round(),
        gravity,
        day_len: rng.range(300.0, 560.0),
        orbit: 0.0,
        phase: rng.range(0.0, std::f32::consts::TAU),
        resources: Vec::new(),
        top: jitter(rng, top, 0.14),
        soil: jitter(rng, soil, 0.12),
        rock: jitter(rng, rock, 0.1),
        deep: jitter(rng, deep, 0.1),
        sky: jitter(rng, sky, 0.1),
        atmosphere,
        amp: amp * rng.range(0.85, 1.15),
        mount,
        terrace,
        craters,
        cave,
        flora,
        flora_density,
    }
}

impl StarSystem {
    pub fn generate(seed: u64, index: u32) -> StarSystem {
        use PlanetKind::*;
        let mut rng = Rng::new(mix64(seed, index as u64 + 1));
        let name = names::star_name(&mut rng);
        let classes: [(L2, [f32; 3]); 5] = [
            (("Sarı cüce", "Yellow dwarf"), [1.0, 0.95, 0.8]),
            (("Turuncu cüce", "Orange dwarf"), [1.0, 0.8, 0.6]),
            (("Kırmızı cüce", "Red dwarf"), [1.0, 0.65, 0.5]),
            (("Mavi-beyaz yıldız", "Blue-white star"), [0.8, 0.88, 1.0]),
            (("Beyaz yıldız", "White star"), [1.0, 1.0, 0.98]),
        ];
        let (class, star_color) = *rng.pick(&classes);

        let n = 4 + rng.int(3) as usize;
        let mut others = vec![Desert, Ice, Volcanic, Barren, Toxic, Crystal];
        rng.shuffle(&mut others);
        others.truncate(n - 1);
        let mut kinds: Vec<(PlanetKind, f32)> = others.iter().map(|&k| (k, k.heat() + rng.range(-1.2, 1.2))).collect();
        kinds.push((Temperate, Temperate.heat()));
        kinds.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        let mut taken: Vec<String> = vec![name.clone()];
        let mut planets = Vec::new();
        let mut home = 0;
        for (i, (kind, _)) in kinds.iter().enumerate() {
            let pname = names::planet_name(&mut rng, &taken);
            taken.push(pname.clone());
            let is_home = *kind == Temperate;
            if is_home {
                home = i;
            }
            let mut p = make_planet(&mut rng, *kind, pname, is_home);
            p.orbit = 1.0 + i as f32 * 0.85 + rng.range(-0.15, 0.15);
            p.resources = match kind {
                Temperate => vec![(Item::IronOre, 0.8), (Item::CopperOre, 0.7), (Item::Quartz, 0.7), (Item::Carbon, 0.6), (Item::Ice, 0.4)],
                Desert => vec![(Item::Quartz, 1.0), (Item::CopperOre, 0.6), (Item::IronOre, 0.5)],
                Ice => vec![(Item::Ice, 1.0), (Item::Quartz, 0.4), (Item::Carbon, 0.4)],
                Volcanic => vec![(Item::IronOre, 1.0), (Item::Carbon, 0.8), (Item::CopperOre, 0.4)],
                Barren => vec![(Item::IronOre, 0.6), (Item::Quartz, 0.5), (Item::Ice, 0.3)],
                Toxic => vec![(Item::Carbon, 0.9), (Item::CopperOre, 0.7), (Item::Ice, 0.3)],
                Crystal => vec![(Item::Quartz, 1.0), (Item::CopperOre, 0.4)],
            };
            planets.push(p);
        }

        // Special resources: each one is guaranteed to exist somewhere away from home,
        // on a planet whose type fits, spread as evenly as possible.
        let specials: [(Item, &[PlanetKind]); 4] = [
            (Item::Heliosite, &[Crystal, Ice, Barren]),
            (Item::TitaniumOre, &[Volcanic, Barren, Desert]),
            (Item::Lithium, &[Desert, Toxic, Ice]),
            (Item::Uranite, &[Barren, Volcanic, Ice, Toxic]),
        ];
        let mut special_count = vec![0u32; planets.len()];
        for (item, prefs) in specials {
            let mut best = usize::MAX;
            let mut best_score = f32::MAX;
            for (i, p) in planets.iter().enumerate() {
                if i == home {
                    continue;
                }
                let rank = prefs.iter().position(|k| *k == p.kind).unwrap_or(5) as f32;
                let score = special_count[i] as f32 * 10.0 + rank + rng.f32() * 0.5;
                if score < best_score {
                    best_score = score;
                    best = i;
                }
            }
            special_count[best] += 1;
            planets[best].resources.push((item, 0.85));
        }

        StarSystem { seed, index, name, class, star_color, planets, home }
    }

    /// Fuel units needed for a shuttle hop.
    pub fn fuel_cost(&self, from: usize, to: usize) -> u32 {
        3 + 2 * (from as i32 - to as i32).unsigned_abs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::{Tech, RECIPES, SHIP_PARTS, SK};

    /// Follows the whole production chain: from what can be dug up in a system, through every
    /// recipe, to every structure and every starship part.
    #[test]
    fn full_production_chain_is_reachable() {
        for seed in 0..60u64 {
            let s = StarSystem::generate(seed * 7919 + 3, (seed % 4) as u32);
            let mut have: Vec<Item> = vec![Item::Organic, Item::Sample, Item::DataFragment];
            for p in &s.planets {
                for (item, _) in &p.resources {
                    if !have.contains(item) {
                        have.push(*item);
                    }
                }
            }
            assert!(!s.planets[s.home].flora.is_empty(), "home planet needs plants for Organic");
            loop {
                let before = have.len();
                for r in RECIPES.iter() {
                    if r.data == 0 && !have.contains(&r.output.0) && r.inputs.iter().all(|i| have.contains(&i.0)) {
                        have.push(r.output.0);
                    }
                }
                if have.len() == before {
                    break;
                }
            }
            for (item, _) in SHIP_PARTS {
                assert!(have.contains(&item), "seed {seed}: {item:?} can never be produced");
            }
            for kind in SK::BUILDABLE {
                for (item, _) in kind.def().cost {
                    assert!(have.contains(item), "seed {seed}: {kind:?} needs unobtainable {item:?}");
                }
            }
        }
    }

    #[test]
    fn early_game_needs_only_home_resources() {
        // Everything up to and including the shuttle and its fuel must be buildable on the home planet.
        let s = StarSystem::generate(11, 0);
        let mut have: Vec<Item> = vec![Item::Organic];
        have.extend(s.planets[s.home].resources.iter().map(|r| r.0));
        let early_tech = [None, Some(Tech::Steel)];
        for _ in 0..6 {
            for r in RECIPES.iter() {
                if r.data == 0 && early_tech.contains(&r.tech) && !have.contains(&r.output.0) && r.inputs.iter().all(|i| have.contains(&i.0)) {
                    have.push(r.output.0);
                }
            }
        }
        for kind in [SK::Smelter, SK::Research, SK::Fabricator, SK::Beacon, SK::BioGen, SK::Storage, SK::Solar, SK::Battery, SK::ChemLab, SK::Shuttle] {
            for (item, _) in kind.def().cost {
                assert!(have.contains(item), "{kind:?} needs {item:?}, which the home planet cannot provide");
            }
        }
        assert!(have.contains(&Item::Fuel));
    }

    #[test]
    fn tech_tree_has_no_dead_ends() {
        for t in Tech::ALL {
            let mut cur = Some(t);
            let mut depth = 0;
            while let Some(c) = cur {
                cur = c.def().prereq;
                depth += 1;
                assert!(depth < 20, "{t:?} has a cyclic prerequisite chain");
            }
        }
    }
    #[test]
    fn systems_are_winnable() {
        for seed in 0..200u64 {
            let s = StarSystem::generate(seed, (seed % 3) as u32);
            for need in [Item::Heliosite, Item::TitaniumOre, Item::Lithium, Item::Uranite] {
                assert!(s.planets.iter().any(|p| p.resources.iter().any(|r| r.0 == need)), "seed {seed} lacks {need:?}");
            }
            let h = &s.planets[s.home];
            for need in [Item::IronOre, Item::CopperOre, Item::Quartz, Item::Carbon, Item::Ice] {
                assert!(h.resources.iter().any(|r| r.0 == need));
            }
            if seed < 4 {
                println!("{} ({}): {:?}", s.name, s.class.1, s.planets.iter().map(|p| format!("{} {:?}", p.name, p.kind)).collect::<Vec<_>>());
            }
        }
    }
}
