# Lostranout: shared context for contributors

3D exploration and survival game in Rust (Bevy 0.16.1 + bevy_egui 0.34, multipass mode). A survey ship
loses its fuel, drifts for centuries, crashes on a procedural voxel planet; the player mines,
smelts, researches, builds a shuttle, visits other planets and finally builds a starship.
Targets: Linux (X11), Android (arm64, cargo-apk, NativeActivity, touch), Windows (cross build).
UI text is bilingual: every user-visible string needs Turkish and English (`L2 = (tr, en)`).
The owner is Turkish and plays on both PC and a Galaxy A36 phone.

## Module map (src/)

| File | What lives there |
|---|---|
| `lib.rs` | `AppState` (Menu, Intro, Loading, Playing, Ending), `Ui` resource (all UI/tool state, `actions` queue), `Cine`, system registration |
| `galaxy.rs`, `names.rs` | `StarSystem::generate(seed, index)` → planets (`PlanetDef`), resources, flora species, names. Pure, deterministic |
| `voxel.rs` | `TerrainGen` (height, density, caves, ore veins, `color_at`), `VoxelWorld` (16³ chunks of i8 density, sample/normal/raycast/edit, faceted mesher, edit export/import) |
| `terrain.rs` | `Streamer`: chunks generated / meshed on worker threads around the player, far planet shell, eviction |
| `decor.rs` | `DecorStream`: plants, ore crystals, specimens, black box fragments placed per region with stable ids, streamed in and out |
| `meshgen.rs` | `MB` mesh builder (vertex-colour meshes): plants, crystals, planet ball, star dome, fallbacks for structures |
| `models.rs` | Blender-made models (astronaut, smelter so far) embedded from `assets/models/models.lrm`; `tools/blender/` regenerates it |
| `network/` | power cables, oxygen relays ("Oksijen Halatı"), pure graph rules with tests, wire visuals, HUD hooks |
| `items.rs` | `Item`, `SK` (structure kinds) with `def()`, `Tech`, `RECIPES`, `Inventory`, `Lang`/`tx` |
| `state.rs` | `Game` (entire save), `Structure`, `PlanetSave`, save/load (bincode, `SAVE_VERSION`) |
| `story.rs` | intro, journal entries, quests, ending |
| `world.rs` | `Gfx` shared assets, menu scene, `load_planet`, `spawn_structure`, `sync_structs`, sky/day cycle |
| `player.rs` | `gather_input` (kbd/mouse/touch → `Controls`), `spawn_player`, `player_move` (density-field collision), `camera_follow` |
| `gameplay.rs` | excavator (terrain tool), harvesting, placement, interaction, `tick_structs`, `simulate` (oxygen, death, quests, autosave), actions, cinematics |
| `ui.rs` | all egui UI: menu, HUD, tablet tabs, structure panels, touch controls |
| `debug.rs` | scripted bots for automated runs |

Conventions: planet centre is the origin, 1 voxel = 1 m, density > 0 is solid, "up" at a point
is `pos.normalize()`. Models stand on their origin, +Y up, facing -Z. Colours are given as
sRGB `[f32; 3]` and converted when written to vertices. One shared vertex-colour
`StandardMaterial` (`Gfx::vc`) renders almost everything.

## Shared contracts (everyone codes against these)

- **Depth zones.** `depth = gen.height(dir) - r` for a point at radius `r` in direction `dir`.
  Surface: depth < 4 m. Upper caves: 4-40 m. Deep caves: below 40 m (down to the solid core).
  Cave generation puts an upper cave layer roughly in 6-35 m and a deep layer roughly in
  45-120 m, connected by shafts; hazards and story triggers classify places with these bands.
- **Planet distance tiers.** `home = sys.home`; a planet `i` is *near* when `|i - home| == 1`
  and *far* when `|i - home| >= 2`.
- **Names and IP.** Only the ship AI has a name: MİRA (TR) / MIRA (EN). The crew are referred
  to by role only. The lost ship is Öncü (EN Pioneer), the new one Öncü II / Pioneer II.
  Never reference or imitate other games: no other game's names, characters, item names or
  distinctive terms (the tool is "Kazı Aleti" / "Excavator", specimens are "Numune" /
  "Specimen", the assembler is "Üretim Tezgahı" / "Assembler", relays are "Oksijen Halatı" /
  "Oxygen Relay"). All content is original.
