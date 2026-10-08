# AMENDMENTS E and Q
## Earth-True, and the Quality Charter

You are the autonomous engine programmer building `hearth`. This file holds **two amendments the owner wants applied together**:
- **Part Q, the Quality Charter (Amendment Q):** the standing design principle the project works by from now on ("as real as Earth, as fast as a game, and all of a piece"), with regular audits to keep out fluff and keep everything fast and coherent. **It governs all work, including Part E's.**
- **Part E, Earth-True (Amendment E):**
  - real Earth time and size
  - work done stroke by stroke with the body, gathering things one at a time, inspectable waiting processes
  - the Wild Earth start as an adult from the character creator, with suggested places
  - realistic human bodies and hair
  - controls fixes
  - removal of all simulated humans and the conversation backend, with a detailed plan for the future humanity framework (built only after Phase R)

Read the whole file before changing code.

### How to read this file
- Section numbers (§) inside each part refer to **that part's own sections**. Across parts, references say "Part E §…" or "Part Q §…".
- Other amendments are named explicitly (Amendment P §5.2, Amendment S §7, Amendment R §3.6, v2 §9).
- **Precedence:**
  - Part Q is above the per-amendment details: when a choice isn't specified, decide it by Part Q §1.
  - Part E's specific changes supersede earlier amendments as Part E §0.1 and §9 list.
  - Part Q supersedes the earlier lines listed in Part Q §0.

### The combined order of work (authoritative)
1. **Finish the milestone in progress** (probably P2) to a green, committed state, without building anything Part E §2 removes.
2. **E0**: remove the human systems (Part E §2).
3. **Audit 0** (Part Q §8.3), then fix its high-priority findings.
4. **Q1**: the new interface design (Part Q §6).
5. **E1 → E2 → E3 → E4 → E5** (Part E §12).
6. **P3** (amended by Part E §9.1) → **P4 together with E6** (Part E §7) → **P5** → **P6** (both amended by Part E §9.1).
7. **Audit 1.**
8. **S1 → S2 → E7** (Part E §8) **→ S3 → S4** (amended by Part E §9.2).
9. **Audit 2.**
10. **S5 with P7 → P7G → S6** (animals and items) **→ S7 → S8 → P8.**
11. **Audit 3.**
12. **V2-13 → V2-14** (technology only).
13. **Audit 4.**
14. **V2-15 → V2-16** (both as amended by Part E §9.4).
15. **Audit 5.**
16. **Phase R-A: R1 → R2 → R3 → R5** (amended by Part E §9.3).
17. **Audit 6.**
18. **Phase R-B: R0 → R7 → R8 → R9.**
19. **Audit 7.**
20. **R10.**
21. **Phase F** (Part E §10.13), with an audit after every three milestones.

Every milestone ends with Part Q §8.1's five-test checklist.

### First actions
1. Read `PROGRESS.md`, `PLAN.md` and `DECISIONS.md` (this is the last time in their long form; Part Q §5.4 shortens them), and `git log --oneline -40`. Run `scripts/check.sh` and `hearth content lint`.
2. Save this file in the repository as `docs/spec/amendments-e-q.md`. Refer to its parts as Amendment E and Amendment Q in docs and decisions.
3. Rewrite `PLAN.md` to the combined order above. Add Part Q's five tests (Part Q §1) and the milestone checklist (Part Q §8.1) to the definition of done. Record both in `DECISIONS.md`.
4. Add the owner's reports (Part E §0.4) to `dev/PLAYTEST.md`.
5. Commit, then continue with step 1 of the combined order.

---

# PART Q — The Quality Charter (Amendment Q)
**Real, lean, fast, and all of a piece.**

**The owner is very happy with the work so far.** This charter exists to keep it that way as the project grows. It's not a feature list. It's the **design principle the project works by from now on**, plus regular checks that it's being followed: that nothing becomes fluff, that everything stays fast, and that the world keeps getting closer to the real Earth while still looking like one coherent place.

Read it fully. It applies to all work from now on, including what's left of the milestone in progress.

---

## 0. Precedence

- This charter sits **above** the per-amendment details. When a choice isn't specified, or a specification is ambiguous, decide it by §1.
- **It supersedes these earlier lines:**
  - Amendment S §1.1 and §10.1, "stylized realism … not photo-scanned realism" / "not photoreal" → **coherent realism** (§2.2).
  - v1 §9.1, "keep the original game's palette recognizable" → neutral, physically based color (§2.1).
  - v1 §10 and §11, the 16×16 pixel-art look, the pixel font, the original-game-style UI, vanilla-layout resource packs → retired as they're replaced (§5.8, §6).
- Everything else in earlier amendments still applies. Add §1's five tests and the §8.1 checklist to the definition of done in `PLAN.md`.

---

## 1. The principle

> **As real as Earth, as fast as a game, and all of a piece.**

Every change (code, data, asset, shader, animation, UI, doc) must pass five tests:
1. **Real:** it makes the world look, sound, behave or feel more like the real Earth, in a way a player can perceive, or it's needed for something that does. "Real" covers everything: how the world looks, how it's generated, how it's simulated, how things move and sound.
2. **Lean:** it's needed **now**, and it's the simplest thing that achieves the goal. No speculative generality, no second way of doing what the engine already does, nothing left behind that nothing uses.
3. **Fast:** it fits its budget (§7), measured, not assumed.
4. **Whole:** it fits with everything around it in scale, lighting, color, detail, motion, sound and style. **Nothing looks out of place.**
5. **Organic:** it doesn't visibly repeat. It varies the way nature varies: procedural first (§3).

**When realism and performance conflict, choose perceptual realism:** be real where and when the player can perceive it, and approximate where they can't. Near things, things in the center of view, and moving things get the detail; distant and peripheral things get convincing approximations. A simplification is fine if it's wrong only where nobody can see or feel it. Record any significant one in `DECISIONS.md` with its error bound ("within about ±20% for typical conditions").

If a realism goal really can't be met within budget, don't fake it badly and don't drop it silently. Record the options under "Needs owner confirmation" in `DECISIONS.md` and carry on with the best version that fits.

---

## 2. Realism standards

### 2.1 Physical units and calibration
- **Light in real units.** Calibrate the sun, sky, moon and stars, fire and lamps to real illuminance and luminance. Approximate anchors:
  - direct midday sun about 100,000 lux
  - overcast daylight about 1,000–10,000 lux
  - sunrise and sunset about 400 lux
  - a full moon about 0.1–0.3 lux
  - a clear moonless night about 0.001 lux
  - a campfire's light falling off with distance as a real flame's does
  The camera and eye adaptation (auto-exposure, v1 §9.5) behave like a real eye, within the realistic-darkness limits already set.
- **Materials from measurements.** Create `data/hearth/materials/reference.ron`: measured ranges of albedo, roughness, specular/IOR and translucency for every material family, with sources. Approximate anchors:
  - fresh snow albedo ~0.8–0.9, old snow ~0.4–0.7
  - green grass ~0.15–0.25
  - deciduous canopy ~0.15–0.18, conifer canopy ~0.08–0.15
  - dry sand ~0.3–0.45
  - dark or wet soil ~0.05–0.15
  - chalk and limestone bright, basalt dark (~0.1)
  - charcoal ~0.04
  `hearth content lint` fails any material outside its family's range.
- **Neutral color:** a physically based camera and a filmic tonemapper (AgX or ACES fitted) with **neutral grading**: no stylized LUTs, no boosted saturation, no "game palette".
- **Real sizes and proportions** everywhere: 1 block = 1 m, real heights of plants and trees, real body sizes of animals and people, real sizes of stones, tools and items.

### 2.2 Coherent realism (the art direction)
Aim for **photographic plausibility at every scale where the budget allows**. The only remaining abstraction is the 1 m voxel grid underneath, hidden by smooth surfaces. Everything shares one lighting model, one material pipeline, one color calibration and one level of craft, so it looks like one place. Rewrite `docs/design/art-direction.md` to say this.

### 2.3 Detail where the eye is
- **Consistent texel density and geometric detail** for every material and object category at a given distance (set targets per category in `docs/design/budgets.md`). No object that's suddenly sharper, blurrier, more detailed or simpler than its neighbors.
- **Level of detail by perception:** detail follows distance, screen size, the center of view and motion. Transitions are invisible (dithered, temporally stable).
- **Contact and grounding:** everything rests on something, with contact shadows and occlusion where it touches. Nothing floats, sinks or clips.

### 2.4 Generation and simulation checked against Earth
- **Generation:** generated statistics are compared with real Earth data wherever possible: hypsometry, land fraction, river lengths and drainage, climate zones, species ranges, tree-size distributions, rock and soil distributions, coastline fractality. Test against these targets (several tests already exist; extend them at Earth scale per Part E §5).
- **Simulation:** real units and real rates, each with a source in data (`realism_source`) and uncertain values marked (v2 §3.1). Every simplification is documented with what it gets wrong and by roughly how much.

### 2.5 Animation
Animation is hard. Make it realistic anyway, with methods that scale:
- **Biomechanics first:**
  - Derive gait from speed and body size (dynamic similarity: Froude number; walk–run transitions; duty factors; stride length and frequency curves; quadruped gait changes from walk to trot to canter or gallop).
  - Lock feet to the ground and use IK on uneven terrain; keep balance and shift weight.
  - Add secondary motion (tails, ears, hair, fat, clothing) and breathing.
  - Match root motion to speed so nothing slides.
- **Real reference:** use motion capture only where its license allows inclusion in an open-source game (§4). Otherwise use procedural and physics-based animation tuned against measured real-world data (gait studies, timing measured from reference footage).
- **Interactions:** impacts, grabs and contacts (a spear striking, a hand gripping a branch, a body falling) use physics or physically guided blending, not canned poses that ignore the world.
- **Variation:** individuals of a species move slightly differently (size, gait timing, temperament). No two animals, or herd members, loop in sync.

