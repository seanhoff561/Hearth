# AMENDMENT P — Playability Pass
## Make the game easy to understand, pleasant to control, and honest about how real life works

*Received from the owner on 2026-10-07, during S0. Kept verbatim.*

You are the autonomous engine programmer building `hearth`. The owner has been **playing the game** and reports that the content and look are good, but the game is hard to play: patchy menus, missing world management, no creative mode, a confusing interaction model, actions that happen without the body doing them, unclear sleep and time, a sky that moves far too fast, plants that slow you down when they shouldn't, generation that still fits the grid too neatly, and too many options. This document is **Amendment P**. It's a playability pass from the player's point of view. It adds little new simulation; it makes what exists playable and enjoyable.

Read all of it before changing code. All earlier ground rules still apply: v1, v2, V2.1 with Addenda A and B, Amendment S (in progress), Amendment R (last), clean-room, data-driven, deterministic, multiplayer-ready, always green, no stubs, performance budgets, the V2.1 content rules (children are never targets).

---

## 0. Ordering and first actions

### 0.1 Where this fits
1. Finish the **Amendment S milestone you're on now** to a green, committed state.
2. Do **P0–P6** (§12) next, before the remaining S milestones. They're mostly menus, input, animation, time and rules, which barely touch S's rendering work, and the owner is playtesting now.
3. **P7 (natural generation without the grid)** merges into **S5 (trees and foliage)**: do them together when S reaches S5, or right after the current S milestone if S5 is already done.
4. **P7G (grasses and ground cover)** comes right after P7/S5. It needs S2's smooth ground and S4's distant terrain, which come before S5.
5. **P8 (playability review)** runs after S8, before resuming V2-12 and the remaining H/V2 milestones. Amendment R stays last.

Insert these into `PLAN.md` accordingly and record the ordering in `DECISIONS.md`.

### 0.2 First actions
1. Read `PROGRESS.md`, `PLAN.md`, `DECISIONS.md`, `git log --oneline -40`; run `scripts/check.sh` and `hearth content lint`.
2. Write `dev/PLAYTEST.md`: every issue in this document as a numbered item with how to reproduce it, the cause once found, and its status. Keep it current; it's how the owner will check the fixes.
3. Start P0.

### 0.3 Where the reported problems come from (as of commit `cd68bba`)
These are starting points found by reading the code. Confirm each before fixing, and record the real cause in `dev/PLAYTEST.md`.
- **Options hidden in fullscreen, can't scroll:** `crates/hearth/src/menus.rs`, `Screen::Video` (and other option screens) lay rows out in a fixed `Column` with no scroll area, so at some resolutions and GUI scales rows fall off the bottom of the window.
- **No spawn choice at world creation:** `Screen::NewWorld` has no place picker. The minimal globe picker already exists in `crates/hearth/src/globe.rs` (opened in-game); reuse it.
- **Can't delete worlds:** `Screen::Worlds` offers Watch / Play / New / Back only.
- **Too many options:** `Screen::NewWorld` asks for death preset, inhabit scope, after-death knowledge, born-again, knowledge mode and loincloth. The loincloth no longer makes sense now that the player is born. Realism presets live in `data/hearth/balance/presets.ron`; death presets and knowledge modes in `hearth_save::settings`.
- **No creative mode:** only an input category `Category::Creative` exists (`hearth_input/src/action.rs`).
- **Small grass slows running:** `crates/hearth_physics/src/mover.rs` (~line 425) takes the **maximum** drag of every block overlapping the body's bounds, so a short plant at the feet slows you as much as a thicket.
- **Clouds too fast:** `crates/hearth/src/environment.rs` (~line 165) drifts clouds on **game** time ("time-lapsed like the rest of the day scale"): about 30× real speed at the default 48-minute day.
- **Stars flashing:** `crates/hearth_render/src/shaders/sky.wgsl` (~line 115) twinkles with `sin(t * (3 + h * 400))`, up to roughly 60 flickers per second.
- **Water too fast:** `crates/hearth_render/src/shaders/water.wgsl` moves both wave scales with one drift speed instead of a real dispersion relation.
- **Text options always popping up:** `crates/hearth/src/crafting_ui.rs` lists every offer by the crosshair all the time (the wheel chooses, primary action does it); `aim_words` in `client.rs` adds more text.
- **Actions without animation:** `crates/hearth_character/src/animate.rs` `Activity` covers locomotion and lying only; there are no work poses.
- **"Lying down… wait":** `body.lying` in `data/hearth/lang/en_us.json`: lying down only leads to sleep when already tired, with no explanation and no choices.
- **Grass is still blocks:** the ground is a `grass_block` (a tinted top with a side overlay and a `grass_spread` random tick) in `data/hearth/blocks/terrain.json`, and meadows are cross-sprite plant blocks (`short_grass`, `tall_grass`, `short_dry_grass`, `tall_dry_grass`) in `data/hearth/blocks/plants.json`. The flora data has 17 grass-like species, but temperate grassland is one lumped "Meadow grasses" entry, and the boreal, Mediterranean, desert and tropical-forest files have no grasses. There's no instanced grass renderer yet; Amendment S §7.3 assumed there was.
- **Spectating doesn't load the world nearby:** the Observer (`crates/hearth/src/observer.rs`) works from the globe and chronicle; the near field isn't streamed around the spectator's camera.

---

## 1. Principles for this pass
1. **The player's intent comes first.** The game should work out what the player most likely means from what they're looking at and holding, do it with a natural animation, and never make them fight the interface.
2. **Show, don't tell.** Highlight things, animate the body, and use the world's own cues. Text appears only when it helps: names, short reasons, and options when asked for.
3. **Every refusal explains itself** and suggests what would work ("You're not sleepy yet. You could rest until dusk.").
4. **Realism stays, friction goes.** Real durations, real bodies, real limits, but no busywork: long waits can be skipped, routine steps happen automatically (stowing a tool to free a hand), and nothing is hidden behind obscure controls.
5. **Fewer, clearer choices.** A small number of well-explained options beats many settings.
6. **Test everything as a player would.** Every fix gets a scripted playtest (bot or screenshot) that proves it from the player's side.

---

## 2. Game modes: Creative, Easy, Realistic

Replace the many world-rule settings with **three modes**, chosen at world creation, each with a short summary on the Create World screen.

