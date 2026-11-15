# Brief: a longer story

Read `docs/dev/CONTEXT.md` first, especially the "Names and IP" contract.

## Why

The owner asks to make the story longer. Today (`src/story.rs`): a 4-page intro, 20 journal
entries (8 of them black box logs decoded from fragments), 16 linear quests from first organic
to launching the starship, and an ending. MIRA, the ship's AI, narrates. The arc: the Öncü
(Pioneer) lost its fuel at T+212, drifted for centuries, MIRA had to choose which one of four
crew pods to keep powered (the player), a human signal says beacons lead home; the player builds
Öncü II and leaves for the next star. A new star system repeats the loop (`Game::advance_system`).

## What done looks like

1. **More story through the whole game,** not just more text at the start: MIRA reacts to
   meaningful moments (first cave, first deep cave, first hazard hit, first blackout, each new
   planet type visited, first wreck site, building key machines, nights, long absences...),
   additional black box logs that deepen the crew (by role only: commander, engineer, pilot,
   doctor) and MIRA's choice, discoveries that tell what happened to the Öncü's pieces in this
   system, and a stronger build-up to the launch. Objectives (quests) are extended to cover the
   mid and late game with clear, achievable steps that teach the systems (cables, relays,
   caves, research, other planets, the shipyard parts).
2. **A story that continues across systems.** The ending sends the player to the next star;
   give each following system its own chapter (new intro text when arriving, new logs or beacon
   messages, a reason to go on) for at least two more systems, then a believable open continuation.
3. **Tone and craft.** Restrained, warm, a little melancholic, with MIRA's dry humour; short
   paragraphs that read well on a phone. Turkish is the primary language and must read as
   natural Turkish (not a translation); English matches it. No names except MİRA / MIRA, ship
   Öncü / Pioneer. No references to other works.
4. **Triggers in one place.** Put the trigger checks for new entries in a function in
   `story.rs` (for example `story::check(game, &StoryCtx)` called once per frame from
   `gameplay::simulate`, with a small context struct for things the save does not know:
   depth zone, planet tier / kind, nearby wreck, last hazard hit). Other people are adding caves,
   wreckage and hazards right now; trigger on what you can observe (depth from
   `gen.height(dir) - r`, planet index / kind, `Game::deaths`, inventory, built structures) and
   add clearly named hooks (`StoryEvent::HazardHit`, `WreckFound`...) that their code can fire,
   with a short list in your report of where to call them.

## Constraints

- Saves: new journal ids must not reorder old ones; quests progress by index, keep that logic
  sound (`Game::quest`).
- Bilingual everything; the journal UI shows entries newest first and is fine as is.

## Files

You own `src/story.rs` and the journal / quest text; small hooks elsewhere (a call in
`simulate`, an event enum). Others in parallel: caves + wreckage (`voxel.rs`, `decor.rs`),
visuals (`meshgen.rs`, sky), hazards + blackout (`gameplay.rs` blackout code, `hazards.rs`):
keep your edits in `gameplay.rs` to the single call site.

## Verify before you report

Tests: every journal entry id is reachable by some trigger or quest, quests are completable in
order on a few seeds (simulate the conditions), no names other than MİRA/MIRA appear in the
text (a test that scans the strings). A bot run showing a few new entries arriving in the
journal (screenshot of the journal tab) and the new intro of the second system.
