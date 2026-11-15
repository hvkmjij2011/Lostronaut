# Brief: hazards, blacking out and being carried home

Read `docs/dev/CONTEXT.md` first, especially "Shared contracts" (depth zones, planet tiers).

## Why

The owner's request, in their words:

- "Add hazards. On the temperate planet only in deep caves; on the nearest one also in the upper
  caves; and on the far ones also on the surface."
- "When you die, or rather while being carried back to base, MIRA should say she dropped the
  weight and carried you back to base. Every time, so the items go. And not up, but directly to
  the base."

Today the only danger is oxygen running out (health drains, then a respawn near the closest
oxygen source, found with a surface raycast, with the message "Bilincini kaybettin. MİRA seni
üsse geri taşıdı.") and fall damage.

## What done looks like

1. **Hazards placed by depth zone and planet tier** exactly as the owner said:
   home planet: deep caves only; near planets (|i - home| == 1): upper and deep caves; far
   planets (>= 2): caves and surface. Several kinds, chosen per planet type so they fit the
   world (for example toxic spore pods, scalding vents on volcanic worlds, radiation around
   uranite, falling rocks / unstable ceilings in caves, crystal shards, acid pools, electric
   storms on a far surface; your design). Each hazard is telegraphed (visual cue before it
   hurts, distinct look), avoidable with attention, readable on a phone, and has a clear
   effect (health, oxygen, knockback, slowing...). They should make the deeper caves and far
   worlds feel dangerous without making the early game miserable. Some may be neutralised
   (dug out, harvested for something useful, avoided with a tech) if that fits.
   Placement is deterministic per region (seed + position) and streams with the player like
   decor does; hazards never sit on the landing site, the pad, or inside a base.
2. **Blacking out.** When health hits zero, MIRA carries the player back: the whole backpack
   content is left behind where the player fell, as a recoverable bundle marked on the HUD (one
   bundle per blackout; picking it up returns the items, it persists in the save until picked
   up). The player reappears immediately at the base, standing on the ground next to the base's
   main structure (the escape pod on the first planet, otherwise the shuttle or the base's
   oxygen source), never dropped from above and never placed on top of terrain far above a
   cave. Every time, MIRA says (exact text): TR "MİRA ağırlıkları bırakıp seni üsse geri
   taşıdı." EN "MIRA dropped the weight and carried you back to base." A short fade or cut is
   welcome. Soil canister and research Data are kept; only backpack items stay behind.
3. **Feedback.** Damage is felt (screen edge flash, sound is unavailable so make it visual,
   HUD health), a MIRA warning the first time each hazard kind is met (bilingual, short), and
   the cause of a blackout is mentioned.

## Constraints

- Machines on other planets keep running from save data alone; hazards only exist near the
  player and need no simulation elsewhere.
- Do not break oxygen relays / cable networks (`src/network/`); oxygen logic lives there.
- Everything bilingual, touch friendly.

## Files

You own a new `src/hazards.rs`, the blackout / respawn code in `src/gameplay.rs` and the fields
you need in `src/state.rs`. Others in parallel: caves + ore placement + wreckage (`voxel.rs`,
`decor.rs`, `terrain.rs`; the cave layers you place hazards in are being built right now, so
place hazards in whatever open space exists at the right depth and test with a dug pocket or
the existing tunnels), visuals (`meshgen.rs` crystals and plants, sky), story (`story.rs`; MIRA's
hazard warnings may live in your module, keep them short).

## Verify before you report

Unit tests for the placement rules (zones x tiers) and for the blackout (items moved to a
bundle, bundle recovery, respawn position is on the ground next to the base). Bot runs with
screenshots: each hazard kind's telegraph and its hit, a blackout ending next to the pod with the
MIRA message, the bundle marker and its recovery, one far-planet surface hazard.