- **Saves.** Do not change `SAVE_VERSION`; the integrator bumps it once after merging. Adding
  fields to `Game` / `PlanetSave` / `Structure` is fine (old saves are discarded anyway).

## Build and test

```bash
source ~/.cargo/env
cargo build                # dev profile (deps optimised), ~15-40 s incremental
cargo test --lib           # must stay green; add tests for new pure logic
cargo check --lib --target aarch64-linux-android   # catches Android-only breakage
```

- Every shell command that runs cargo or `tools/shot.sh` must first
  `export CARGO_TARGET_DIR=/home/hvkmjij/Masaüstü/Lostronaut/target`. All worktrees share that
  one target dir (a fresh one costs 7 GB and six minutes). Others build at the same time, so a
  build may wait for the lock. Never run `cargo clean`, never touch `Cargo.lock` casually.
- No sudo on this machine. No new native/system dependencies. Audio is unavailable (no ALSA
  headers). Keep Bevy features minimal; anything you add must build for Android and Windows.
- New Rust dependencies need a strong reason; say so in your report.

## Running the game for verification

`tools/shot.sh <out.png> <seconds> ENV=VAL...` starts the debug build, waits, captures the real
window (including egui, which Bevy's own screenshot misses) with ImageMagick `import`, kills
the game and prints `[bot]` log lines. Look at the PNG with your image-reading tool.

Environment variables understood by the game (see `lib.rs::start_from_env`, `debug.rs`):

- `LOSTRANOUT_AUTOSTART=<seed>` skip menu, new game with that seed. `LOSTRANOUT_CONTINUE=1` load the save.
- `LOSTRANOUT_HOME=<dir>` save directory (shot.sh uses a temp dir by default).
- `LOSTRANOUT_CHEAT=1` 40 of every item + 9000 Data. `LOSTRANOUT_TIME=<secs>` start time (night etc).
- `LOSTRANOUT_UI=inv|build|research|map|journal|pause|machine<ID>` opens a panel after 4 s.
- `LOSTRANOUT_BOT=walk|dig|fill|flat|mouse|place|travel|finale|craft` scripted input; prints
  `[bot] t=.. fps=.. inv=.. soil=..` every 2 s. `LOSTRANOUT_DEST=<planet index>` for `travel`.
- `LOSTRANOUT_CLICKS="4.5:857,270;10:497,227"` real egui clicks at time:x,y (logical px).
- `LOSTRANOUT_TITLE` window title (shot.sh sets a unique one).

Rules for test runs, they matter because the owner is using this desktop while you work:

- Test windows must never take keyboard focus (already handled when AUTOSTART/CONTINUE is set)
  and you must never move the real mouse pointer (`Window::set_physical_cursor_position` warps
  it; use `Ui::dbg_mouse` or the click bot instead). Keep runs short (under ~25 s) and do not
  leave game processes running.
- Extend `debug.rs` with new bot modes when you need to drive a new feature; a feature that
  was never exercised by a run or a test does not count as done.
- A phone may or may not be attached; do not use `adb` unless your brief says so.

## How the work is organised

Several people work in parallel, each in their own git worktree on their own branch; the
integrator merges the branches into `master` afterwards. Your brief names the files you own.
You may edit other files when the feature needs it, but keep those edits small and local (a new
field, a call, a match arm) so the merge stays clean, and list every such file in your report.
Do not reformat or reorder code you are not changing. Do not edit `README.md`, `dist/`,
`Cargo.lock` (unless a new dependency forces it) or the briefs.

**Commit to your branch after every working milestone** (it builds and tests pass), with a
short English message. A session can be cut off at any moment by a usage limit; whatever is
not committed is lost, and committed work can be resumed by you or someone else. Never push,
never switch or touch other branches, never rewrite history.

Interfaces already prepared in the base you start from: `Item::Scrap` (+ smelter recipe),
`SK::Tether`, `SK::Pylon` (with placeholder meshes), `Structure::links: Vec<u32>`,
`SAVE_VERSION = 4` (old saves are simply discarded; leave the constant alone). Terrain tools
were just slowed down (`VoxelWorld::edit` now takes a linear max density step per call).

## Report format (your final message)

1. What now works, in plain sentences, and how you verified each part (test name, bot run,
   screenshot path).
2. What you did not finish or could not verify, and anything that behaves differently from the brief.
3. Files changed outside your owned set, with one line each on what and why.
4. Anything the integrator must do by hand (system registration order, constants to tune).

Screenshots you want the integrator to see: save them under `docs/dev/shots/<your-package>/`.
