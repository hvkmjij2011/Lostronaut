# Brief: visuals pass (ores, plants, terrain colour, light, sky)

Read `docs/dev/CONTEXT.md` first.

## Why

The owner asks to "increase the graphics", naming the ore graphics first. Today ore crystals
are a few flat-shaded prisms on a grey blob, plants are primitive spheres and cones, terrain
colour comes from `TerrainGen::color_at` (flat colour fields with per-facet ore shades),
lighting is one sun plus a flat ambient, the sky is a clear colour plus a star dome and fog.
Everything is vertex-coloured with one `StandardMaterial`.

## What done looks like

1. **Ores you want to mine.** Each ore type has its own recognisable crystal / rock formation
   (shape language per material, e.g. iron as rusty angular nuggets, copper as teal-green
   crusts, quartz as clear prisms, heliosite glowing amber, uranite with a faint green glow),
   reading clearly at 5-25 m, with subtle emissive glow only where it fits. Ore inside terrain
   (veins in walls) should read as a different material from rock too: work inside
   `TerrainGen::color_at`, which you own (someone else is changing where the veins are).
2. **Flora with character.** Richer plant meshes per archetype (trunks with branches, leaf
   clusters with colour variation, glowing bulbs at night, swaying is optional and cheap),
   so different planets' flora feels alien and distinct. Keep `meshgen::plant_mesh` as the entry
   point and its cost sane: plants are numerous and streamed.
3. **Light and atmosphere.** Better day / night / dusk: sky colour gradient with a visible
   horizon glow and sun disc, atmosphere that depends on the planet (thin on barren worlds, thick
   and tinted on toxic ones), softer shadows, hemisphere-style ambient (sky colour from above,
   ground bounce from below) instead of flat ambient, and a readable night. Consider Bevy
   features that work on both desktop and the Android phone (Vulkan, no shadows there); keep a
   quality tier per platform (`world::mobile()`), and if you enable new Bevy features, check
   `cargo check --lib --target aarch64-linux-android` and keep the APK size reasonable.
4. **Terrain palette.** Surface, soil, rock strata and cave walls with more pleasing colour
   variety per planet type (still flat per facet, no blur), without hurting readability of ore.

## Constraints

- No textures from outside; everything is generated in code (vertex colours, materials).
- Desktop stays at 60 fps (GTX 1650); the phone (Galaxy A36) at 45+.
- Don't change gameplay, placement or generation of where things are; others own that.

## Files

You own `TerrainGen::color_at` in `src/voxel.rs` (only that function and helpers it needs),
the plant / crystal / specimen / fragment / star-dome / planet-ball parts of `src/meshgen.rs`,
and in `src/world.rs` the `Gfx` materials, the camera setup and `update_sky` (sky, sun, fog,
ambient). Others in parallel: caves + ore placement + wreckage (`voxel.rs` density, `decor.rs`,
`terrain.rs`), hazards + death (`gameplay.rs`, `hazards.rs`), story (`story.rs`).

## Verify before you report

Before / after screenshots of the same seed and camera for: a crystal field of each ore type
(add a debug switch that spawns one cluster of every ore in front of the player), a forest on
two different planet types, dusk, night, a toxic and a barren planet sky, a dug wall with a
vein. Report fps on desktop from the `[bot]` lines and state what the phone tier turns off.
