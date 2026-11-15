# Brief: power cables and oxygen tethers

Read `docs/dev/CONTEXT.md` first.

## Why

The owner's feedback on the first build: "There is no connection between the things you build.
A cabling logic would be good. And an oxygen tether is needed, for leaving the main area
safely."

Today power is one global pool per planet (`gameplay::tick_structs`): every generator feeds
every machine wherever it stands. Oxygen is a fixed radius around the pod, shuttle, shipyard
and each Oxygen Station (`StructDef::oxygen`); outside those circles the tank drains.

## What done looks like

1. **Cabled power networks.** Structures only share power when they are connected by cables.
   A network is a connected component; generators, batteries and consumers interact only inside
   it. A machine with no powered network does not work and the player can tell why at a glance
   (in the world and in its panel). The player can connect and disconnect things themselves,
   with a length limit per cable, and `SK::Pylon` carries cables over distance. Make the common
   case effortless (for example: a new structure placed close to an existing one is cabled
   automatically) and keep manual control for everything else. It must be fully usable with
   touch only, and with keyboard + mouse.
   Cables are visible in the world: they hang or lie between the structures without cutting
   through the ground too badly, and a powered cable looks different from a dead one.
   The HUD power readout should describe the network the player is dealing with, not the planet.
2. **Oxygen tethers.** `SK::Tether` posts chain from an oxygen source outward:
   a post is live when a chain of posts within reach leads back to a source, the chain is drawn
   as a line that shows whether it is live, and the player breathes when near a live post, with
   a visible line from that post to the astronaut's backpack. Placing tethers while walking
   away from base must be quick (one key, one touch button), they must be cheap, and it must be
   possible to pick them up again. Losing the link (walking too far, a broken chain) should be
   noticeable before the tank is empty. Shrink the free oxygen bubble around the pod and the
   other sources enough that tethers actually matter, without making the first ten minutes
   miserable; the Oxygen Station should need power to work now that power is local.
3. **Everything keeps working around it.** Bases on other planets keep running while the
   player is away (networks are evaluated per planet from save data alone, no entities, no
   voxels). The shuttle travels with the player and leaves its cables behind cleanly. Dismantling
   removes links. The quest that asks for an Oxygen Station and the journal / tutorial texts
   that explain oxygen stay truthful: update `story.rs` wording (Turkish and English) where the
   rules changed, and consider one early objective that teaches tethers.
4. **Rules as tested pure logic.** Put the graph rules (components, per-network power balance
   with batteries, tether reachability) in functions that take plain data and cover them with
   unit tests, next to the existing `tick_structs` tests, which must be adapted rather than
   deleted.

## Constraints

- `Structure::links: Vec<u32>` (symmetric ids) already exists in the save for this; use or
  extend it. Balance numbers (cable length, tether reach, bubble radii) are yours to choose;
  state them in the report so they can be tuned.
- Planets are about to become several times larger (radius 220-380 m) and the terrain will be
  streamed around the player; do not assume terrain data exists far from the player. Use
  `VoxelWorld::sample / raycast` only near the player and degrade gracefully (straight
  segment) elsewhere.
- Keep per-frame cost flat: do not rebuild every cable mesh every frame.

## Files

You own a new `src/network.rs`, plus `src/gameplay.rs`, `src/state.rs`, `src/items.rs`,
`src/story.rs` and `src/debug.rs`. In `src/ui.rs` keep your changes to small, clearly separate
additions (the UI is rewritten next; put network-specific panel content in functions the
rewrite can call). Two other people are rewriting `world.rs` / `voxel.rs` / `player.rs`
(terrain streaming, camera) and replacing the models (`meshgen.rs`, `spawn_structure`): keep
your own render assets in your module instead of adding to `world::Gfx`, and use the
placeholder meshes for the tether and pylon as they are.

## Verify before you report

Tests green. Bot runs with screenshots: a base where the smelter only works once cabled to the
pod (extend the `craft` bot), a dead vs. powered cable, a tether chain leading away from the
pod with the player breathing at the far end and losing oxygen beyond it, picking a tether up.
Show the touch path too (`LOSTRANOUT_CLICKS` on the touch buttons; touch UI can be forced on
desktop if you add an env switch for it).
