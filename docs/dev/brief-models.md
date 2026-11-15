# Brief: Blender-made models for the astronaut, structures and items

Read `docs/dev/CONTEXT.md` first.

## Why

The owner's feedback on the first build: "The objects and the character are far too simple. It
would be better if you opened Blender from flatpak, made the character and the buildable things
there and embedded them in the game." They also called the UI "AI slop" and asked for more
graphics; a later UI pass will use rendered icons that you produce here.

Today every model is a handful of boxes and cylinders assembled in `src/meshgen.rs`
(`structure_mesh`, `astronaut`, `sample_mesh`, `fragment_mesh`, `starship_mesh`, `flame_mesh`).

## What done looks like

1. **A reproducible Blender pipeline.** Blender 5.2 is installed as a flatpak
   (`flatpak run org.blender.Blender -b -P script.py`, background mode works; a Blender MCP
   tool may also be available but do not depend on it). Scripts under `tools/blender/` build
   every model from code, so the whole asset set can be regenerated with one command.
   Document that command at the top of the main script.
2. **Models with real design**, stylised low-poly in a stylised low-poly style of our own: chunky readable
   silhouettes, bevelled edges, panel lines, feet, hatches, pipes, vents, antennae, a clear
   colour scheme (white hull, orange accents, dark mechanical parts, coloured glass and
   lights). Each structure should be recognisable for what it does at gameplay distance
   (camera 6-20 m away) and be believable as a set from one manufacturer. The astronaut needs
   proper proportions: helmet with visor, torso, a distinct backpack, arms, legs, boots, the
   terrain tool in hand.
   Needed: astronaut; escape pod (crashed, scorched); smelter; fabricator; chemistry lab;
   research station; solar panel; bio-generator; battery; storage crate; oxygen station
   (`SK::Beacon`); oxygen tether post (`SK::Tether`); power pylon (`SK::Pylon`); shuttle;
   radioisotope generator; star shipyard with the starship growing on it (0-4 hull sections,
   fusion core, nav nose, cryo ring, driven by what was delivered, see `sync_structs`);
   rocket flame; research sample; black box fragment; three scrap pieces and two large wreck
   chunks (the terrain work will scatter them; just make them loadable by name).
   Plants and ore crystals stay procedural in Rust.
3. **Embedded, no runtime asset files.** The game must keep working as a single binary / APK
   with nothing to unpack: bake the exported data into the executable (`include_bytes!`).
   Stay within the existing vertex-colour material approach (no textures, no new Bevy
   features) unless you find a strong reason; glowing parts may use the existing unlit
   material. Mind the size: the whole model set should add a few MB at most, and the phone
   draws these without shadows, so keep triangle counts sensible (a structure in the low
   thousands, not tens of thousands).
4. **Named, pivoted parts where motion helps.** The astronaut's limbs swing while walking and
   the right arm raises while the tool is used (see the end of `player_move`); keep that
   working with the new mesh parts, and expose the backpack as its own named part with a known
   position because the UI pass will make panels emerge from it. Give structures a little
   life where it is cheap: a part that turns or pulses while the machine has a job
   (`Structure::job`), the research dish, the solar panel. Keep it simple and data-driven.
5. **Icons.** Render one icon per `Item` (30) and per `SK` (15) from small item models /
   the structure models: transparent background, consistent three-quarter view and lighting,
   128x128 PNG, into `assets/icons/item_<EnumName>.png` and `assets/icons/sk_<EnumName>.png`
   (exact enum variant names). Raw ores should read as rough chunks in the item colour
   (`Item::def().color`), refined items as ingots / plates / canisters, components as
   recognisable parts. Do not wire them into the UI; someone else does that next.

## Lessons from the rolled-back attempt

- Its contact sheet was rendered top-down from far away, so nobody noticed the models were
  blobs. Render every model large, from a three-quarter gameplay-like angle, and look.
- In the game its multi-part models came apart (roofs beside the hull, limbs floating): part
  pivots / parent transforms / axis conversion were wrong and it was never checked in a
  screenshot. Verify each model in the running game next to the astronaut, at least once.
- Its icons were read from an assets folder at runtime and were simply missing in the build
  and on Android. Everything must be embedded.
- Many item icons were unreadable arrangements of loose primitives. An icon must read as one
  object at 48 px.

## Constraints

- Structures are placed with `StructDef::radius` as footprint and are collided as cylinders of
  that radius; keep each model within its footprint (change the radius in `items.rs` only if a
  model truly needs it, and say so).
- Models stand on the origin, +Y up, facing -Z in the game (Blender is Z-up, convert on export).
- The spawn code must stay cheap: structures are respawned when travelling and synced every
  frame by id (`sync_structs`).

## Files

You own `tools/blender/**`, `assets/models/**`, `assets/icons/**`, a new `src/models.rs`, the
structure / astronaut / sample / fragment / starship / flame parts of `src/meshgen.rs`, and in
`src/world.rs` only `spawn_structure`, `sync_structs` and the model-related `Gfx` fields, and in
`src/player.rs` only `spawn_player` and the limb animation. Someone else is rewriting the rest
of `world.rs` and `player.rs` (terrain streaming, camera) right now, and a third person is
changing `gameplay.rs` / `state.rs` / `items.rs`: stay out of those regions.

## Verify before you report

Render a contact sheet of all models and one of all icons from Blender and look at them
critically; iterate on anything that reads as a primitive blob. Then in-game screenshots: the
astronaut walking (bot `walk`), a base with several structures (bot `place` with
`LOSTRANOUT_CHEAT=1`; extend the bot to place more kinds), the shuttle launch (`travel`) and
the starship (`finale`). `cargo check --lib --target aarch64-linux-android` must pass.