| | **Realistic** | **Easy** | **Creative** |
|---|---|---|---|
| Summary shown | "Life as it really is. Real needs, real dangers, real time. You only know what you learn, and when you die, life goes on through someone else." | "The same world, more forgiving. Needs and injuries are gentler, danger is rarer, and the game gives you more hints while you learn." | "Build, explore and experiment freely. You can't be hurt, can fly, can place or summon anything, and can control time and weather." |
| Needs and injuries | Authentic values (`presets.ron` "authentic") | Lenient values (the current "hardy" preset, retuned so a new player survives a first season with basic care) | Off: health, food, water, warmth, rest and stamina always full; no damage |
| Knowledge | Discovery | Guided (clearer hints) | Open (everything known) |
| Hints and tutorial prompts | Off by default (can be turned on) | On | Off |
| Clock and date | No clock; the pause menu describes the time in words ("Late afternoon, the third day of autumn") | As Realistic, plus an optional small clock and day counter | Exact clock and date, editable |
| Spectating, Observer, time control, weather control | **Not available**, from the world or the main menu | **Not available** | Available any time |
| After death | Inhabit another person (adult or child) or be born again (Addendum B), knowing only what they know; or restart the world. **No spectating.** | As Realistic, but discoveries carry over (Addendum B "Keep everything") | Can't die |
| Creative inventory, flight, instant actions, clear view | No | No | Yes (§3) |
| Start | Born (Addendum A) at the place chosen on the globe | Born, same flow | Appear as an adult at the chosen place (or choose to be born) |

