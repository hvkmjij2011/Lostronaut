# Brief: layered caves, wreckage, ore placement

Read `docs/dev/CONTEXT.md` first, especially "Shared contracts".

## Why

The owner's feedback: "There are no caves, only cave-ins; only the surface exists. Below there
should be caves layer by layer. Maybe scrap parts in random places." and now "improve the ore
graphics and where they are". Today the density field only has thin noise tunnels in the top
~34 m, ore veins are noise blobs, surface ore crystals are scattered randomly, and the lost
ship's wreckage is not in the world at all (`Item::Scrap` exists with a smelter recipe, nothing
drops it).

## What done looks like

1. **Layered caves.** An upper cave layer (about 6-35 m deep) and a deep layer (about 45-120 m),
   following the depth zones in CONTEXT.md: chambers large enough to walk and build in, winding
   tunnels, vertical shafts and ramps linking the layers, and entrances that can be found from the
   surface (sinkholes, cliff mouths) without being everywhere. Caves differ by planet type and seed.
   The solid core stays. Generation stays deterministic, chunk-local and fast enough for the
   streamer (watch the `[stream]` timings).
2. **Ore placement that makes sense.** Veins run through rock as readable bands / pockets rather
   than noise splotches; richer and rarer ore deeper, so the deep layer is worth the trip;
   surface ore crystal clusters sit where a vein reaches the surface or a cave wall, so a cluster
   hints at a vein below it. Cave walls show ore. Keep the per-planet resource lists from
   `galaxy.rs` meaningful and the winnability tests green (every special resource exists off
   home, home supports the early game; the guaranteed starter deposits near the landing site stay).
3. **Wreckage of the Öncü.** Scrap pickups (harvested like crystals, give `Item::Scrap`) on the
   surface and in caves, plus a few large wreck sites per planet (hull plates, a torn tank, an
   engine bell) as landmarks you can see from a distance; black box fragments tend to lie in or
   near wreck sites. Use stable decor ids (harvested decor is stored in `PlanetSave::removed`).
   Models: build them from the mesh builder; a later pass may replace them with Blender models,
   so keep each wreck piece a separate mesh constructor.
4. **Cave decor.** Some cave-only growths or crystals that give a little light so caves are not
   pitch black beyond the helmet lamp. Coordinate nothing else visual: someone else owns colours
   and lighting (below).

## Constraints

- Gameplay relies on `VoxelWorld::{sample, normal, raycast, surface_point, edit}`,
  `world::offset_on_surface`, the streamer and `DecorStream`; keep those contracts. Tool yields
  from `edit` (soil / ore volumes) must keep working.
- Player spawn and the pod landing site must stay on open surface, never in a cave mouth.
- Performance: desktop 60 fps while walking through caves; phone budgets in `terrain::budget()`.

## Files

You own `src/voxel.rs` (except `TerrainGen::color_at`, which the visuals engineer owns),
`src/terrain.rs`, `src/decor.rs`, `src/galaxy.rs`, and the `ORES` table in `src/items.rs`. Others
are working in parallel on visuals (meshgen crystals/plants, `color_at`, sky, lighting), hazards
+ death (`gameplay.rs`, new `hazards.rs`) and the story (`story.rs`).

## Verify before you report

Tests green (add tests: caves exist in both bands for several seeds and planet kinds; landing
site is open surface). Screenshots: a cave entrance from the surface, a chamber in the upper
layer, a chamber in the deep layer (add a bot / env switch that teleports the player to the
nearest open space in a given band), a wreck site, a surface crystal cluster above a vein,
`[stream]` timings while roaming through caves.