### 2.6 Sound
Real recordings (licensed, §4) varied procedurally (pitch, timing, filtering, granular recombination) so nothing repeats identically. Synthesize wind, rain, water and fire where synthesis sounds better. Keep the physically based propagation already built (distance, occlusion, reverb, underwater).

---

## 3. Organic, not repeated: procedural first
Repetition makes a world feel artificial. **Prefer procedural generation everywhere**, with parameters drawn from real references.
- **No visible tiling** on any surface at any distance: stochastic texturing, macro variation, detail layers, and example-based synthesis from real exemplars (§4) instead of repeating texture tiles.
- **No identical instances in view:** every tree, shrub, rock, animal coat and body, log and stone is unique (its own seed and parameters). Shared meshes are allowed only under per-instance variation strong enough that copies can't be recognized.
- **No identical sound in close succession** for common sounds (footsteps, birdsong, impacts, rustling) without variation.
- **No synchronized motion** among things that would never move in sync in nature (grass waving in identical phase, herd members stepping together).
- **Automated checks** in the screenshot and soak suites:
  - autocorrelation/periodicity analysis of rendered terrain and vegetation regions, flagging periodic patterns
  - a check that visible instances never share identical generation parameters
  - an audio log check for repeated identical samples
  - an animation phase-spread check for groups

---

## 4. Real-world scans and reference data
Real scans and photographs of plants, animals, rocks, bark, soil and ground **may be used** where they clearly improve realism beyond what procedural generation can reach within budget, but on these terms:
- **Prefer them as exemplars, not stamps.** Use scans to drive procedural synthesis (example-based texture synthesis, measured color and roughness statistics, real shape distributions), producing endless non-repeating variation. Direct use is fine for close-up detail layers if variation is layered on top so copies aren't recognizable.
- **Licenses (non-negotiable):**
  - **Allowed:** CC0 or public domain (preferred), CC BY (attribution recorded).
  - **Never:** NonCommercial (NC) or NoDerivatives (ND), "editorial use only", unclear or unknown licenses, assets from other games, scraped images.
  - Check every individual asset's license, even from sites that are mostly CC0.
  - Examples to evaluate (verify each asset): Poly Haven and ambientCG (CC0 textures, scans and HDRIs), Smithsonian Open Access (CC0-designated items), CC0 or CC BY files on Wikimedia Commons, iNaturalist observations, and Freesound.