- Everything previously set separately (death preset, inhabit scope, after-death knowledge, born-again, knowledge mode, realism preset, predator behavior) is now **derived from the mode** in one data file, `data/hearth/balance/modes.ron`. Keep the underlying settings in code for tuning and tests, but don't show them on the Create World screen.
- **Changing mode later** (world's Edit screen): only toward less strict. Realistic → Easy is allowed; any → Creative is allowed and marks the world permanently "Played in Creative" (shown in the world list). Nothing can become Realistic again. Confirm with a clear warning.
- **F3 debug overlay:** in Realistic and Easy it shows performance information only, unless **Developer mode** is enabled in Settings → Advanced. In Creative it shows everything.
- **Multiplayer (for Amendment R):** the server sets the mode. In Creative worlds the host can grant or deny creative powers per player. Note this in `dev/AMENDMENT_R.md`.

---

## 3. Creative mode

### 3.1 Powers
- **Always full:** health, food, water, warmth, rest, stamina; no injuries, illness or fall damage.
- **Flight:** double-tap Jump to fly or land; Sprint for speed; Crouch descends; adjustable flight speed with the wheel while flying. A **No-clip** toggle passes through terrain.
- **Instant actions** (toggle, on by default): processes complete immediately; place and break instantly; sculpt terrain with adjustable brushes (Amendment S §8.3).
- **Creative inventory**: a **Creative** tab in the inventory screen (the inventory key, Tab by default), an alternative to the normal inventory with search, categories and favorites:
  - **Materials and terrain:** every natural material and rock, with brush size and shape for placing and removing.
  - **Plants:** every species, choosing age or size and season state; place one, or paint with a brush at a chosen density. Grasses are painted as a sward (§11.5.6).
  - **Animals:** every species, choosing sex and age (adult or young); summon one or a group (herd, pack).
  - **People:** summon an adult of a chosen species and era, either a stranger or added to a nearby group. **Children can't be summoned or removed**; they only come from births (V2.1 §1.2).
  - **Items:** every item form × material, with quality and condition; containers, clothing, tools, weapons, food.
  - **Building pieces and workstations:** every piece and station, placed finished or as a construction stage.
  - **Knowledge:** grant or revoke any knowledge node and set skill levels (for testing).
- **Pick (Ctrl + middle-click):** puts the highlighted thing in hand from the creative inventory.
- **Remove tool:** removes a highlighted thing (item, plant, animal, adult person, terrain volume). It never affects children.
- **Time and weather panel** (in the pause menu and on a key): time of day, time speed (stopped to very fast), date and season, starting a weather state, wind, cloud cover, temperature offset.

### 3.2 Clear view
A **Clear view** toggle (key `F4` by default in Creative) for seeing exactly what's going on: full brightness regardless of darkness, no fog or aerial perspective, no volumetrics, bloom, auto-exposure or color grading, no weather particles, optional hiding of grass and leaves, and optional outlines on all interactables. Each part is individually toggleable in a small panel. Available only in Creative (and Developer mode).

### 3.3 Spectating in Creative
- **Spectate** (key `F6` by default, or from the pause menu) detaches the camera and leaves your body where it is, safe (invulnerable as in all of Creative).
- **The world loads around the spectator exactly as around a player:** the full-detail near field (cubic chunks, meshing, physics-ready cubes), the LOD around it, and the ecology and people materializing nearby. The spectator camera is a full **interest and simulation center** like a player (the same code path that Amendment R uses for multiple players; this is a good first test of it).
- **Resume here:** leaving spectate brings your body to the camera's position, placed safely on the ground below it (never inside terrain, never in deep water unless chosen). **Return to body** goes back to where you left it instead.
- Observer features (V2.1 §15.4: overlays, chronicle, follow, time speeds) are available while spectating in Creative.

---

## 4. Menus and world management

### 4.1 Layout that always fits
- Every screen uses a **scrollable layout container** that adapts to the window size and GUI scale: rows never fall off the screen, and the mouse wheel, drag, keyboard and controller all scroll.
- Long option screens are grouped into **tabs or collapsible sections** (Video: Display, Quality, Effects, Distance, Advanced).
- **Automated layout test:** render every screen at 1280×720, 1600×900, 1920×1080, 2560×1440, 3840×2160, 2560×1080 (ultrawide) and 1024×768, at each GUI scale (Auto, 1–4), in windowed and fullscreen; assert every interactive widget is reachable (on screen or within a scroll area), nothing overlaps, and no text is clipped. Add it to `scripts/check.sh` (headless).

### 4.2 Worlds screen
List with each world's name, mode, era, play time, last played, the character's name and age, and a small globe thumbnail. Actions: **Play**, **Create new**, **Edit** (rename, change mode per §2), **Duplicate**, **Back up** (a dated copy), **Open folder**, and **Delete** (a confirmation that names the world; deleted worlds go to a recoverable `trash/` folder for 30 days, with "Empty trash" in Settings). Watch is shown only for Creative worlds.

### 4.3 Create World (short and clear)
1. **Main page:** World name, Seed (optional), **Mode** (Creative / Easy / Realistic, each with its summary), **Era** (with its one-line description), and a **More options** section, collapsed by default, holding only world-shape settings (planet size with "Standard (recommended)", vertical scale, day length, days per season, starting season). Remove the loincloth choice and all death/knowledge/realism settings from this screen.
2. **Create** generates the planet with a progress screen that says what it's doing ("Raising mountains…", "Running rivers…", "Peopling the land…").
3. **Choose where to be born** on the **globe** (finish the `globe.rs` picker here): rotate and zoom; hovering shows the region's climate, season now, dangers and resources in plain words, and who lives there in this era; filters for "Recommended for new players" and "Surprise me"; clicking chooses (water snaps to the nearest suitable coast).
4. **Choose your birth** (Addendum A: 2–4 households near that place, daughter / son / chance). In Creative, this step is replaced by "Appear here as an adult" with an option to be born instead.
5. Show a short loading screen with a tip, then begin.

### 4.4 Pause menu
Resume, Field Guide (§9.3), Settings, the time described in words (Realistic/Easy) or the exact clock (Creative), the Creative panel (Creative only), Save, Save and quit to title. Single-player pauses the world while the pause menu is open.

---

## 5. Looking, highlighting and the hands

This replaces the always-on list by the crosshair (`crafting_ui.rs`) and most aim text.

### 5.1 Highlighting what you look at
- The **single thing** the player is looking at is **highlighted** with a soft outline or glow: a specific branch on a tree, one plant, a stone on the ground, a patch of soil (showing the area a dig would take, Amendment S §8.3), a water surface, an animal, a person, an item, a building piece, a workstation.
- Picking is precise at the sub-object level: tree branches are picked from the tree's skeleton segments, plants by instance, terrain regions by the sculpt preview, items and entities by their shapes. Use an ID-buffer pick or ray tests against proxies; it must be cheap (under 0.2 ms per frame).
- An optional **small name** appears near the crosshair, as the character knows it ("hazel branch", or for an unknown plant "a low shrub with dark berries"). Setting: Off / Brief (default; fades after a moment) / Always.
- Two small **hand hints** by the crosshair show what Left and Right click will do with the highlighted thing (an icon and one or two words, e.g. "L: snap off · R: —"). Setting to hide them.

### 5.2 Left and right click: each hand does its natural thing
- **Left click uses the left hand, right click the right hand**, performing that hand's **intended use** on the highlighted target, with the matching animation (§6). If there's no sensible use, nothing happens (and the hand hint shows "—").
- The **intent resolver** (data-driven rules in `data/hearth/interaction/intents.ron`) chooses the default action from the target, the item in that hand (or empty hand), the character's knowledge, and context:
  - empty hand + reachable branch → snap it off (if breakable by hand)
  - empty hand + berries → pick
  - empty hand + water → cup hands and drink
  - empty hand + loose stone or item → pick up
  - empty hand + soft soil → dig by hand
  - hammerstone + flint nodule → strike a flake
  - stone axe + tree → chop
  - spear + animal → thrust
  - water skin + water → fill
  - food in hand + no target → eat
- **Safe defaults only:** a default action is never irreversible or risky when the player might not mean it. Eating an unknown plant, attacking a person, breaking something you built, dropping something into water: these are only in the action menu (§5.3) or need a deliberate hold.
- **Learned preference:** if the player keeps choosing a different action for the same target and item from the menu, that becomes the default for that combination (per character, saved, resettable in Settings).
- **Freeing a hand automatically:** if the action needs a hand that's holding something, the character **stows** it first (to the best fitting place: sheath, belt, pouch, bag; animated), then acts. If there's no room, it puts the item down beside them and says so. Two-handed actions stow both. Setting: **Return items to hand afterwards** (default on).
- **Hold to repeat:** holding the button repeats the action (keep picking berries, keep digging) until released or until it can't continue.

### 5.3 The action menu (middle click)
- **Pressing the mouse wheel** opens a compact **action menu** for the highlighted thing: every action possible with what's in your hands, what you carry and what you know, for that target. The wheel scrolls; **left click does it with the left hand, right click with the right** (or the game chooses the right hand when only one makes sense); Esc or another middle click closes it.
- Each entry shows the action, the hand(s) and tool it uses, roughly how long it takes ("about 10 minutes"), and, if it's not possible right now, it's greyed with the reason ("needs a sharp edge", "too heavy to lift alone").
- Combining things in your hands works here too: hold a flake in one hand and a shaft in the other, look at nothing or at the shaft, middle click: "Bind the flake to the shaft".
- Actions on yourself (eat, drink from a skin, bandage, change clothes, sleep, rest) are available with no target by middle-clicking while looking at your own body (looking down) or at the ground.
- The former "pick block" binding moves to Ctrl + middle click (Creative).

### 5.4 Feedback without clutter
- A small **progress ring** at the crosshair for actions in progress.
- Short **result lines** that fade ("Got 3 hazel sticks"), batched when repeating.
- Discoveries and hunches (V2-5) appear as a brief, quiet toast with a journal icon, not a block of text.
- All other on-screen text is reviewed: anything that isn't one of the above, the HUD from v2 §9.9, subtitles, or a menu should be removed or moved behind a key.
- Controller support: the action menu as a radial on a held button, hands on triggers.

---

## 6. The body does the work: poses, animation and skipping waits

### 6.1 Work poses and animations
- Extend `hearth_character`'s `Activity` (and the skeleton animation system) with a **work-pose library**, each with an entry pose, a looping work motion, an exit, IK for hands to the target and feet to the ground, and tool motion. At minimum: kneel and dig by hand, dig with a stick or tool, squat and knap, sit and work in the lap (weave, sew, carve, scrape), kneel at water to drink or fill, kneel and blow on a fire, drill fire (hand and bow drill), stand and chop, reach and pick, pluck low plants, snap a branch (two-handed bend), carry heavy (both arms, leaning), drag, roll a log, hammer on an anvil, grind on a quern, sit and eat, lie to sleep, sit to rest, stow and draw from each attachment point.
- **Every process** (v2 §11.3) declares in data its pose, which hands it uses, tool motion and target point. `hearth content lint` fails if a process has no pose.
- The **first-person view** shows the hands and tools doing the work; **third-person** shows the whole body. Moving the camera during a long action is allowed (look around while you work).
- **Agents use the same poses** (V2.1): people kneeling to dig, sitting to sew, squatting to knap. A camp should look like people at work.
- Works with the current cuboid rigs and carries over to Amendment S's smooth bodies (S6), since both share the skeletons.

### 6.2 Long actions: skip the wait
- Any action longer than a few seconds shows a small prompt: **Hold Space to skip ahead** (rebindable). Holding fades the screen and **fast-forwards the world** (like sleep, v2 §9.5) until the action finishes. The world keeps simulating at the accelerated rate (fires burn down, weather changes, animals move), so the result is identical to waiting.
- **Interruptions** stop the skip and the action: a threat nearby, being hurt, getting too cold or wet, hunger or thirst becoming urgent, a person speaking to you. The reason is shown ("Something is moving in the brush").
- **Cancel** by moving or pressing Esc; **partial progress is kept** where it's real (half-dug soil stays dug; a half-scraped hide stays half-scraped; a half-fired pot is spoiled, as in reality).
- **Queue:** the action menu can queue a repeat ("Dig until the basket is full", "Pick all the berries on this bush"), done with animation and skippable as one.
- **Multiplayer (for Amendment R):** the world can't be fast-forwarded for one player. Instead, skipping shows a compact progress view while the character keeps working in real time; the player can look around, open menus and talk. Note this in `dev/AMENDMENT_R.md`.

---

## 7. Sleep, rest and time that make sense

### 7.1 Sleep that works like a human's
- Model sleepiness with the standard **two-process model**: **sleep pressure** (rises while awake, falls during sleep) plus a **circadian rhythm** (strong drive to sleep at local night, alertness in the morning, a mild afternoon dip), adjusted by light, exhaustion, comfort, temperature, pain, hunger and fear. So lying down in the dark at night leads to sleep within a realistic time even if you aren't exhausted, and sleeping at noon is hard unless you're very tired.
- **Lying down** (middle-click the ground, bedding or a bed, or the Sleep key) opens a small menu instead of "wait": **Sleep until morning**, **Sleep until rested**, **Rest for one / two / four hours** (sit or lie and recover stamina without sleeping), **Get up**. If sleep isn't possible yet, it says why and what will happen ("You're not sleepy yet. If you lie here you'll probably fall asleep around dusk.") and offers **Rest until sleepy**.
- While sleeping or resting, time is fast-forwarded with the fade (v2 §9.5), with interruptions explained on waking ("You woke shivering: the fire has gone out").
- In Easy and Realistic, **sleeping well matters and feels good:** a clear "well rested" state and a visible effect when sleep-deprived (v2 §9.5), described in the Body panel.

### 7.2 Making time understandable
- Show the time without a clock in Realistic: the **pause menu** and the **Body panel** describe it in words ("Late afternoon, about two hours before sunset; the third day of autumn; you are 19").
- When time is skipped (sleep, rest, long actions), a short line tells how long passed ("You slept about seven hours").
- The Field Guide (§9.3) explains the two time scales (v2 §4.2) simply: a day passes in about 48 minutes of play, and growing seasons and life go faster still.
- Easy shows an optional small clock and day counter; Creative an exact clock.

---

## 8. Motion that looks real: slowing the sky and everything that moves

The day is compressed (48 minutes by default), so the **sun, moon and stars must move across the sky faster than in reality**; that's by design. But everything else that moves must move at **real-world speeds in real seconds**, unless the player is actively fast-forwarding.

- **Clouds** drift at the real wind speed at cloud height, in real seconds (fix `environment.rs`). Cloud shapes evolve slowly (minutes, not seconds). Storm fronts move at realistic speeds.
- **Stars:** most stars look steady. Scintillation is subtle (a few hertz at most, small amplitude), stronger near the horizon and on turbulent or windy nights, barely visible overhead (fix `sky.wgsl`). Planets don't twinkle.
- **Water:** each wave scale moves at its physical phase speed from the deep-water **dispersion relation** (c = √(gλ/2π); long swell faster than short chop), shallow-water slowing near shore, rivers flowing at their simulated current speed, waterfalls and splashes at real speeds (fix `water.wgsl`).
- **Audit everything that animates** and list each in `docs/design/motion-timing.md` with its clock (real or game) and speed: wind sway of grass, leaves and branches (real gust timescales); rain (falls at about 9 m/s) and snow (about 1 m/s); fog drift; smoke and fire flicker; dust; lightning; animated textures; animal and human animation playback (feet must not slide); particle effects; the sun's glitter on water.
- **When time really is fast-forwarded** (sleep, skip, Creative time speed, Observer), visuals may time-lapse, but smoothly: blend cloud evolution, use motion blur or crossfades instead of strobing, and cap the visible rate of change.
- Test: a "motion timing" check that renders short sequences at normal speed and verifies each visual's rate against its documented real speed.

---

## 9. Learning to play

### 9.1 Learn to Play (main menu)
A **tutorial** in a small, hand-tuned scenario (a fixed seed and spot: a temperate valley in late spring, with a stream, flint in a bank, a berry bush, a hazel thicket, a few deer and a safe camp), in Easy rules, in short chapters, each skippable and replayable:
1. Looking around, highlighting, the hand hints.
2. Left and right hands; picking berries; snapping a branch; picking up a stone.
3. The action menu (middle click); combining things in your hands.
4. Carrying: hands, the loincloth tie, bundles; putting things down.
5. Drinking from the stream; eating; reading your body (the Body panel, the sensations).
6. Striking a flake from flint; your first hunch and discovery; the journal.
7. Fire: carrying embers from a lightning-struck tree, or making it by friction; feeding and banking it.
8. Time: the sun's path, how fast the day goes, resting, pulling grass for a bed, sleeping through the night, skipping a long action.
9. Danger: wind and scent, a deer that smells you, keeping a fire at night.
10. People (if the era has them): gestures, the speech-act wheel, learning by watching.
11. The globe and map; saving.
The tutorial uses the real systems (no special-cased rules) and is tested by a bot that completes it.

### 9.2 First-time hints (Easy by default; optional in Realistic)
Short, contextual, one-time hints when something first comes up ("It's getting dark. People sleep at night. Middle-click the ground to lie down."), each dismissable, with "Turn off hints" and "Reset hints" in Settings.

### 9.3 Field Guide (basic version now)
A searchable in-game guide (pause menu, key `H` by default) with short plain-language pages on each system: the body, sleep and time, carrying, the hands and action menu, making things, fire, food, plants, animals and danger, the land, building, people, modes, controls. Keep it brief now; Amendment R §9 expands it later from the same source.

---

## 10. Moving through plants, and plants that look like plants

### 10.1 Fix the slowdown (do this in P0: it's small)
Replace the "maximum drag of any overlapping block" rule (`mover.rs`) with a model per plant:
- Each plant contributes drag from its **height relative to the body**, its **stem density and stiffness**, and **how much of the body's path it occupies**, combined over the plants actually touched (not the worst block in the bounding box).
- **Short grass, herbs and seedlings (below the knee):** essentially no slowdown (≤ 2%). **Knee- to waist-high grass:** a few percent. **Dense shrubs and brambles:** substantial (20–50%), with scratching and noise. **Thickets:** you push through slowly or go around.
- For grasses, sedges and reeds, the height, density and stiffness come from the **sward** (§11.5) rather than from individual blocks; for shrubs and other plants, from their real shapes (§11.1).
- Plants you push through are **bent and partly flattened** (trails form where people and animals pass often, v2 §5.6), so the second pass is easier.
- Crouching and crawling through tall grass hides you (v2 stealth) but costs speed as it should; running through it makes noise but stays fast.
- Test: sprint speed across short meadow grass is within 2% of bare ground; a thicket slows as specified.

### 10.2 Plants and foliage that look like what they are (with P7)
See §11. Bushes, shrubs and ground plants are generated as **real plants with real forms** (like trees), so you can see what you're walking into, and they connect into continuous masses as they do in nature.

---

## 11. Natural generation without the grid (P7 with Amendment S S5; grasses in P7G)

Everything natural should look like it grew or formed on its own, not like it was placed in grid cells. Use the realism principles: every pattern needs a natural cause.

### 11.1 Plants
- **Shrubs and bushes are procedural individuals**, grown like trees (v2 §6.2, Amendment S §7) with species forms: multi-stemmed hazel, arching bramble canes, rounded juniper, spreading blueberry, sprawling rose, upright elder. Berries, flowers and thorns sit on their real branches.
- **Continuous positions and orientations:** plants are placed at real-valued positions with random rotation, lean and size variation, never one-per-cell or cell-centered. Several plants can share a cell; one plant can span many.
- **Natural distributions:** spacing from competition (Poisson-disk-like spacing with species-specific distances), clustering from seed dispersal (berry bushes along forest edges and clearings where birds drop seeds; wind-dispersed plants spread downwind), **clonal patches** spreading from a root (bracken, reeds, cattails, bamboo, nettle beds), understory layers under canopies, and edges that blend gradually between habitats.
- **Connected foliage:** neighboring crowns overlap and **merge into continuous masses**: hedges, thickets, brambles, reed beds, shrub belts along streams and forest edges. Foliage occupancy (Amendment S §7.2) is computed from the actual plant shapes, so passability, slowdown, hiding and shade follow what you see.
- **Grasses and ground cover** are a living layer of their own: see §11.5. Herbs, ferns and flowers grow as individual plants within it, in drifts and patches.
- **Seasons and growth:** plants grow, flower, fruit and die back by season (v2 §4.3) at their real forms.

### 11.2 Everything else natural
- **Rocks and boulders:** varied shapes and sizes, randomly oriented, partly buried, in natural places (glacial erratics on shields, talus under cliffs, river cobbles sorted by size downstream, beach shingle).
- **Fallen logs and deadwood:** at real angles following how trees fall (downslope, with the prevailing wind), decaying over time.
- **Trees:** natural lean (toward light, downslope on unstable ground), asymmetric crowns, spacing by competition, gaps from old falls.
- **Water and land forms:** shorelines, river banks, dunes, scree fans, snowdrifts, ice and caves all follow Amendment S's smooth surfaces; nothing aligns to grid axes.
- **Geology:** ore veins along dipping planes and fractures, strata that dip and fold with the province (v2 §5.1), not axis-aligned blobs.
- **People's places:** camps and settlements placed and oriented by terrain, shelter, water, sun and wind (V2.1), not aligned to the grid's axes.

### 11.3 Built structures (evaluate)
Built pieces snap to a grid (Amendment S §6). Evaluate giving each **structure its own building grid with any origin and rotation** (chosen when its first piece is placed), so a hut can face the view or the wind instead of the world's axes. The structural solver would run in the structure's local grid, and the pieces would be rasterized into the world voxels for light, fluids and collision. Implement it if it fits the budgets and stays deterministic; otherwise record why in `DECISIONS.md`. Small placed things (items, fire pits, drying racks, lean-to poles) are always freely positioned and rotated.

### 11.4 Tests
Statistical tests that plant spacing, clustering and cover match their designed distributions; that no natural feature's positions are quantized to the grid (a test on fractional positions and orientations); that foliage occupancy matches rendered plants within tolerance; and the screenshot suite (meadow, forest edge, thicket, reed bed, talus, river cobbles).

### 11.5 Grasses and ground cover (P7G)

Grass is what the player sees most, so it has to be right. Today it's blocks (§0.3). Replace them with a living grass layer, real grass species, and a renderer that draws real blades on the smooth ground. This section **supersedes Amendment S §7.3** for grasses and grass-like plants.

#### 11.5.1 Grass is a living layer, not a block
- **Remove `grass_block`.** The ground's material is its real soil (v2 §5.3: brown forest soil, chernozem, podzol, alluvium, loess, sand and so on). "Grassy" is not a kind of ground: it's a **sward**, the living grass layer growing on it.
- **Each sky-lit surface of natural soil has a sward**, with: its species mix (up to about four species with their shares), **cover** (how much ground the grass covers; the rest is bare soil, litter or moss), current **height**, the shares that are **green, cured (standing dry) and dead thatch**, the flowering and seed stage, and its disturbances: trampled or flattened, grazed or cut (biomass removed), burned (time since the fire), pressed by snow, and wet (rain, dew or frost).
- **Derived, not stored.** The undisturbed sward is computed deterministically from climate and season, soil, moisture and drainage, light (shade from trees and shrubs), slope and aspect (sunny slopes drier), elevation, the ecological cell's grazing pressure and fire history (v2 §6.6), and natural pattern noise. Only **deviations caused by events** (grazing near the player, cutting, trampling, burning, digging, sleeping on it) are stored, sparsely, and they relax back toward the undisturbed state as the grass regrows on the calendar time scale. Keep the save-size impact small.
- No sward on bare rock (a few tufts in cracks), thin on steep slopes, little in deep shade (only shade-tolerant woodland grasses and sedges), none under water (seagrass stays separate).
- **One source of truth:** the sward drives rendering (§11.5.4), movement drag and noise (§10.1), hiding (§11.5.5), forage for grazing animals (v2 §6.6, §7.4), fire fuel (v2 §5.7), the materials gathered from it, and the color of the land at every distance.

#### 11.5.2 Grass types: real growth forms and real species
Growth form decides how grass looks and behaves. Give every grass and grass-like species in `data/hearth/flora/` these fields:
- **Habit:**
  - *Sod-forming*: spreads by rhizomes or runners into continuous turf (meadows, pastures, grazing lawns).
  - *Bunch*: separate clumps with bare soil between (steppe, semi-arid and Mediterranean grasslands).
  - *Tussock*: dense raised mounds (tundra cottongrass and sedge tussocks, páramo, alpine snow tussocks).
  - *Tall*: stands 1.5–4 m high (tallgrass prairie, elephant grass, reeds).
  - *Hummock*: spiny domes (spinifex, in arid Australasian-like realms).
  - *Clonal emergent*: stands rooted in shallow water (reeds, cattails, papyrus, bulrush).
  - *Woody*: bamboo, grown by the tree system (Amendment S §7) as culms, spreading as running or clumping groves.
- **Photosynthesis pathway:**
  - *C3 (cool-season):* greens up early in spring, can brown in summer heat or drought, greens again in autumn. Dominates cool temperate, boreal, tundra and alpine places.
  - *C4 (warm-season):* greens up late, peaks in summer heat, cures golden in autumn or the dry season. Dominates the tropics, savannas and warm prairies.
  - The C3/C4 mix should emerge from the species' climate envelopes, so a landscape's colors through the year are right for its latitude and altitude.
- **Form and color:** blade height range, width, stiffness, curvature and droop; colors from root to tip by season; sheen (glossy, matte, or a hairy blue-grey bloom); flower and seed heads (panicle, spike, raceme, feathery awns, plumes, cotton tufts) with their colors and timing.
- **Phenology:** green-up, flowering, seed set, curing and dormancy by pathway and climate (v2 §4.3).
- **Ecology:** shade, moisture, soil and salt tolerance; grazing tolerance and palatability by season (forage value for each herbivore); trampling tolerance; fire response (many grasses resprout fast after fire); spreading rate.
- **Uses:** bedding, thatch, tinder (when dry), fibre for cordage and basketry (some species), edible seeds after processing, fodder and hay (from the Neolithic), turf as a building material (§11.5.5).
- **Grass-likes** stay distinct where it matters: sedges (triangular stems; tussock sedges, cottongrass, papyrus), rushes (round stems) and cattails share the system but aren't true grasses.
- **Identification:** until the character knows a species, it's described by its appearance ("a tall grass with purple seed heads").

**Species set.** Replace the lumped "Meadow grasses" entry and give every zone its characteristic grasses. Keep the existing 17 entries. This is a minimum; add more where a zone needs them, with realistic values and sources, marking uncertain ones (v2 §3.1):
- **Temperate meadows and pastures:** smooth meadow-grass (*Poa pratensis*), red fescue (*Festuca rubra*), common bent (*Agrostis capillaris*), cocksfoot (*Dactylis glomerata*), Yorkshire fog (*Holcus lanatus*), sweet vernal grass (*Anthoxanthum odoratum*), crested dog's-tail (*Cynosurus cristatus*), meadow foxtail (*Alopecurus pratensis*).
- **Temperate woodland floors:** wood melick (*Melica uniflora*), wood millet (*Milium effusum*), false brome (*Brachypodium sylvaticum*).
- **Wet meadows, heaths and moors:** tufted hair-grass (*Deschampsia cespitosa*), purple moor-grass (*Molinia caerulea*), wavy hair-grass (*Avenella flexuosa*).
- **Chalk and limestone grassland:** sheep's fescue (*Festuca ovina*), upright brome (*Bromopsis erecta*).
- **Prairie:** big bluestem and blue grama (existing), little bluestem (*Schizachyrium scoparium*), indiangrass (*Sorghastrum nutans*), switchgrass (*Panicum virgatum*), buffalograss (*Bouteloua dactyloides*), western wheatgrass (*Pascopyrum smithii*), needle-and-thread (*Hesperostipa comata*).
- **Steppe:** feather grass and steppe fescue (existing), hair feather grass (*Stipa capillata*), crested wheatgrass (*Agropyron cristatum*).
- **Mediterranean:** wild oats (*Avena sterilis*), wild barley (*Hordeum spontaneum*), bromes (*Bromus*), esparto (*Stipa tenacissima*, a fibre for cordage and baskets since prehistory), diss grass (*Ampelodesmos mauritanicus*).
- **Savanna and tropical grassland:** red oat grass, guinea grass, elephant grass and wild sorghum (existing), thatching grass (*Hyparrhenia*), Bermuda grass (*Cynodon dactylon*, the short grass of grazing lawns), signal grasses (*Urochloa*).
- **Arid lands:** Indian ricegrass (*Achnatherum hymenoides*), galleta (*Pleuraphis*), drinn (*Stipagrostis pungens*), spinifex (*Triodia*), and short-lived annual grasses that appear after rain.
- **Tropical forest and clearings:** sparse shade grasses under the canopy (*Olyra*, *Pharus*); bamboos (giant clumping bamboos in Asian-like realms, *Guadua* in Neotropical-like ones, *Chusquea* thickets in montane forest); cogongrass (*Imperata cylindrica*) and wild sugarcane (*Saccharum spontaneum*) in clearings and on riverbanks.
- **Boreal:** bluejoint reedgrass (*Calamagrostis canadensis*), wavy hair-grass, sedges in fens. Most boreal ground is moss, lichen and dwarf shrubs, which already exist.
- **Tundra:** cottongrass (existing), tussock cottongrass (*Eriophorum vaginatum*), Bigelow's sedge (*Carex bigelowii*), arctic bluegrass (*Poa arctica*), alpine foxtail.
- **Alpine:** alpine fescue, ichu, páramo grass and afroalpine fescue (existing), *Kobresia* sedge meadows on high plateaus, snow tussocks (*Chionochloa*) in Australasian-like realms.
- **Coasts:** marram and smooth cordgrass (existing), lyme grass (*Leymus arenarius*), saltgrass (*Distichlis spicata*).
- **Wetlands:** common reed and wild rice (existing), reed canary grass (*Phalaris arundinacea*), soft rush (*Juncus effusus*), sedges (*Carex*), papyrus and bulrush.
- **Wild cereals (the ancestors of V2-12's crops):** wild einkorn (*Triticum boeoticum*), wild emmer (*Triticum dicoccoides*), wild barley, wild rye (*Secale*), wild oats, Asian wild rice (*Oryza rufipogon*), teosinte (*Zea mays* subsp. *parviglumis*), wild sorghum, green foxtail (*Setaria viridis*, the ancestor of foxtail millet). Each grows in climates and realms like its real homeland (wild wheats and barley on seasonally dry Mediterranean-climate hills, teosinte in seasonally dry tropical uplands, wild rice in monsoon wetlands), so domestication starts where it would on Earth.

#### 11.5.3 How grass behaves
- **Regrowth:** grazed or cut grass regrows from its base at rates set by species, season and moisture (calendar time scale): fast in the growing season, not at all while dormant.
- **Grazing shapes the land:**
  - Herds crop the sward where they feed. Near the player this is visible: short grazed patches, **grazing lawns** kept short by heavy grazing right beside tall ungrazed grass, rabbit-cropped turf around warrens.
  - Grass grows greener and taller around dung.
  - Trampled bare mud surrounds water holes.
  - Link the existing grazing animation (`hearth_fauna` `Act::Graze`) to real biomass removal at the animal's mouth, with the ecological cell's grazing pressure setting the undisturbed state.
- **Trampling and trails:**
  - Passing bodies bend grass, which springs back over hours.
  - Repeated passage flattens it and finally wears a path to bare soil (game trails, v2 §5.6).
  - Deer leave flattened ovals where they bedded down, and the player's camp and sleeping spot flatten too.
- **Fire:**
  - Cured, continuous grass burns fast and hot. Spread rises with the cured share, the wind and the slope, and the gaps between bunchgrasses slow it (v2 §5.7).
  - Burned ground is black. In the growing season a **green flush** of new shoots appears within days to weeks and draws grazers in.
- **Seasons:**
  - Spring green-up from the base, through the old thatch.
  - Flowering in early summer: heads sway and catch the light when backlit.
  - Seed set, then curing.
  - Winter dormancy: tan grass, matted by snow and lying flat after snowmelt.
  - In savannas, golden in the dry season and green in the wet.
- **Weather:**
  - Rain darkens grass and bows tall stems.
  - **Dew** forms on clear, calm nights and wets the legs and lower clothing in the morning (v2 §9.4 wetness).
  - **Frost** whitens grass on cold, clear mornings.
  - Wind sends **waves** rolling across tall grass, at real gust speeds (§8).
- **Succession and competition:**
  - Bare ground (dug, burned, eroded) is recolonized over the seasons by spreading sod and seedlings.
  - Trees and shrubs shade grass out, while grazing and fire keep grasslands open (v2 §6.6). The sward follows all of this.
- **Seeds that hitch a ride:** in late summer, walking through seeding grass catches seeds on clothing and fur, a real way grasses spread. Awned seeds such as needle grasses can work into clothing and are a minor nuisance.
- **Sound:** the swish of moving through grass, louder in taller and drier grass (it matters for stealth); rustling in the wind; insects singing in summer grass at night (v2 §7.9).

#### 11.5.4 Rendering grass
- **Blades generated on the GPU:** a compute pass generates grass instances from the sward in tiles around the camera, seated on and aligned to the smooth ground (Amendment S). Each instance is a species-shaped blade or tuft (a curved strip of a few vertices) with its flower and seed heads.
  - The habit decides the pattern: continuous carpets for sod, clumps with soil showing between for bunchgrass, raised mounds for tussocks, dense stands for reeds.
  - Each blade varies slightly in color, and the season's state comes from the sward.
- **Distance chain:**
  - Full blades near the camera (about 25–40 m at High).
  - Fewer blades and whole clumps at mid range.
  - **Shell or card layers** beyond that, so tall prairie and savanna still look tall at 100–300 m.
  - Then the ground's own shading: the terrain shader and the LOD terrain (Amendment S §4, §5) take the sward's color, roughness and cover at every distance (turf, thatch, bare soil between tussocks), so distant grassland has exactly the right color and nothing pops in.
- **Motion:**
  - Wind from a real-time gust field, so waves roll across fields.
  - Stiffness per species: fescue tufts stay stiff while feather-grass awns stream.
  - Bending around bodies passing through (a small displacement texture around the player and nearby animals), plus the lasting flattening from the sward's trample state.
- **Shading:** two-sided with translucency, so grass glows when backlit at sunrise and sunset; sheen; darker roots (AO); wetness, dew and frost; snow partly burying short grass.
- **Culling and LOD:** through the existing GPU-driven pipeline. Grass receives shadows, and casts them only near the camera at High.
- **Budgets** (reference machine, densest meadow and tallgrass-prairie scenes):
  - ≤ ~1.5 ms GPU at High (1440p).
  - ≤ ~0.6 ms GPU at Low (1080p).
  - ≤ ~0.2 ms CPU.
  - A grass density and distance setting per graphics preset.
  - Add the scenes to `BENCHMARKS.md`.

#### 11.5.5 Interacting with grass
- **Highlight:** looking at grass highlights a **patch** (§5.1), named as the character knows it.
- **Default actions** (§5.2):
  - Empty hand: pull a handful.
  - Cutting edge: cut an armful. The yield depends on species, height and state: green fodder, dry hay, tinder, bedding, thatch bundles.
  - Seeding grass: strip the seeds into a container. This is how wild cereals are first gathered.
- **Action menu** (§5.3):
  - Gather bedding, or make a grass bed (flatten a spot and lay an insulating layer, adding sleep comfort, v2 §9.5).
  - Twist grass into cordage (species with fibre).
  - Cut **turf** with a digging tool. Turf blocks are a building material for walls and roofs, as used historically in cold, wet or treeless regions (v2 §14).
  - Set dry grass alight.
  - Clear a patch for a fire, a camp or a field.
- **Hiding:** crouching in grass taller than your crouched body hides you, and hides animals too (a lion in tall savanna grass). Compute it cheaply from sward height and density along the line of sight.
- **Movement:** drag and noise (§10.1) come from the sward's height, density and species stiffness.

#### 11.5.6 Creative
The Creative inventory's Plants category includes grasses: paint a sward with a brush (species mix, height, cover), mow, burn, and set the season state, for testing and building.

#### 11.5.7 Farming builds on this (for V2-12)
The domesticated cereals are grasses. V2-12's crops (wheat, barley, rice, millets, maize, sorghum) use this same sward-and-blade system: fields as a sward of crop species with ripening heads and weeds, hay meadows and mowing, pasture and grazing management. Note this in `PLAN.md` under V2-12.

#### 11.5.8 Migration
- Remove `grass_block` (and its `grass_spread` tick) and the grass plant blocks (`short_grass`, `tall_grass`, `short_dry_grass`, `tall_dry_grass`). The ground under them becomes its soil type and the grass becomes the undisturbed sward. Ferns and other non-grass plants become individual plants (§11.1).
- Update everything that touches them:
  - the block and flora data and `hearth_texgen`'s grass textures
  - worldgen placement (`hearth_worldgen` `cubegen/features.rs`)
  - seasonal snow burying low plants (`season_cover.rs`)
  - the renderer's grass tint path (`hearth_render` `mesh.rs`, `models.rs`)
  - foraging and fire fuel
  - the tests that use them
- Add a save migration for existing worlds.

#### 11.5.9 Tests
- **Determinism and data:** the undisturbed sward is deterministic. It matches each species' climate envelope, with C3 and C4 shares following temperature by latitude and altitude. Stored data is deviations only, and save size stays bounded.
- **Behavior:**
  - Grazing lowers the sward near herds, and it regrows at the specified rates.
  - Trampling forms a trail after the specified number of passes.
  - Fire spreads faster in cured, continuous grass than in green or bunch grass.
  - A green flush appears after fire in the growing season.
  - Drag and hiding follow sward height.
- **Screenshot set:**
  - a temperate hay meadow in four seasons
  - chalk grassland
  - tallgrass prairie in autumn (rust and bronze)
  - feather-grass steppe in early summer, its silver awns in the wind
  - savanna in the wet and dry seasons, with a grazing lawn beside tall grass
  - tundra tussocks
  - an alpine meadow
  - a salt marsh
  - marram dunes
  - a reed bed
  - a bamboo grove
  - burned ground with a green flush
  - dew and frost mornings
  - views from a hill showing grassland color carrying seamlessly into the distance
- **Performance:** the §11.5.4 budgets as benchmark assertions.

---

## 12. Milestones

Each milestone: implement, update design docs and data, add tests, run `scripts/check.sh` and `hearth content lint`, update `PROGRESS.md` and `dev/PLAYTEST.md`, commit.

**P0 — Triage and quick fixes.** `dev/PLAYTEST.md`; reproduce every reported issue; the plant slowdown fix (§10.1); the motion-timing fixes for clouds, stars and water (§8, the documented audit can finish in P5). *Accept:* sprint through short grass ≤ 2% slower; clouds, stars and water move at real speeds at normal time; each issue has a cause recorded.

**P1 — Menus and world management.** Scrollable, responsive layouts and the automated layout test (§4.1); Worlds screen with delete (to trash), rename, duplicate, back up, edit mode (§4.2); the short Create World screen, planet progress screen, globe birthplace picker and birth step (§4.3); pause menu (§4.4). *Accept:* layout test passes at every resolution, scale and display mode; a world can be created with a chosen birthplace, deleted and restored from trash.

**P2 — Game modes and Creative.** `modes.ron` and the three modes (§2); mode changes; F3 and Developer mode; Creative powers, creative inventory, pick and remove, time and weather panel, clear view, spectating that streams the world and resumes at the camera (§3). *Accept:* tests that Realistic and Easy have no spectating, time control or Observer; Creative is invulnerable and its inventory places and summons every category; spectating far away loads full-detail terrain, plants, animals and people around the camera, and resuming puts the body safely on the ground there; children can't be summoned or removed.

**P3 — Looking and the hands.** Highlighting and precise picking, name tags and hand hints (§5.1); the intent resolver with safe defaults, learned preferences, auto-stow and hold-to-repeat (§5.2); the middle-click action menu (§5.3); feedback cleanup and removal of the crosshair offer list (§5.4); controller mapping. *Accept:* a scripted playtest covers every example in §5.2; no action happens without a highlighted target or a self-action; picking costs under 0.2 ms per frame.

**P4 — Poses, animation and skipping.** The work-pose library and IK; every process with its pose (lint); first- and third-person views; agents using the same poses; skip with fade, interruptions, cancel with partial progress, queued repeats (§6). Sleep and rest via the two-process model with the lie-down menu, and the time-in-words displays (§7). *Accept:* every process animates; lying down at night leads to sleep within a realistic time; every refusal explains itself; skipping gives the same results as waiting.

**P5 — Motion timing audit.** The complete `docs/design/motion-timing.md` and its automated check, including fast-forward behavior (§8). *Accept:* every animated visual is listed with its clock and speed and passes the check.

**P6 — Learn to play.** Tutorial chapters and the bot that completes them, first-time hints, the basic Field Guide (§9). *Accept:* the bot completes every chapter; a fresh player can learn sleeping, drinking, fire and the hands without outside help.

**P7 — Natural generation without the grid** (with S5). §11.1–11.4. *Accept:* §11.4 tests and screenshots; the §11.3 evaluation recorded or implemented.

**P7G — Grasses and ground cover** (right after P7/S5). §11.5 in full: the sward layer replacing grass blocks, grass species and growth forms for every zone, grazing, trampling, fire and seasons, the GPU blade renderer with its distance chain, interactions, Creative painting, and the migration. *Accept:* §11.5.9 tests, screenshots and budgets.

**P8 — Playability review** (after S8). Play the game the way the owner does, in all three modes: create a world, choose a birthplace, be born, grow up, survive a season, sleep, make fire, hunt, build a shelter, die and continue; spectate and build in Creative. Record a short video or screenshot sequence per step and write `docs/review/playability.md` with what still feels confusing, slow or fiddly, then fix the top items. *Accept:* `dev/PLAYTEST.md` has every reported issue resolved or explained; the review's top items fixed.

---

## 13. Default keys
New default keys in this amendment (Clear view `F4`, Spectate `F6`, Field Guide `H`, skip `Space` during a long action, action menu on middle click, Creative pick on Ctrl + middle click) must be checked against the existing bindings; resolve any conflict in favor of the established v1/v2 keys and record the final layout in the Controls screen and the Field Guide.

## 14. Resume protocol additions
Keep all earlier protocols. Also keep `dev/PLAYTEST.md` current and add a **Playability Status** row to `PROGRESS.md`. On restart, run the layout test, the motion-timing check and the tutorial bot along with `scripts/check.sh`.

---

Begin with §0.2. The bar for this amendment: the owner can sit down, create a world, pick where they're born, and play for an hour without ever wondering how to do something, why a button did nothing, or why the sky is racing, and when they want to experiment, Creative lets them do anything.
