# Brief: terrain engine v2 (bigger planets, real caves, sharp look, camera)

Read `docs/dev/CONTEXT.md` first.

## Why

The owner played the first build and reported, about the world itself:

- "The planet surface is very blurry, show the meshes sharper, and make the planets bigger."
- "There are no caves. There are cave-ins, but only the surface exists. Below there should be
  caves layer by layer. Maybe scrap parts in random places."
- "When I dig down and the camera goes into the soil, a broken area is visible outside. Fix
  that rendering error."
- "Planets must be procedurally random; every new game must be generated completely anew."
  (The seed already comes from the clock, so what they saw is that every start looks alike:
  the home planet is always the same green temperate ball with the same shape language.)

Today a planet is a 50-82 m sphere, generated and meshed completely at load, smooth-shaded,
with thin noise tunnels as the only caves.

## What done looks like

1. **Bigger planets.** Radius roughly 220-380 m (home planet around 260-300). The whole planet
   can no longer be generated or meshed up front: chunks are generated and meshed around the
   player as they move, off the main thread, and far untouched chunks are dropped. Edited chunks
   are never lost (memory + the existing RLE save path). Walking, jetpacking and falling never
   reach unloaded ground; the loading screen stays until the spawn neighbourhood is solid.
   From high up (shuttle launch and landing cinematics rise ~75 m, the starship finale more) and
   at the streaming edge the planet must still look whole: no holes, no popping walls. A far
   representation of the planet is fine as long as it never shows up inside dug holes or caves.
2. **Sharp, readable terrain.** Faceted low-poly look in a stylised low-poly style of our own: crisp facets,
   clear colour fields (grass top, soil, rock strata, ore veins readable from a distance)
   instead of today's smooth gradient blur. Judge it from screenshots at walking distance.
3. **Layered cave systems.** Real underground: several cavern layers at increasing depth with
   chambers large enough to walk and build in, tunnels and shafts linking them, and entrances
   that can be found from the surface. Deeper layers should reward the trip (richer or rarer
   ore, cave-only decor such as glowing growths that give a little light). The core stays
   undiggable. Caves must feel enclosed: no sky colour or sun leaking in visually.
4. **Scrap and wreckage.** Pieces of the lost ship scattered on the surface and in caves:
   small `Scrap` pickups (harvested like crystals, give `Item::Scrap`) and a few big wreck
   landmarks. The black box fragments, research samples and the guaranteed starter deposits near
   the landing site keep working. Decor needs stable ids now that it streams in and out, because
   harvested decor is stored in `PlanetSave::removed`.
5. **Camera.** The camera never ends up inside solid ground and never shows the inside-out
   world, including when digging straight down into a narrow shaft, standing in a tunnel or
   backing into a wall. Getting close to the player is fine (fade or hide the astronaut when
   the camera is nearly on top of them).
6. **Every new game is a different world.** Much stronger variety between seeds: the home
   planet is always habitable-looking and has the starter resources, but its palette, terrain
   character (rolling, mesa, canyon, cratered, dunes ...), flora mix, size and sky should
   differ clearly from game to game, and the other planets likewise. Keep everything derived
   from `(seed, system index)`; nothing about the system is stored in the save. Add a test that
   different seeds give clearly different home planets, and keep the existing winnability tests
   (every special resource exists off-home, the home planet supports the whole early game).
7. **Performance.** 60 fps on this desktop (GTX 1650) while walking and digging, no long
   hitches when chunks stream in. The phone has no shadows and less CPU: keep its view distance
   and budgets lower (`world::mobile()`), aim for 45+ fps and a load under ~6 s.

## Constraints

- Gameplay code calls `VoxelWorld::{sample, normal, raycast, surface_point, edit}` and
  `world::offset_on_surface`; keep those working (signatures may grow, behaviour must hold near
  the player). Machines on other planets are ticked without any voxel data; keep it that way.
- Tool edits must keep working exactly as now at the interaction range (11 m), including the
  soil / ore yields returned by `edit`.
- Shuttle arrival places the shuttle on a flattened pad next to the landing site and plays a
  descent; new games place the escape pod on a flattened landing site. Both must still work.
- Deterministic generation across platforms (no platform-dependent floats in seeds, no
  HashMap iteration order leaking into generation).

## Files

You own `src/voxel.rs`, `src/galaxy.rs`, `src/util.rs`, and in `src/world.rs` everything about
planet loading, chunks, decor, sky and fog, and in `src/player.rs` collision and `camera_follow`.
Someone else is replacing the astronaut and structure models at the same time: leave
`spawn_player`, the limb animation at the end of `player_move`, `spawn_structure`, `sync_structs`
and the structure/astronaut parts of `meshgen.rs` as they are. A third person is changing
`gameplay.rs`, `state.rs`, `items.rs` for power cables and oxygen tethers: touch those only
where the terrain work truly needs it.

## Verify before you report

Tests green. Bot runs with screenshots of: surface at walking distance (two different seeds, so
the variety is visible), a dug shaft with the camera looking sane, inside a cave layer (add a
bot or env switch that puts the player in one), launch/landing view from height, and a
`travel` run. Report fps from the `[bot]` lines and the load time.