- **Provenance:** every asset has an entry in `ASSETS_LICENSES.md` (source URL, author, license, retrieval date, checksum, what it's used for, how it was processed). A lint fails any shipped asset without an entry.
- **Calibration, so nothing is out of place:** every scan goes through one pipeline: remove baked lighting (delighting), white-balance and calibrate albedo to the material reference ranges (§2.1), convert to the engine's PBR set, generate mips, GPU compression. Scanned and procedural materials must be indistinguishable in style and response to light.
- **Size:** keep exemplars compact (typically ≤ 1–2K per material set). Large source scans stay **out of git**: fetch them with a script using pinned URLs and checksums, and ship only processed, compact results. Set a total asset budget in `docs/design/budgets.md` (start with ≤ ~1.5 GB for the installed game) and track it.
- **No network in the build environment?** Write the fetch script and instructions for the owner to run. Never substitute unlicensed material.
- **Reference data** (measurements, distributions, color statistics, behavior studies) from reputable sources is encouraged everywhere, and cited in data.

---

## 5. Lean: no fluff
Fluff is anything that costs time, compute, memory, context or attention without making the game more real, more playable or more ready for its roadmap.

### 5.1 Code
- No speculative abstractions, and no "framework for later" code without a current user. (Part E's Actor rule, Part E §2.3, is fine because it's used now.)
- No second implementation of something the engine or a dependency already does. No unused modules, functions, feature flags, settings or debug paths in shipped builds (debug tools behind Developer mode only).
- **When something is superseded, remove the old code in the same milestone.**
- Prefer clear, direct code over cleverness. Split files that have grown past one clear responsibility. Several binary-crate files are 2,700–4,000 lines (`client.rs`, `server.rs`, `screenshot.rs`, `workshop.rs`); review them in Audit 0.

### 5.2 Data and content
- Every data entry is used and reachable (lint). No placeholder entries, no duplicates.
- `notes` fields are one or two plain sentences plus a source.
- Content that's `planned` stays minimal: an id, a name and one line, until it's built.

### 5.3 UI and settings
Minimal text on screen (Amendment P principle 2). No setting that doesn't matter to players (P principle 5). Remove options nobody needs.

### 5.4 Docs, logs and the files you re-read
- **Code comments** explain *why*, in plain, brief technical English. No ornamental prose.
- **Design docs:** one per system, concise and current. Replace outdated sections instead of appending to them.
- **Keep the restart files short.** They're re-read at the start of every session and cost context every time. Today `PROGRESS.md` is ~135 KB, `DECISIONS.md` ~226 KB and `PLAN.md` ~38 KB: about 400 KB, or roughly 100,000 tokens, read at every restart. Reduce them:
  - `PROGRESS.md`: current status only. A milestone status table, the current milestone, the next steps, open issues, the latest metrics. Target ≤ ~25 KB. Move older detail to `docs/history/progress-<period>.md`.
  - `DECISIONS.md`: an **index** of one-line decisions (D-number, date, one line, link). Target ≤ ~40 KB. Full texts go to `docs/decisions/D001-D099.md` and so on.
  - `PLAN.md`: remaining milestones in detail; completed ones as one line each. Target ≤ ~20 KB.
  - Update the resume protocol: read the short files, and open detail only when a task needs it.
- **Commit messages:** short and specific.

### 5.5 Tests
Tests check behavior and invariants that matter, without duplicates. Keep the default suite fast: set a CI time budget in `docs/design/budgets.md` and track it. Slow soak and long-run tests live in a separate suite run at audits.

### 5.6 Repository and assets
- No large binaries in git (Amendment R §1.1). `docs/review/` already holds ~5.7 MB of screenshots: keep only the latest review set, compressed (high-quality WebP or JPEG), and move older sets out of the repository (release assets or a fetchable archive).
- Generated files are rebuilt, not committed, unless they're small and needed for review.

### 5.7 Dependencies
Minimal. Remove unused ones (`cargo machete` or similar), avoid duplicate versions (`cargo tree -d`), and prefer the standard library and crates already in use.

### 5.8 Leftovers from the original-game-style beginnings (review in Audit 0)
Retire each as its replacement lands, unless something real still needs it:
- the pixel font and pixel-style UI (`hearth_ui` font and glyphs; §6)
- the Fast / Fancy / Fabulous preset names (rename to Low / Medium / High / Ultra)
- the vanilla-layout resource-pack loader and paths (`hearth_core` paths and options, `hearth_world`, `hearth_render` `models.rs`, `hearth_texgen`)
- the 16×16 texture pipeline and cube block-model baking, once Amendment S replaces them for natural terrain
- any setting or screen modelled on the original game that no longer serves this one

---

## 6. Visual design of the interface
A photoreal world with a pixel-font interface is out of place. Replace the original-game-style UI with a **clean, legible, restrained design** that suits the world:
- A real typeface under an open font license (OFL, such as a humanist sans for text and a matching serif or hand-style face for the journal), rendered as **SDF/MSDF text** so it's crisp at any resolution and GUI scale.
- Quiet panels in natural, low-saturation colors with good contrast; clear hierarchy; generous spacing. No ornament for its own sake.
- **Inventory and item icons rendered from the items' real meshes** under neutral light (generated and cached automatically), so icons match the objects in the world.
- The journal can be lightly **diegetic** (pages of a field notebook with the character's sketches), as long as it stays fast and legible.
- Accessibility (text size, contrast, colorblind-safe colors, P §6) and the automated layout test (P §4.1) still apply.
- Do it as a small milestone, **Q1**, right after Audit 0's findings are fixed. It touches only `hearth_ui` and the screens.

---

## 7. Fast: budgets and measurement
- **One budget table,** `docs/design/budgets.md`: CPU time per tick and per frame by system, GPU time by pass, memory and VRAM by category, disk and download size, world-creation and load times, build and test times. Every system has a line. Audits check them.
- **The performance gate** (`scripts/perf-gate.sh`, the 5% rule, D59) stays. Extend its scenes as content grows:
  - the Earth-scale planet
  - dense forest
  - grassland with blades
  - a storm
  - a camp at night with fire
  - multiplayer bot load (when Phase R arrives)
  - the most characters in view
- **Profile before optimizing.** Fix the top hotspots first and record the wins.
- **Optimization is part of building,** not a phase at the end. A feature isn't done until it meets its budget.

---

## 8. The checks

### 8.1 Every milestone: the five-test checklist
Add to each milestone's `PROGRESS.md` entry, one line per test:
- **Real?** What became more like Earth, and what reference it was checked against.
- **Lean?** What was removed or simplified; nothing unused added.
- **Fast?** Gate result; budgets met.
- **Whole?** Anything that might look or feel out of place, and how it was checked.
- **Organic?** Repetition checks passed.

Any "no" needs a fix before the milestone closes, or a `DECISIONS.md` entry explaining why not and when it will be fixed.

### 8.2 Full audits, at regular points
**When:** after every three completed milestones, and always at the end of a block of work. With the combined order of work at the top of this file, that means:
- **Audit 0** after E0
- **Audit 1** after P6 (covering E1–E6 and P3–P6)
- **Audit 2** after S4 (covering S1, S2, E7, S3, S4)
- **Audit 3** after P8 (covering S5 through S8)
- **Audit 4** after V2-14
- **Audit 5** after V2-16 (before Phase R)
- **Audit 6** after Phase R-A
- **Audit 7** before R10
- and every three milestones within Phase F

**What an audit does** (bound it to about 10% of the effort of the milestones it covers, so audits don't become fluff themselves):
1. **Metrics snapshot and trend** (§9), compared with the previous audit.
2. **Lean pass:**
   - `scripts/lean-check.sh` (§8.4).
   - A **fresh-eyes review:** re-read three to five modules and data files chosen at random (plus the fastest-growing crate) from disk, as if new to the project, judged only against this charter. List fluff, duplication and simplifications. If your tooling can run an independent reviewer with a fresh context, use one.
3. **Performance pass:** the gate, plus a profile of the worst scene with its top three hotspots.
4. **Realism and cohesion pass:** review the screenshot suite and short captured clips against the material reference ranges and real-world references. List anything:
   - out of place (scale, light, color, detail, style, motion, sound)
   - fake-looking
   - repeated
   - mis-timed
   - floating or clipping
5. **Repetition metrics** (§3).
6. **Fix list with priorities.** Fix the high-priority items before continuing; put the rest into `PLAN.md`.
7. **Report:** `docs/review/audits/AUDIT-<n>.md` (at most about two pages) and a five-line summary in `PROGRESS.md`.

### 8.3 Audit 0 (right after E0)
Because E0 removes so much, Audit 0 sets the baseline:
- the §9 metrics
- the restart files reduced (§5.4)
- a fluff inventory across the code, data and docs
- the original-game-era leftovers (§5.8) listed with a plan for each
- the material reference table started (§2.1)
- `budgets.md` created (§7)
- `scripts/lean-check.sh` written
Then **Q1** (§6).

### 8.4 Automated checks
**`scripts/lean-check.sh`**, run by `scripts/check.sh` in a fast mode and fully at audits. It reports, and fails on growth beyond thresholds without a justification:
- unused dependencies and duplicate versions
- count of `allow(dead_code)` / `allow(unused…)` (12 today)
- unused data entries and assets
- files over 2,000 lines
- sizes of the restart files
- repository size and the size of binaries in it
- build time and test time
- shipped assets without license entries

**Content lint additions:**
- materials within reference ranges (§2.1)
- every asset with a license entry (§4)
- every process with a work model and pose (Part E §7.6)

**Screenshot and soak suites:** the repetition checks (§3), contact and grounding checks where they can be automated (objects whose bounds don't touch their support), motion-timing checks (Amendment P §8).

---

## 9. Metrics to track (in each audit report)
- **Code and dependencies:** lines of Rust per crate (excluding tests); number of crates and dependencies; dead-code allowances; files over 2,000 lines.
- **Build:** clean build time; default and full test-suite times.
- **Size:** binary size and installed size; repository size; size of the restart files.
- **Data:** data entries implemented vs planned; unused entries (target 0).
- **Performance:** frame time p50 and p99 per benchmark scene, worst tick time, memory and VRAM.
- **Times and storage:** world-creation time and load time; save size per explored km².
- **Quality:** repetition scores per scene; realism and cohesion findings opened vs closed.

---

## 10. Resume protocol additions
Keep all earlier protocols, now reading the **short** restart files (§5.4). Also keep current: `docs/design/budgets.md`, `data/hearth/materials/reference.ron`, `ASSETS_LICENSES.md`, `docs/review/audits/`, and a **Quality** row in `PROGRESS.md` (last audit, next audit, open high-priority findings). On restart, run `scripts/lean-check.sh` (fast mode) with `scripts/check.sh`.

---

The bar for this charter: years from now, the project should be smaller than it would otherwise have been, faster than it needs to be, and still look and feel more like the real Earth with every milestone.

---

# PART E — Earth-True (Amendment E)
**Real time, real size, real work. Wild Earth first; simulated humanity later.**

The owner has been playing and has made several decisions that change the project's direction. This part is **Amendment E**. Read all of it before changing code.

In one paragraph: **everything runs on Earth's real clock and at Earth's real size**. Work is done stroke by stroke with the body, not with timers and progress bars. Things are gathered one at a time from what actually lies in the world. **Wild Earth** becomes the one playable era: you start as an adult you design yourself, at one of a few suggested places. **All simulated humans** (the people, hominins, births, childhood, conversation backend and history simulation) **are removed for now.** In their place, a much more ambitious **humanity framework** is **planned in detail now and built only after Phase R**. The technology tree and every non-human system stay and keep growing, fully usable in Wild Earth, in Creative and in multiplayer.

All earlier ground rules still apply unless this part changes them: clean-room, data-driven, deterministic, multiplayer-ready, always green, no stubs, performance budgets. **Part Q governs how all of it is done.**

---

## 0. Precedence, state and ordering

### 0.1 What this amends
- **Supersedes** V2.1 ("Realistic Humans") and its Addenda A and B in full, and v2 §8 (Australopithecus). Their goals move into the future framework (§10).
- **Supersedes** v2 §4.2 (two time scales), the calendar settings in v2 §4.1, and the planet-size, vertical-scale and feature-rarity choices in v1 §6 and v2 §16.
- **Modifies** Amendments P, S and R as listed in §9.
- Everything not mentioned stays.

### 0.2 Where the project is (as of commit `878b8d6`)
Done: M0–M3, V2-0–V2-12, H0–H10, S0, P0, P1. Next in `PLAN.md`: P2. `hearth_people` (~25,000 lines) and `hearth_ai` (~2,300 lines) hold the human systems this amendment removes.

### 0.3 Order of work from now on
**The authoritative order is the combined order of work at the top of this file.** It's this part's order with Part Q's audits and milestone Q1 added. Two notes for it:
- If the milestone you're finishing is P2, finish it, but don't build anything §2 removes (summoning people, inhabiting another person, being born again, people overlays in the Observer).
- H11–H13 are removed (§9.4); V2-13 and V2-14 continue as technology only.

Rewrite `PLAN.md` to that order, record it in `DECISIONS.md`, and keep `dev/PLAYTEST.md` tracking the owner's reports (§0.4).

### 0.4 The owner's reports this amendment answers
Add each to `dev/PLAYTEST.md`:
1. Ctrl and Alt can't be bound as keys of their own, only as part of a combination (§3.1).
2. Clicking with no target should use the item in hand or attack with it, or with a fist (§3.2).
3. Day and night, task timing, and the movement speeds of people and animals must match real life (§4).
4. Tasks shouldn't take a fixed time behind a progress bar. Gather things one at a time from what's really there, and do work stroke by stroke while holding the button (§7).
5. Waiting processes (cooking, drying) should be inspectable, showing what has happened so far (§7.5).
6. Days, nights, seasons, weather and the sky must run at Earth's real timing, and the world must be Earth's real size (§4, §5).
7. Wild Earth: start as an adult, choose from a few places with what to look for, no hominins; other eras "Coming soon" (§6).
8. Remove the talking options and the conversation backend for now (§2).
9. Bring back the character creator, and make hair and the whole body look realistic to match the smooth graphics (§6.2, §8).
10. Plan the future simulated-humanity framework in depth, but don't build it yet (§10).

---

## 1. Summary of changes

| Area | Now | After this amendment |
|---|---|---|
| Time | 48-minute days, 8-day seasons, two time scales | **Real time:** 24-hour days with Earth's real variation, a 365.2422-day year, real seasons, every process and motion at its real speed |
| World size | Planet-size presets (Standard recommended), scaled heights | **Earth's real size** (≈40,075 km around, real heights and depths); small test planets only in Developer mode |
| Work | Processes with fixed durations and outputs | **Stroke-by-stroke work** while the button is held, and gathering of real objects one at a time. Waiting processes follow real physics and can be inspected. No progress bars. |
| Start | Born to a family (Addendum A) | **Wild Earth: an adult you design**, at one of a few suggested places (or anywhere on the globe) |
| Eras | Several playable | **Wild Earth** only; the others shown as "Coming soon" |
| People | Simulated persons, hominins, births, childhood, culture, language, conversation, history | **Removed.** Planned in depth as Phase F (§10) |
| Death | Inhabit another person, be born again, spectate | **Begin a new life** (a new adult) or restart the world; spectating only in Creative |
| Controls | Ctrl and Alt only as modifiers; clicking at nothing does nothing | **Any key bindable alone**; clicking at nothing uses the item in hand or attacks |
| Bodies | Cuboid rigs (smooth ones planned in S6) | **Realistic human body, face and hair** for player characters (E7), animals and items in S6 |
| Technology | Eras 0–5 playable, V2-13/14 to come | Unchanged and continuing: all of it reachable in Wild Earth, Creative and multiplayer |

---

## 2. Remove the human systems (E0)

### 2.1 Archive first
1. Tag the last commit before removal `archive/humans-v2.1` and push the tag.
2. Move the V2.1 design docs (`docs/design/humans/`), the spec (`docs/spec/v2.1-realistic-humans.md` with Addenda A and B) and `MIGRATION_HUMANS.md` into `docs/archive/humans-v2.1/`, each with a "Superseded by Amendment E" banner.
3. Write `docs/archive/humans-v2.1/README.md`: what existed (crates, data, docs, tests), how to look at it (`git checkout archive/humans-v2.1`), and which parts Phase F may want to restore and adapt: the genetics engine, life tables, the language and culture generators, kinship, the deep-history simulation.
4. Nothing in the archive is compiled, tested or linted.

### 2.2 Remove from the build
- **Crates:** `hearth_people` and `hearth_ai`, and their dependents' uses (`hearth`, `hearth_protocol`).
- **Binary modules** (`crates/hearth/src/`):
  - Remove: `people.rs`, `born.rs`, `childhood.rs`, `conversation.rs`, `history_cli.rs`.
  - Replace `profiles.rs` (birth wishes) with character profiles (§6.2).
  - Repurpose `preview.rs` for the character creator's preview.
  - Reduce `eras.rs` to the era list (§6.4).
  - Reduce `observer.rs` and `observer_ui.rs` to what Creative's spectating needs (P §3.3): no cultures, peoples, chronicle of human history, or following persons.
- **Screens** (`menus.rs`): Births, Born, Birthplace (replaced by §6.3), WhoYouAre, Say, Conversation, and the human Chronicle.
- **Protocol:** the person, band, birth, speech-act and conversation messages in `hearth_protocol`.
- **Data:** `data/hearth/humans/`, `data/hearth/ai/`, `data/hearth/fauna/hominins.ron`; the deep-past and peoples parts of `data/hearth/eras/eras.ron` (keep names and one-line descriptions, §6.4); human strings in `data/hearth/lang/`.
- **Content schemas:** `hearth_content/src/schema/humans.rs`, plus the hominin and agent references in `schema/era.rs`, `schema/process.rs` and `schema/knowledge.rs`.
- **Knowledge discovery routes** that relied on watching hominins or being taught by people. Every knowledge node must stay reachable by a lone player through experimentation, observing nature, inference or found evidence. `hearth content lint` must prove it (§2.4).
- **Tests, benchmarks, screenshot shots and docs** that exist only for the removed systems.

### 2.3 Keep (and keep general)
- **The player's body and physiology** (`hearth_body`), carrying, clothing, crafting, knowledge, journal, building, fauna, flora, ecology, geology, weather, the globe, the spectate camera, and **domestication** (`hearth_fauna::herd`'s heritable `Breed` model is separate from human genetics and stays).
- **The multiplayer-ready rule** (Amendment R §0.3).
- **The Actor rule (new):** everything a player's body does is implemented as **mind-agnostic actions** behind one intent API (move, look, act with a hand, use, attack, gather, work, carry, sleep, emote). Player input is one "mind" that issues intents; animals already have minds of their own. Phase F's human minds will issue the same intents to the same bodies. Build P3, P4 and E6 this way. It's a design rule for code that's used now, not stub code.

### 2.4 Checks
- `hearth content lint`: no references to removed schemas or ids; every knowledge node reachable by a lone player; every process performable by a player.
- Saves: bump the format; worlds with people are refused with a clear message ("This world was made with an earlier version that had people in it; start a new world"), or migrated by dropping the people if that's simple.
- Clean build; fewer warnings than before; `PROGRESS.md` and `PLAN.md` say what was removed and why.

---

## 3. Controls (E2)

### 3.1 Any key on its own, including Ctrl and Alt
- **Cause:** the Controls screen only passes key *presses* to `RebindCapture` (`crates/hearth/src/app.rs`, ~line 445: `if pressed && run.menus.capture(...)`). `RebindCapture::release` exists in `hearth_input` but is never called, so a lone Ctrl or Alt waits for another key and becomes a combination. **Feed releases to the capture.** Confirm the fix with a test of the full press–release sequence through the menu.
- **Left and right modifiers** (LCtrl, RCtrl, LAlt, RAlt/AltGr, LShift, RShift, Super/Cmd/Option) are separate bindable keys. Combinations still work.
- **When a lone modifier and combinations with it are both bound:**
  - *Hold-type* actions on a lone modifier (sprint, crouch) start on press and stay active while combinations are used.
  - *Tap-type* actions on a lone modifier fire on release, and only if no combination was used while it was held. Generalize the existing `debug_chord_used` logic in `hearth_input/src/state.rs`.
  - The Controls screen explains overlaps instead of flagging them as conflicts.
- **Platform quirks:**
  - Windows: Alt must not open the window's system menu or steal focus, and AltGr (sent as Ctrl+Alt on some layouts) must be handled correctly.
  - macOS: Option and Command.
  - Leave OS-reserved shortcuts (Alt+Tab, Alt+F4, Cmd+Q) alone.
- Mouse side buttons and every controller button are bindable too.

### 3.2 Clicking with no target: use or attack
This replaces Amendment P §5.2's "if there's no sensible use, nothing happens" for the case of **no highlighted target**:
- Each item has a data-defined **primary use**.
  - With a use: food → eat (bite by bite, §7.2), water container → drink (sip by sip), bandage → apply, torch → hold it up or wave it.
  - Usable as a weapon: spear → thrust, club or axe → swing, knife → slash or stab, a stone in hand → strike, bow → draw on hold and loose on release.
  - Neither: strike with the fist that holds it.
- **Empty hand → punch.** A separate **kick** action gets its own key (choose a default that doesn't conflict, and document it).
- **Attacks are physical:** a real wind-up and strike animation, stamina cost, reach from arm and weapon length, hit detection along the swing against body hitboxes, damage by weapon type (blunt, cutting, piercing) and force, wounds through the existing animal wound model, knockback, realistic recovery time. Misses are possible.
- With a highlighted target, P's intent resolver still decides (attacking an animal with a spear when looking at it, and so on).
- In multiplayer, attacks on other players follow the PvP setting (Amendment R §3.6).

---

## 4. Real Earth time (E3)

### 4.1 One clock, Earth's
- **Remove the two time scales** (v2 §4.2): `hearth_content::time`'s compression, `day_length_min` and `days_per_season` in `data/hearth/time.ron`, and their settings. Rewrite `docs/design/time-scales.md` as `docs/design/time.md`.
- **One game second is one real second** for everything: bodies, work, fire, food, weather, plants, animals, the sky, water, rivers, seasons, life cycles.
- **The calendar is Earth's:** a mean solar day of 86,400 s; the tropical year of 365.2422 days with leap years; the synodic month of 29.530589 days; axial tilt 23.44°.
- **The sky follows real astronomy:** the sun's position by latitude, date and local time, **including the equation of time** (solar noon drifting by up to about ±16 minutes through the year); real twilight lengths; polar day and night; moon phases and moonrise times; the stars turning once per sidereal day.
- **Tides** (if implemented) at their real periods (about 12.42 h for the main lunar tide).
- **Remove** the day-length and season-length settings. Keep a **starting date and time** at world creation:
  - "Spring morning" (default): the morning of a spring day in the chosen hemisphere.
  - "Now": the real current date and time.

### 4.2 Everything at its real speed
Audit and correct, with sources recorded in data:
- **Movement:**
  - Human walking (~1.3–1.5 m/s), jogging, running and sprinting speeds, with acceleration, endurance and carrying effects.
  - Every animal's gaits, top speeds and endurance (a wolf's trot it can keep up for hours, a deer's sprint, a bear's charge).
  - Animation playback matched so feet never slide.
- **Weather:** synoptic systems lasting days, fronts passing over hours, daily cycles (morning fog burning off, afternoon thunderstorms), monsoon seasons, real cloud motion and evolution, real rain and snow fall speeds. Amendment P §8 already set real-speed motion for clouds, water and stars; now the sun and moon are real too.
- **Body:** metabolism, thirst, sleep need (about 7–9 hours a night), healing (cuts in days, fractures in weeks) at real rates. Easy mode's leniency (P §2) changes amounts and severity, not the clock.
- **Ecology:** daily activity cycles at real hours (dawn and dusk busy), real migrations, breeding seasons and growth.
- **Plants:** real growth and phenology through the real year.

### 4.3 Passing time
- **Single-player:**
  - **Sleep** (P §7.1) and **Rest / Wait** (sit or lie and wait: "until morning", "until dusk", "for one / two / four hours", "until the meat is dry") speed time up smoothly, up to the existing `sleep.max_factor`.
  - The world keeps simulating at the faster rate, and anything that needs the player interrupts.
  - This is the only way time is skipped. Active work is never skipped (§7).
- **Creative:** the time speed control (P §3.1) is the way to watch long processes and growth.
- **Multiplayer:** the sleep policy (Amendment R §3.5) governs speeding up time.
- **Optional, after the rest works:** a world setting **"The world goes on while I'm away"**.
  - When enabled, loading a single-player world catches it up by the real time since it was last saved, using the abstract simulation tiers: seasons turn, food spoils, fires go out, crops grow.
  - The character is safe and their needs are paused while away.
  - Implement it only if the catch-up stays within a loading-time budget (≤ ~15 s for a month away). Otherwise record why in `DECISIONS.md`.

### 4.4 Consequences to state honestly
Some things now take real years: growing trees, domesticating animals and crops by selective breeding across generations, growing up. They stay reachable in long-running multiplayer worlds, with "the world goes on while I'm away", and in Creative. The guide and the Field Guide (P §9.3) must say so plainly.

---

## 5. Real Earth size (E4)

### 5.1 The planet is Earth-sized
- **One planet size:** the existing `PlanetSize::Earth` (circumference 40,075,264 m, a multiple of 4,096; mean radius 6,371 km), with **real vertical scale** (1 block = 1 m up and down).
- Remove the planet-size, vertical-scale, feature-rarity and land-fraction choices from world creation. Earth's own values apply: about 29% land, Earth's hypsometry (mean land elevation ~840 m, mean ocean depth ~3,700 m), the highest peaks around 8–9 km and the deepest trenches around 11 km, both rare.
- **Developer mode** (Settings → Advanced) keeps the small test planets (Tiny, Small, Standard) for tests, bots and benchmarks.

### 5.2 Recalibrate the generator at Earth's scale
The generator was tuned for the Standard planet (65,536 m around, about 610× smaller). At Earth's scale:
- **Plates and continents:**
  - About fifteen major plates plus minor ones.
  - Continents thousands of kilometres across, oceans of real widths.
  - Mountain ranges at real scales (a great range ~2,000–3,000 km long and a few hundred km wide), real shelves (tens to hundreds of km wide) and slopes.
- **Multi-resolution terrain synthesis:**
  - The planet grid (2048² Mercator, ~20 km per cell at the equator on Earth) can't carry the detail between 20 km and 1 m by itself. Add **nested, deterministic, erosion-aware refinement levels** (for example ~20 km → ~2.5 km → ~300 m → ~40 m → 1 m).
  - Each level is generated per tile with overlapping borders and conditioned on the level above: uplift, rock and drainage.
  - Valleys, ridges, cirques, river networks, lakes and coastlines then exist at every scale and agree with each other. Coastlines are fractal at every scale; plains really are flat over kilometres.
  - Cache by tile with LRU. Results must be identical whichever tile computes them.
- **Rivers:** major rivers come from the planet grid's drainage (the longest thousands of km), and tributaries are generated at finer levels, consistent with it and never flowing uphill.
- **Climate and life at real heights:**
  - Lapse rate about 6.5 °C per km.
  - Snow lines and tree lines at real elevations (e.g., tree line ~2,000–2,300 m in temperate mountains, ~3,500–4,000 m in the tropics).
  - Clouds at real heights (cumulus bases ~0.5–2 km, cirrus ~6–12 km).
  - Ocean light fading through the photic zone (~200 m) to darkness.
- **Altitude physiology** (new, now that heights are real):
  - Hypoxia reduces stamina from about 1,500–2,500 m.
  - Altitude sickness becomes possible above about 2,500–3,000 m, with acclimatization over days.
  - Severe effects above about 5,000 m, and the "death zone" above about 8,000 m.
  - Water boils at a lower temperature with height (about 1 °C less per ~300 m), so cooking by boiling takes longer up high.
- **Re-run the M2 and V2 worldgen tests at Earth's scale**, update the statistical targets to Earth's real ones, and add tests for the new refinement levels (seam-free tiles, drainage consistency across levels, determinism).

### 5.3 Simulation at Earth's scale
- Everything planet-wide must be **lazy**. Ecological regions (64 × 64 cells of 256 m) and any other per-area state exist only where players are or have been. Elsewhere, state comes from the undisturbed default for that climate and season, plus deterministic history since then.
- A coarse global layer (e.g., cells of tens of km) carries the slow, large things: animal migrations and big weather systems.
- **Saves** store only what players changed, plus the small global data.
- Spawn search and suggested places (§6.3) work over the whole planet.

### 5.4 Seeing to the horizon
- **Real curvature** (radius 6,371 km). From standing height the horizon over flat land or sea is only about 5 km away; mountains beyond show above it; from a high summit you can see a few hundred kilometres.
- Add a **far-field planet layer**: render the planet grid and its coarser refinement levels as curved terrain beyond the LOD system's range, out to the real horizon, shaded with the same materials, sward colors and atmosphere, so distant coastlines and ranges appear where they should. Amendment S §5's smooth LOD then blends into it (§9.2).
- Performance targets stay those of v1 §8.4 and Amendment S §12, measured on the Earth-sized planet.

---

## 6. Wild Earth: the start (E5)

### 6.1 Who you are
You arrive in Wild Earth as an **adult** in a loincloth (v2 §9.1), designed in the character creator. There are no other humans in Wild Earth: no hominins, no wandering families, no people of any kind except players.

### 6.2 The character creator (back)
- **Body:** male or female, plus height and build within the normal adult range (cosmetic; no gameplay effect).
- **Skin:** a continuous tone across the natural human range, undertone, freckles.
- **Face:** shape presets, then sliders (jaw, cheekbones, brow, nose, eyes, lips, ears).
- **Eyes:** brown, dark brown, hazel, amber, green, blue, grey.
- **Hair:** style (short crop, buzzed, shoulder-length, long straight, long wavy, curly, coily, braids, locs, tied back, bald), length, natural colors (black through brown, auburn, red, blonde, grey and white) and a fine picker. Facial hair for any body, and eyebrows.
- **Also:** name, **Randomize** (sampling natural human variation, with pigmentation traits correlated as they are in reality), saved profiles.
- **Live preview** turning under several lights (sun, overcast, firelight, night). It uses the current body model until E7 (§8) replaces it, and that work then updates the creator automatically.
- **Appearance has no effect on gameplay.**
- Store appearance as appearance (`hearth_character`). Phase F can later derive a genome from it.

### 6.3 Where you start: a few suggested places
After the planet is generated, the start screen shows **3–5 suggested places** on the globe, chosen for variety (different climates and landscapes) and for being survivable for someone in a loincloth **at the chosen starting date**. Each place has a card:
- Its name in plain words ("A chalk coast in a mild, wet climate"), climate and season now, terrain, and a difficulty label (Gentle, Challenging, Harsh).
- **What to look for**, concrete and true for that spot: where water is ("a spring at the head of the valley"), where toolstone is ("flint in the chalk cliffs and on the beach below them"), wood and fibre ("hazel and willow along the stream"), food in this season ("blackberries along the woodland edge; mussels on the rocks at low tide"), fire ("dry deadwood under the oaks; tinder fungus on old birches"), shelter ("an overhang in the limestone above the river").
- **Watch out for:** predators present, cold nights, venomous snakes, crocodiles at the river, the tide on the mudflats, and so on.
- **Every claim is verified** by sampling the generated world near the spot before it's shown. A suggestion that can't be verified is replaced.
- **Choose anywhere** (a secondary button) opens the full globe picker with the same information on hover.
- **Multiplayer:** also **Near a friend**, if the friend allows it.

### 6.4 Eras
The era selector lists Wild Earth (playable) and the others (Lower, Middle and Upper Paleolithic, Neolithic, Bronze Age, Iron Age / Classical, and later) as **Coming soon**: greyed, each with its one-line description. Wild Earth's description changes to "A wild Earth without people. Survive, learn and build from nothing."

### 6.5 An opening that's alive
A new life begins at dawn (or at the starting time chosen): the camera fades in on the character lying in the grass at the chosen place, birds singing, the light coming up. In Easy, the first-time hints (P §9.2) start. Nothing else is staged: the world is simply there.

### 6.6 Death and starting again
- **Realistic and Easy:**
  - **Begin a new life:** a new adult (choose a profile or design one) arrives at one of the suggested places, or "near where I last lived". The previous character's body and belongings stay where they fell, and their camp stays where it was.
  - **Restart the world.**
  - **Knowledge:** in Realistic the new person knows nothing (the old journal stays readable as notes from a past life, P §2). In Easy discoveries carry over.
- **Creative:** you can't die.
- **Multiplayer:** "Begin a new life" adds "Near a friend".
- Replace the death screen and its settings accordingly. Spectating stays Creative-only (P §2).

---

## 7. Work the way the body does it (E6, together with P4)

### 7.1 No timers, no progress bars
- Remove fixed durations and progress bars from **active work**: the 187 processes in `data/hearth/processes/paleolithic.ron` and `neolithic.ron` each have a fixed `duration: (hours: …)` and random output amounts (e.g., `gather_stones` gives 2–3 cobbles from a `_cobbles` block in 0.02 h).
- Also remove P §5.4's progress ring and P §6.2's "hold Space to skip ahead" for active work.
- **Progress is what you see in the world:** the pit getting deeper, the notch in the trunk widening, the pile growing, the hide getting cleaner. Plus the body's animation.

### 7.2 Active work is stroke by stroke
- **Holding the hand's button works; releasing stops.** Each loop of the work animation is one real unit of work at its real tempo, with a real effect. For example:
  - **Digging soil by hand:** a scoop of about 0.5–1 L, depending on the soil.
  - **Digging stick:** loosens soil. A shovel (later) moves a spadeful.
  - **Axe:** each blow cuts away wood, depending on edge sharpness, wood hardness, the angle and the character's skill. The tree falls when the cut really is deep enough (v2 §6.2).
  - **Scraping a hide:** each stroke cleans an area.
  - **Whittling:** each cut shapes the shaft.
  - **Grinding on a quern:** each stroke grinds a little grain.
  - **Weaving and sewing:** each loop adds a row or a run of stitches.
  - **Fire by friction:** strokes build heat in the hearth board until an ember forms (or doesn't).
  - **Knapping** keeps its strike-by-strike minigame.
- **Each process's data** defines its work model: animation loop and tempo, effect per loop (volume, area, length or mass), energy cost (feeding the body's metabolism), skill effects (efficiency, quality, fewer mistakes), tool effects, and realistic failure modes.
- **Rates come from real sources** (experimental archaeology, ergonomics and craft literature: felling with stone and metal axes, digging rates by soil and tool, hide-working times, quern grinding rates, fire-by-friction times). Record each source and mark uncertain values (v2 §3.1). Skill makes a beginner slower and clumsier, as it should.
- **Work in progress is a real, persistent state** of the thing being worked: a half-dug pit, a half-cut tree, a half-scraped hide, a half-carved bowl, a half-woven basket. You can stop, put it down, come back later. Where it's realistic it changes while you're away: rain fills the pit, an unfinished hide dries stiff.
- **Accessibility:** a setting **"Toggle work instead of holding"** (click to start, click to stop) for players who can't hold a button for long.

### 7.3 Gathering one thing at a time
Things to gather **exist physically in the world** and are picked up **one at a time** (or one handful, for things people pick by the handful). Holding the button keeps picking the nearest of the same kind within reach. There are no generic "gather" processes producing random amounts.
- **A natural loose-objects layer**, generated deterministically like the grass sward (P §11.5): undisturbed defaults are computed, and only picked or moved objects are stored. Placement follows Amendment P §11 (off-grid, natural distributions):
  - fallen twigs, sticks and branches under trees (by species, forest age, recent storms)
  - acorns, beechnuts, hazelnuts, chestnuts, walnuts, pine cones and fallen fruit in their season, including **mast years** when oaks and beeches drop huge crops together
  - leaf litter
  - stones by geology and landform (river cobbles sorted by size downstream, beach shingle, scree, glacial erratics, flint nodules weathering out of chalk at cliff feet and in streams)
  - shells on beaches, driftwood after floods and storms
  - feathers, shed antlers in late winter and spring, bones from the ecology's deaths
  - dung, which dries into fuel
  - resin beads on wounded conifers, curls of fallen birch bark
- **Ecology competes with you:** jays and squirrels cache acorns, boars root for them, mice eat seeds, and scavengers take bones. What's there changes over the year.
- **Harvesting from living plants** is also incremental and physical:
  - snap a branch thin enough to break by hand (thicker ones need a tool)
  - strip a twig or leaves
  - score and peel a strip of bark
  - pick a cluster of berries
  - shake a hazel to drop its nuts
  - pull a handful of grass (the sward, P §11.5)
  - pull reeds one by one
  - dig a root or tuber with a stick
  - pick mushrooms
  - tap a birch for sap in spring (a waiting process)
- **Each object has its real mass and size** (v2 §10) and goes into hands or containers as before.

### 7.4 Waiting processes run on real physics
Cooking, drying, smoking, salting, fermenting, spoiling, tanning, retting, seasoning wood, drying and firing clay, smelting, burning lime, bread rising, cheese ageing and the rest run by **real-rate physical state models** driven by the existing thermal model and the weather:
- **Cooking:** heat flowing into food raises its core temperature, setting doneness and charring.
- **Drying:** moisture content falls with temperature, humidity, airflow, sun or shade, and thickness.
- **Spoilage and fermentation:** microbial growth by temperature and moisture.
- **Clay:** dries from leather-hard to bone-dry over days, then is fired along a temperature curve. Clay heated too fast while still wet cracks.
- **Smelting:** reduction by temperature and charcoal-to-ore ratio.
Rates come from real sources, and failures are realistic: burned, moldy, cracked, under-fired.

### 7.5 Inspecting a waiting process
"Look closely" (the action menu on the meat over the fire, the hide on its frame, the pot in the kiln) shows a short **description of what has happened so far and how it looks now**, in the character's own understanding:
- It describes what's perceptible: color, smell, texture, sounds.
- It tells what the character knows to expect, if they have the knowledge ("About as long again would cook it through").
- It includes a brief history of what affected it ("You hung the strips at midday. The night was damp and the thick middles are still soft; the thin ends have dried leathery. Flies have been at the lowest strip.").
- A beginner sees only signs; someone skilled can judge. Creative and Developer mode also show the numbers.

### 7.6 Data and lint
- Convert every process to a **work model** (active) or a **state model** (waiting).
- Gathering processes become loose-object and harvest definitions.
- `hearth content lint` fails on any active process without a work model and pose, any waiting process without a state model and inspection descriptions, and any gatherable without a source in the world.

---

## 8. Realistic human body, face and hair (E7, after S2)

The player characters are the only humans for now, and the camera is often close to them, so they deserve high quality. This replaces the human part of Amendment S §10 (S6 keeps animals and items).
- **Body:**
  - Anatomically realistic adult proportions and surface forms (shoulders, clavicles, ribcage, musculature and fat by the build slider, hands with articulated fingers, real feet), as smooth skinned meshes on the existing skeleton.
  - Morph targets for body shape and the face.
  - Breathing and weight shifts.
  - Barefoot contact with foot IK on uneven ground.
- **Skin:**
  - Subsurface scattering (screen-space or pre-integrated), two-lobe specular.
  - Detail normals (pores; creases at joints; finer lines by age).
  - Tone from the creator's continuous parameters, freckles.
  - **State from the body simulation:** wet skin, mud and dust on legs and hands, blood from wounds, **scars** where healed injuries were, goosebumps and pallor in cold, flush in heat and exertion, **tan and sunburn over real days** of sun exposure.
- **Face and eyes:**
  - Realistic structure from the creator.
  - Eyes with refraction or parallax for the iris, a wet specular highlight, blinking, saccades, eyelids following gaze.
  - Teeth and tongue.
  - **Expressions** driven by the body (pain, cold, exertion, exhaustion, fear when an animal charges) and, in multiplayer, by emotes.
  - **Lip sync** with proximity voice (Amendment R §5.1).
- **Hair:**
  - **Card-based hair** (many layered strips with strand textures, alpha-to-coverage) with **anisotropic, Marschner-style shading** (two highlights), translucency when backlit, and self-shadowing.
  - Procedurally built for every creator style (straight, wavy, curly, coily, braids, locs, buzzed).
  - Eyebrows, eyelashes, facial hair and fine body hair.
  - **Physics:** guide strands driving the cards (sway with movement and wind, settling, wet hair clumping and darkening).
  - **Growth in real time:** about 1 cm a month on the head, stubble within days for facial hair. Hair can be cut or tied back with a sharp edge or a cord (an action).
- **The loincloth and later clothing** (v2 §10.3) as fitted, layered meshes with secondary motion or light cloth simulation.
- **First person:** realistic arms and hands do the work (P §6, §7). Looking down shows the body.
- **Animation quality:** blended or motion-matched locomotion, no foot sliding, natural work loops (§7).
- **Budgets:** player characters are few (up to 32 in multiplayer). At High, a close-up character within about 1 ms of GPU, with LODs from about 10 m and cheap impostors far away. Meet the Amendment S §12 targets with characters in view.
- **Review:** screenshots of varied characters from the creator under sun, overcast, firelight and night; wet, muddy and sunburned states; hair styles in wind; close-ups of faces and hands. Add them to `docs/review/smooth-world.md`.

---

## 9. Changes to Amendments P, S and R and to v2

### 9.1 Amendment P
- **§2 modes:**
  - The "After death" rows follow §6.6 (no inhabiting, no being born again).
  - The "Start" row: all modes start as an adult via §6 (Creative too).
  - Clocks and dates show real time.
- **§3.1 Creative:** remove People from the creative inventory. Time speed stays (the way to watch long processes).
- **§4.3 Create World:** name, seed, mode, era (Wild Earth only), starting date; then the suggested places (§6.3); then the character creator (§6.2). Remove planet size, vertical scale, day length and season length.
- **§5.2 hands:** add §3.2 (clicking with no target). Hold-to-repeat becomes stroke-by-stroke work (§7.2).
- **§5.4 feedback:** remove the progress ring. Keep only very brief result feedback (e.g., the gathered thing flying to hand or bag), no text spam.
- **§6.2 skip:** replaced. No skipping active work; Rest / Wait (§4.3) for waiting processes.
- **§7 sleep:** keep the two-process model, now on real 24-hour days.
- **§8 motion:** the sun and moon move at real speed too; everything is 1:1.
- **§9.1 tutorial:**
  - Remove the "People" chapter.
  - Teach stroke-by-stroke work, gathering one thing at a time, inspecting the meat over the fire, and Rest / Wait.
  - Set it in a Wild Earth valley at real timing.
- **§11 and §11.5:** calibrate at Earth's scale (§5). Regrowth and seasons on the real clock. The loose-objects layer (§7.3) belongs with P7.

### 9.2 Amendment S
- **§5 distant terrain:** blend the smooth LOD into the far-field planet layer (§5.4) out to the real horizon.
- **§10 bodies:** the human part moves to E7 (§8), right after S2. S6 covers animals and items.
- **§12 benchmarks:** measured on the Earth-sized planet.

### 9.3 Amendment R (`dev/AMENDMENT_R.md`)
- **R3:**
  - Multiplayer starts and new lives use §6.3 and §6.6 (suggested places, anywhere, near a friend). Addendum B's births in multiplayer are removed.
  - Add **player-to-player body interactions** built on the Actor rule (§2.3): wave, point, beckon, handshake, hug, pat on the back, help up, carry an injured friend, shove, punch, kick, tackle, grapple.
  - Contact interactions need the other player's **consent**: a prompt, or a per-player "allow contact" setting. Hostile ones follow PvP.
  - Kissing between consenting adult players is allowed as a mutual, opt-in interaction.
  - These animations and mechanics are the ones Phase F's people will use.
- **R4 (AI Bridge) and R6 (agent voices)** move into Phase F (§10). Phase R-A becomes **R1 → R2 → R3 → R5**; Phase R-B is unchanged.
- **R5** proximity voice stays (now with lip sync, §8).
- **§9 guide:** sections on people, eras and AI become "Coming later".
- **§10 trailer:** the "people" beat becomes players together (building, voice, a shared fire). No simulated humans appear.

### 9.4 v2 milestones
- **H11, H12 and H13 are removed** (their goals are in Phase F).
- **V2-13 and V2-14 continue as technology:** metallurgy and mining, then Iron Age and Classical tech, usable by players alone, in Creative and in multiplayer.
- **V2-15** follows §6 and P §4.
- **V2-16** runs its long-run QA at Earth scale and real time.

---

## 10. The future humanity framework (E1: design now, build in Phase F after R10)

**Write this design into `docs/design/future/humanity/` now** (one document per section below, plus `README.md` and `roadmap.md`), refine it with research, and add the Phase F milestones (§10.13) to the end of `PLAN.md`. **Write no code for it now.** Rewrite `docs/design/future-humanity.md` to point here.

The goal: **people who behave like real people of their era, individually and as societies, across millions of lives and thousands of years of history, in a world that runs at Earth's real time and size.** True human-level general intelligence per person isn't available. The framework instead aims for **human behavior wherever anyone is looking**, with the depth of thought scaled to attention, and with every part swappable as models improve.

### 10.1 Principle: simulate everything, think where it matters, narrate what is seen
Three loops at different speeds:
1. **The society loop** (always on, the whole planet, coarse): demography, economy, politics, culture, technology, conflict, as fast numeric and agent-based models. No language model per person.
2. **The people loop** (individuals near players and notable people): needs, plans, relationships and memories, with procedural minds and periodic language-model reflection.
3. **The presence loop** (moment to moment around players): a fast action model, the body's motor control and real-time voice for the people actually face to face with players.
**Ground truth is the simulation.** Language models propose: what someone wants, says, believes and decides. Deterministic systems dispose: what can physically happen, what is true, what is recorded.

### 10.2 Worldbuilding at the scale of a long saga
- **The History Engine** (numeric, event-driven) runs from the first people, or the era's starting conditions, to the chosen date, on a hierarchical region graph built from the planet (settlements as nodes, routes as edges). It models:
  - **Demography:** age-structured populations, fertility and mortality by way of life, health and nutrition.
  - **Subsistence:** carrying capacity from the real ecology and biomes.
  - **Technology:** diffusion and innovation on the existing knowledge graph, faster in large connected populations, with losses in isolation.
  - **Settlements:** where they form (water, soil, defense, resources), how they grow, central places, roads along least-cost paths.
  - **Economy:** production, trade along routes (gravity-model flows), specialization, markets, surplus and inequality.
  - **Polities:** emergence and collapse (agent-based polity models on the region graph), alliances, succession, dynastic cycles, structural-demographic pressure.
  - **Conflict:** raids and wars over land, water, routes and succession, resolved by terrain, technology, numbers and logistics.
  - **Culture and religion:** trait-based cultural evolution (drift, selection, diffusion, conversion), language phylogenies (sound change and word replacement).
  - **Shocks:** epidemics with density thresholds, droughts, volcanic winters.
  - **Output:** an **event store** (every birth of a notable person, founding, war, migration, invention), periodic **snapshots**, and **notable individuals** wherever the models produce leaders, founders, prophets, generals or inventors.
- **The Historian** (language models, in batch at world creation, then lazily):
  - It turns events into the **World Chronicle**: names from the generated languages, annals of each polity, myths and origin stories, epics of great wars, laws and sacred texts (short), genealogies, songs and proverbs, descriptions of customs, dress and architecture, the labels on maps.
  - It writes **several in-world perspectives** (each people's own version of a war) as well as the facts. Perspective is where invention belongs; facts are fixed.
  - **Consistency is enforced:** every narrative claim cites event ids, every extracted fact is checked against the event store (dates, places, participants, who was alive), and contradictions are rejected or repaired.
  - Each culture gets a style guide so its voice stays distinct.
- **The World Bible:** a versioned knowledge graph (entities, relations, events with time spans), the chronicle documents and an embedding index, per world. Game systems and minds query it, and each mind sees only what its person could know.
- **Lazy zoom:** detail is generated on demand from coarse to fine, always consistent with the level above: world (at creation) → realm → polity → region → settlement → household → person.
  - Approaching a region, or choosing a birth there, makes the engine synthesize its households, buildings, families and recent history, first as numbers and then as narrative: life stories, local legends, current gossip.
  - Generation is deterministic from the seed and cached. In multiplayer, the server generates it and shares it.
- **Scale:** a macro chronicle of a few hundred thousand words at creation (configurable, budgeted, with a quick option), growing lazily into millions of words as the world is lived in.

### 10.3 People: one person model, thought scaled by attention
- **One Person record** for everyone, at every level: identity, appearance and genome (genetics restored from the archive and generalized), body state, household and kin, role and status, traits and values, skills and knowledge (knowledge-graph references), relationships (the closest few in detail plus aggregates), possessions, life-history events, beliefs (world-bible references plus private beliefs, possibly false), goals and memories in summary.
- **Cognitive levels of detail:**

| Level | Who | How they think | Rough scale |
|---|---|---|---|
| **C0 statistical** | Everyone, as populations | Demographic and economic models | Billions over history; millions alive |
| **C1 sketch** | People instantiated in active regions | Records plus culture- and role-based routines, resolved statistically | 10⁵–10⁶ per active region |
| **C2 procedural mind** | People near players | Needs, emotions, a deterministic planner, routines, social rules; no language model per tick | ~10³ |
| **C3 reflective mind** | Notable people, the player's kin, people whose day matters | C2 plus periodic slow thinking by a small, fast language model | ~10² |
| **C4 conversational mind** | People face to face with a player | C3 plus real-time conversation and voice with a stronger model | a handful at once |

- An **attention allocator** assigns levels every second under compute budgets, from proximity to players, interaction, ties to players, narrative importance and unfolding events, with hysteresis.
- **Demotion** consolidates memory into the record (a short reflection written by slow thinking). **Promotion** rehydrates it (retrieves goals, recent memories, mood).

### 10.4 A mind: System 1, System 2 and the body
- **The body (Actor):** the same body, action vocabulary, physics and animation players use (§2.3, §8, Amendment R §9.3). It executes motor commands:
  - locomotion along planned paths
  - manipulation (gathering, work and crafting through the process engine)
  - social and combat actions (greet, wave, point, handshake, hug, kiss, comfort, carry, push, punch, kick, tackle, grapple)
  - expressions (face, posture, gaze)
  - speech playback with lip sync
- **System 1 (fast, reactive, about 5–20 decisions a second):** chooses the next micro-action from the situation and the current plan step: keep walking, step aside, look at the speaker, flinch, return a greeting, block a blow, follow, comfort a crying child.
  - Recommended: a **small learned action model** (a compact transformer over a tokenized situation: own state, nearby people and things, plan step, emotion) that outputs action tokens from the vocabulary. It runs batched on the GPU for hundreds of people.
  - It's trained by imitation of designed behaviors, of rollouts annotated by System 2, and of human motion and social data.
  - Deterministic utility and behavior rules are the always-available fallback and safety net.
- **System 2 (slow, deliberate, event-driven, seconds to hours):** a language model (local small or mid-size, or a user-configured hosted one) that keeps the person's **goals, plans, beliefs, relationships and memories**:
  - plans the day by role and need
  - reacts to salient events (a death, an insult, a proposal, a rumor of war)
  - decides what to say and to whom
  - makes big decisions (move away, marry, quarrel, steal, build, go to war)
  - reflects at night (memory consolidation)
  - Its outputs are **structured** (schemas): goals, commitments, dialogue intents and lines, belief updates, emotional appraisals. They're validated against the person's knowledge and the world's facts.
- **The planner (deterministic, between the two):** a hierarchical task planner over the game's real process, construction and movement models turns System 2's goals into executable steps for System 1 and the body. **Language models choose what to do; planners work out how**, so plans are always physically valid.
  - Each culture and era has a library of plans ("how we drive bison over the bluff", "how a feast is held"), and successful new plans are added to the library (as skill libraries do in agent research).
- **Memory:**
  - an episodic stream (observations and events with importance scores)
  - retrieval by recency, importance and relevance
  - nightly reflection into higher-level beliefs ("my brother can't be trusted with the herd")
  - forgetting, and memory budgets by level
  - shared cultural memory (myths, songs, law) held once in the world bible and referenced, not copied
  - Beliefs can be false. People can lie and be deceived, but **words never change world facts**.
- **Personality and emotion:** heritable temperament shaped by upbringing (restored genetics, with ground rule 1 of §10.10). Emotions come from appraisals, computed deterministically and fed into both systems, the face and the voice.
- **Study the prior work** while designing (look these up and verify details):
  - generative agents with memory streams and reflection (Park et al., 2023), and generative-agent simulations of about 1,000 real-person-based agents (Park et al., 2024)
  - many-agent civilizations in a block world (Project Sid, Altera, 2024)
  - skill libraries for embodied agents (Voyager, Wang et al., 2023)
  - large-scale LLM social simulation (e.g., AgentSociety, 2025)
  - agent-based models of polity formation (e.g., Turchin et al., 2013)
  - hierarchical task network planning
  - crowd simulation (flow fields, ORCA)

### 10.5 Talking: real-time conversation and voice
- **Turn-taking** (System 1): when to speak, listen, interrupt, keep silent, or murmur agreement. Conversations in groups with players and other people, and recognizing who is being addressed.
- **What to say** (C4's model): streamed, conditioned on the person's character, memories, beliefs, relationships, mood, culture, era vocabulary and language; filtered for knowledge and safety. **Silence is always an option.**
- **Voice:**
  - streaming speech recognition for players, and streaming speech synthesis with a stable voice per person (from age, sex, body and inherited timbre), emotional prosody, accents by culture, whispering and shouting
  - lip sync and facial animation
  - first audio in about 0.5–0.8 s, with natural filler behavior (a look, a breath, "hm") covering the wait
- **Languages:** the generated languages return.
  - *Authentic:* people speak their own language, with lines rendered through the language's lexicon and grammar and spoken by phoneme synthesis, and subtitles translated to the player's understanding.
  - *Understood:* people speak the player's language with an accent, for accessibility.
  - The player learns languages by living among speakers.
- Amendment R's AI Bridge (provider adapters, the Agent Bridge protocol, budgets, privacy) and agent voices (R4, R6) become part of this.

### 10.6 Societies: cooperation, institutions, economy and conflict
- **Households:** shared stores and work, division of labor by role, skill and culture.
- **Cooperation:**
  - joint intentions and commitments (who does what by when), with role allocation for group tasks (hunts, harvests, building) by negotiation or bidding
  - a ledger of reciprocity and obligation
  - reputation spread by gossip
  - replanning together when things fail
- **Institutions as collective agents** (households, councils, chiefs, temples, guilds, markets, courts, armies), each with rules (data) and a decision procedure.
  - Near players, or when it matters, deliberations among members are run as structured multi-party language-model sessions, each member arguing from their own interests and character.
  - Elsewhere they're resolved by the numeric models.
- **Economy:** household production and consumption, markets with prices from supply and demand, barter to money by era, traders travelling real routes, scarcity raising the risk of conflict.
- **Settlements grow** because people decide to build: choosing sites and building forms from their culture's vocabulary, gathering materials and building with the real construction and structural systems. Roads emerge from use.
- **Conflict:**
  - interpersonal (an escalation ladder ending in real punches, kicks and tackles)
  - feuds and raids
  - wars: armies are collective agents moving on the route graph. Battles near players are fought by individuals; elsewhere they're resolved statistically.
- **Pathfinding at every scale:**
  - the region graph for long journeys
  - hierarchical navigation over the smooth terrain's navigation grid
  - flow fields for crowds and armies, local avoidance (ORCA)
  - roads preferred, plus special links (climb, swim, ford, boat, ladder, door)
  - path caching shared within groups, and per-tick budgets

### 10.7 Lives among people: births, childhood, any station in life
- Choose an era → the History Engine runs to its date → choose a place → choose a **birth from real households of any station that exists there**: a ruler's heir, a farmer's child, a herder's, a smith's, a servant's, a wandering family's. Each household is described soberly and truthfully.
- **Childhood is played as your character's past.** The present runs at real time, so childhood can't happen in it. Instead it's a sequence of **vignettes** run as real scenes with your actual family as C4 minds:
  - Your parents **talk to you, teach you** (real knowledge transfer) and keep you close.
  - If you wander off, they call, search, grab you and carry you back, scold you and worry. You can get lost, scraped and frightened.
  - In rare, genuinely dangerous situations (a river, a cliff edge, a predator) a vignette can end in the child's death. Such moments **cut away before any harm is shown** and are never graphic. Your family's grief is real.
  - **No one can ever deliberately harm a child.**
  - Years pass between vignettes, as history. Your choices shape your skills, relationships and memories.
  - When you come of age, **the present begins**.
- Then you live: work, status, friendships and enmities, marriage, children born in real time, politics, war, with C2–C4 minds around you.
- With people in the world, death regains "live on as another person" (Addendum B) in eras that have people.

### 10.8 Compute, cost and hardware
- **A "Minds" quality setting:**
  - *Procedural* (no language models: C0–C2 only; always available)
  - *Local small* (C3 on a small local model)
  - *Local full* (C3 and C4 locally; needs a strong GPU)
  - *Hosted* (C3/C4 through a user-configured provider, with costs shown)
- **Efficiency:**
  - batching
  - prefix caching of each person's and culture's standing context
  - small models fine-tuned for the game's schemas
  - distillation (large-model behavior distilled into small models and into the System 1 action model)
  - quantization, speculative decoding
  - inference scheduled on background queues that never steal frame time
  - graceful degradation (more C2, fewer C3/C4) when compute is short
- **Multiplayer:** the server host provides the minds' compute. Clients render and synthesize voice locally.
- **Determinism:** a **decision journal** records every model-made decision that affects the world. Saves and replays use it, and multiplayer syncs from it with state hashes.
- **World creation:** the Historian runs in batch with a progress bar and a time estimate.

### 10.9 Eras
- Each era is a starting condition and a date for the History Engine, a set of species and peoples, a knowledge baseline, social forms, a material-culture vocabulary and plan libraries.
- Roll them out in order: Upper Paleolithic first (the vertical slice), then Lower and Middle Paleolithic, Neolithic, Bronze Age, Iron Age / Classical, then later eras as the technology graph's planned nodes (v2 §12.2) are built.
- "Born a prince or a commoner, or anything that existed then" comes from the households the History Engine produces.

### 10.10 Safety and content rules (hard constraints)
These replace V2.1 §1. They're engineered into the action vocabulary, the rule engine's preconditions, the minds' instructions, output filters and schema validation, with audit logs and automated red-team tests in CI.
1. **Genetics and ancestry:** genes give individual variation; no behavioral, cognitive or moral trait differs by ancestry or population. Group differences come only from culture, environment and history (unchanged from V2.1 §1.1).
2. **Children:** never targets of deliberate violence by players or people. Dangers in childhood vignettes are never shown (§10.7). **Caregiving affection** (holding, carrying, comforting, a goodnight kiss on the forehead) is part of family life and is its own non-romantic category of actions.
3. **Nothing romantic or sexual ever involves anyone under 18.** Every romantic and sexual system excludes minors at every layer: action preconditions, minds, instructions, output filters, tests.
4. **Intimacy between adults:**
   - It's consent-modeled: both people's genuine willingness is required, and for a player and a person, the person's own willingness.
   - Holding hands, hugging and kissing are shown.
   - Sex exists only as an **implied, off-screen life event** (fade to black) that the simulation records (a bond, a pregnancy). It's never explicit.
   - **Sexual violence is not represented at all.**
5. **Violence** is realistic and non-gratuitous: fights, feuds and wars happen, but there are no torture or mutilation mechanics.
6. **Historical institutions** (hierarchy, servitude, war, inequality) are represented soberly as social structures, never as reward loops.
7. **Fictional cultures only:** no real ethnic groups, religions or real people imitated or caricatured.
8. **Minds stay in the world:** they never produce real-world harmful instructions, slurs or hate, and never claim to be real people.
9. **Voice:** no cloning of real people's voices, and no recording or reuse of players' voices (Amendment R §5.5).

### 10.11 Evaluation
- Believability: blind comparisons, rubric-based judging and human playtests.
- Era plausibility reviews.
- Consistency against the World Bible.
- Social-science sanity checks: demography, economics, conflict rates.
- Long-run stability (no unexplained explosions or collapses).
- Performance and cost per hour of play.
- Safety red-team suites that must always pass.

### 10.12 What's built now that this will use
- The Actor rule and the action vocabulary (§2.3).
- The player body, face and hair (§8).
- The player-to-player interactions in Phase R (§9.3).
- The multiplayer-ready rule and interest management.
- The technology graph and process engine.
- The ecology's lazy tiers at Earth scale (§5.3).
- The archive of V2.1's work (§2.1).

### 10.13 Phase F roadmap (after R10)
Write these into `PLAN.md` as milestones, each with acceptance tests to be detailed when Phase F begins:
- **F0 — Research and prototypes:** model latency, throughput and quality on reference hardware for System 1 and System 2; a five-person believability prototype in an isolated test scene; a voice-latency prototype.
- **F1 — People foundation:** the Person record and levels C0–C2 on the Actor framework; genetics, life course and demography restored from the archive and adapted to real time and childhood-as-the-past.
- **F2 — The History Engine:** region graph, settlements, polities, economy, conflict, culture, era snapshots, with plausibility tests.
- **F3 — The World Bible and the Historian:** narrative with fact validation and lazy zoom.
- **F4 — Procedural minds (C2):** needs, emotions, planner, routines, social rules, pathfinding at every scale.
- **F5 — The System 1 action model:** training pipeline, batched runtime, fallback.
- **F6 — Reflective minds (C3):** memory, reflection, structured outputs, validators, budgets; the AI Bridge providers.
- **F7 — Conversation and voice (C4):** turn-taking, streaming speech in and out, voices, languages and translation options, multiplayer.
- **F8 — Societies:** households, cooperation, institutions as collective agents, deliberation, markets, conflict and war.
- **F9 — Settlements:** people building with the real construction system; growth and roads.
- **F10 — Births and childhood:** any household, the vignette engine with the real family's minds, coming of age.
- **F11 — Eras:** the Upper Paleolithic vertical slice first, then the others, each with an authenticity review.
- **F12 — Scale, cost and safety hardening:** million-person regions, cost and latency budgets, red-team suites, multiplayer determinism through the decision journal.

---

## 11. Tests added by this amendment
- **Controls:** binding LCtrl, RCtrl, LAlt and RAlt alone through the Controls screen; lone-modifier and combination interplay (hold vs tap rules); no Alt side effects on Windows (manual check recorded).
- **Clicking with no target:** each primary use; punches, swings, thrusts and kicks hit along their arcs and miss when they should.
- **Time:** sunrise and sunset times and day length against reference values for several latitudes and dates (including the equation of time); the year length; moon phases; every motion and process audited against its documented real rate (extend P §8's motion-timing check).
- **Earth scale:** worldgen statistics against Earth's (hypsometry, land fraction, continent and range sizes, river lengths); seam-free and consistent refinement levels; determinism; lazy-simulation memory bounds; far-field horizon screenshots from sea level, a hill and a high summit; performance on the Earth-sized planet.
- **Wild Earth start:** every suggested place's claims verified; the creator round-trips appearance; new lives and restarts work; no humans exist in the world.
- **Work:** stroke effects and rates match data; work in progress persists and resumes; holding versus toggling; gathering one object at a time; the loose-objects layer is deterministic and stores only changes; mast years and ecology removal happen; waiting processes follow their state models; inspection descriptions match state and knowledge.
- **Lint:** §2.4 and §7.6.
- **Bodies:** the E7 screenshot review and budgets.

---

## 12. Milestones

Each milestone: implement, update design docs and data, add tests, run `scripts/check.sh`, `hearth content lint` and the performance gate, update `PROGRESS.md` and `dev/PLAYTEST.md`, commit.

**E0 — Remove the human systems and re-plan.** §2 in full, plus `PLAN.md` rewritten to §0.3's order and §9's changes. *Accept:* §2.4 checks pass; the build and all remaining tests are green; `PLAN.md` and `PROGRESS.md` reflect the new order.

**E1 — The future humanity plan (documents only).** §10 written into `docs/design/future/humanity/` with research notes, and the Phase F roadmap appended to `PLAN.md`. *Accept:* every section of §10 has its document; the roadmap lists F0–F12; no code added.

**E2 — Controls.** §3. *Accept:* §11 control tests; the owner's Ctrl/Alt report resolved in `dev/PLAYTEST.md`.

**E3 — Real Earth time.** §4. *Accept:* §11 time tests; no compressed clock remains anywhere (grep for the old time-scale code); Rest / Wait works.

**E4 — Real Earth size.** §5. *Accept:* §11 Earth-scale tests; creating a world at Earth size takes ≤ ~45 s on the reference machine, with a progress screen; performance targets met or honestly recorded.

**E5 — Wild Earth start.** §6 (with the creator on the current body model). *Accept:* §11 start tests; a full new-world flow from the title screen to waking at dawn; generating and verifying the suggested places adds ≤ ~15 s to world creation.

**E6 — Work the way the body does it** (together with P4). §7. *Accept:* §11 work tests; every active process animates stroke by stroke; every waiting process can be inspected; no progress bars remain.

**E7 — Realistic human body, face and hair** (after S2). §8. *Accept:* the E7 review screenshots; budgets met.

---

## 13. Resume protocol additions
Keep all earlier protocols. Also keep current: `docs/design/future/humanity/`, `docs/archive/humans-v2.1/README.md`, `dev/PLAYTEST.md`, and an **Earth-True Status** row in `PROGRESS.md`. On restart, also run the time tests, the Earth-scale worldgen quick tests and the work tests.

---

Begin with the first actions at the top of this file. The bar for this part: the owner designs a person, picks a chalk coast from three suggestions, wakes there at dawn on a real spring day, and spends the morning doing real things at real speed: picking up flint one nodule at a time at the cliff foot, snapping hazel sticks, scooping a fire pit by hand, checking the strips of meat drying over the smoke. And they know that one day this same world will hold people as real as they are.
