# Brief: interface overhaul, in-world panels, orbit travel

Read `docs/dev/CONTEXT.md` first.

## Why

The owner's feedback on the first build:

- "The interface looks very AI-slop. Raise the graphics a bit and, as far as possible, embed
  the tools into in-game 3D panels; for example it could come out of the backpack, or something
  else."
- "For travel between planets, let me choose the destination planet from orbit. At first every
  planet except the starting one should appear unknown, and open up once visited."

Today the UI is stock dark egui: a top row of text buttons, a centred "tablet" window with
lists (inventory, build, research, map, journal), a centred window per structure, flat HUD
boxes in the corners, and a 2D orbit diagram inside the map tab from which travel starts.
Item icons are painted polygons.

## What done looks like

1. **The tools live in the world.** Opening the backpack makes a panel come out of the
   astronaut's backpack in 3D space, with the camera framing astronaut and panel; a structure's
   controls appear on or beside that structure, tied to it, not as a window in the middle of
   the screen. Opening and closing is animated. The player should feel they are operating
   equipment, not browsing menus. Think equipment you operate in the world rather than menus. Vital gauges
   (oxygen, power, soil) are candidates for living on the backpack / tool too, with the
   screen-space HUD reduced to what truly must float.
   How you achieve it is yours to decide (egui rendered to a texture on world geometry with
   pointer picking, panels laid out in screen space but anchored to and connected with their
   3D object, real 3D slot meshes, a mix). Whatever you choose must stay crisp and usable with
   touch on a 2340x1080 phone (about 832x384 logical px) and with mouse on a desktop, and must
   not regress frame rate noticeably.
2. **A designed look instead of default widgets.** Rendered icons already exist in
   `assets/icons/item_<Item>.png` and `sk_<SK>.png` (128x128, transparent): use them everywhere
   an item or structure is shown. Give the interface an identity that fits a lone astronaut
   with salvaged equipment: a real type choice (fonts must cover Turkish: ğ ü ş ı İ ö ç; bundle
   an openly licensed font in the binary), framed panels with depth, clear hierarchy, state
   colours used sparingly, motion on feedback (pickups, completed jobs, new journal entry).
   Research should read as a tree with prerequisites, not a list. The build menu should show
   what you are about to place. No emoji as icons, no missing-glyph boxes.
3. **Travel from orbit.** Boarding a fuelled shuttle and launching takes the player up into an
   orbit scene: the star, the planets as lit 3D globes on their orbits, the camera free to look
   around. Planets never visited are unknown: no name, no surface colours, no resource list,
   only a silhouette with rough size and the fuel needed. Selecting a planet (tap / click the
   globe) shows its card; confirming flies there and plays the landing. Arriving reveals the
   planet for good (`PlanetSave::visited` already tracks this). The player can also go back
   down to the planet they came from. The system map tab follows the same known / unknown rule.
   Keep the fuel rules (`StarSystem::fuel_cost`) and the `travel` bot working (adapt the bot).
4. **Nothing lost.** Every function the current UI offers still exists: inventory with item
   details and discard, build placement with confirm / cancel and problem text, research,
   journal with unread marker, machine recipes with progress and collect, bio-generator
   fuelling, storage transfer, shuttle fuelling, shipyard deliveries and launch, dismantle,
   pause menu with save / language / exit / fps, main menu, intro, loading, ending, toasts,
   quest tracker, touch controls (joystick, jump, zoom, tool toggle, tether button), and the
   power-cable and tether controls added recently (see `network.rs` and its UI hooks).
   Both languages everywhere.

## Constraints

- World input must keep working around the UI: `player::gather_input` decides per pointer /
  touch whether it hits UI or the world; whatever panel technique you use has to feed that
  correctly (a touch on a world-space panel must not dig or move the camera).
- Android: NativeActivity, no soft keyboard, touch only; anything needing new Bevy features
  must build for `aarch64-linux-android` and Windows.
- The automated test hooks (`LOSTRANOUT_UI`, `LOSTRANOUT_CLICKS`, bots) are how this gets
  verified without a human: keep them working or replace them with equivalents, and use them.

## Files

You own `src/ui.rs` (split it into modules if that helps), a new orbit module, fonts and UI
art under `assets/`, and the travel flow in `gameplay.rs` (`Action::Travel`, the launch /
landing cinematics). Other areas are stable now; change them where the UI needs hooks.

## Verify before you report

Screenshots at 1280x720 and at a phone-like logical size (832x384; add an env switch to open
the window at a given logical size and to force the touch UI) of: main menu, HUD while walking,
backpack panel open, build menu, research tree, a machine panel with a running job, storage
transfer, the orbit scene with unknown and known planets, the selection card, journal. Drive
real clicks through the panels with the click bot (craft an item, unlock a tech, travel) and
show the `[bot]` log proving the state changed.
