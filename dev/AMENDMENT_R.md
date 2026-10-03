AMENDMENT R — Open-Source Release, Multiplayer, AI & Voice, Guide and Trailer
You are the autonomous engine programmer building `hearth`. You've been following the v1 build prompt, the v2 direction change ("Earth, Transposed"), and the V2.1 amendment ("Realistic Humans"). This document is Amendment R. It turns the project into something anyone can download from GitHub and play in minutes, alone or with friends, adds full multiplayer with proximity voice chat, lets people plug their own AI model or agent into the simulated humans (including voice), and produces a plain-language guide and a trailer.
Read all of it before changing code. All earlier ground rules still apply: clean-room, no other games' names or assets, data-driven design, always green, no stubs, deterministic simulation, the V2.1 content rules, performance budgets.
0. Precedence, ordering and first actions
0.1 Where this fits: AFTER the game is finished
This amendment is the last phase of the project. Do not start any R milestone until the whole game is built. That means every remaining milestone in the current plan is complete and its acceptance criteria pass:

* all V2.1 human milestones H0–H13 (including the ones interleaved with V2-12, V2-13 and V2-14), and
* all remaining v2 milestones through V2-16 (long-run balance, performance and cohesion QA).

The order of the whole remaining project is therefore:

1. Finish the game — whatever H/V2 milestone you're on now, then every remaining H and V2 milestone, in the order `PLAN.md` already has them.
2. Phase R-A, online features (game features that need the finished game): R1 networking core → R2 replication → R3 hosting and admin → R4 AI Bridge → R5 proximity voice → R6 agent voices.
3. Phase R-B, release (only after R-A passes): R0 repository audit → R7 packaging and CI → R8 README, guide and docs → R9 branding, press kit and trailers → R10 launch review.

No trailer footage, press kit, README guide, release packaging or GitHub publishing work happens before Phase R-B.
0.2 First actions when you receive this document
You may receive this document while the game is still being built. If so:

1. Don't start it. Save it as `dev/AMENDMENT_R.md`.
2. Append the R milestones to the end of `PLAN.md`, after V2-16, in the order from §0.1, under a heading "Phase R (after the game is complete)". Commit.
3. Adopt only the multiplayer-ready rule (§0.3) from now on, then go straight back to your current H/V2 milestone.

When the last H/V2 milestone (V2-16) is complete:

1. Read `PROGRESS.md`, `PLAN.md`, `DECISIONS.md`, `git log --oneline -40`; run `scripts/check.sh` and `hearth content lint`.
2. Re-read this whole document. Write `RELEASE_PLAN.md`: what the 0.1 release will contain, what's labeled "coming later", and the R milestones. Commit, then start R1.

0.3 The multiplayer-ready rule (applies immediately, while finishing the game)
So multiplayer can be added at the end without a rewrite, everything you build from now on must respect the client/server boundary that v1 §2 already set up:

* All gameplay state lives on the server side of that boundary; the client only renders and sends player intents (move, act, speak, craft, build) as messages.
* Every new message type is serializable, versioned, and goes through the existing in-memory channel. No shortcut where client code reads or writes server state directly.
* Simulation code never assumes a single player (use "for each player" for simulation regions, sleep checks, interest, and anything near "the player"). This costs little now and turns R1–R3 into a transport and replication job instead of a rewrite. Record it in `DECISIONS.md`.

0.4 Honesty about scope
By the time Phase R-B starts the game should be complete, but anything still unfinished (for example, eras authored only as `planned` data) is labeled "coming later" everywhere: README, guide, in-game, trailer. Never advertise features that don't work.
0.5 Decisions that belong to the user
Pick the defaults below so work can proceed, but record each one in `DECISIONS.md` under "Needs owner confirmation before launch" and list them in the final R10 report:

* Project name. Keep the codename `hearth` in one constant. Provide `scripts/rename` (cross-platform; a small Rust `xtask` is fine) that renames the game, binaries, crates, window title, docs and branding in one step. Note that the final name should be checked for conflicts with existing games and projects before launch.
* License. Default: code dual-licensed MIT OR Apache-2.0 (the Rust ecosystem norm); original assets (generated textures, sounds, music, logo, trailer) under CC BY 4.0. If the owner prefers a copyleft license (e.g., GPL-3.0 to keep forks open), the switch should be a matter of replacing license files and headers.
* Repository location (owner/org name), contact email for security reports, and whether to enable GitHub Discussions.

1. Repository and open-source readiness
1.1 Repository hygiene (R0)

* Audit everything: no secrets, API keys, tokens or personal paths anywhere in the tree or git history (scan with `gitleaks` or equivalent; if history contains secrets, document and rewrite before publishing). No large binaries in git: generated assets are built by `hearth_texgen` at build time; any unavoidable large files go to release assets or Git LFS.
* Third-party compliance: `cargo deny` with a license allowlist and advisories check; `cargo about` (or equivalent) generates `THIRD_PARTY_LICENSES.html`, shipped with every build. Every bundled or auto-downloaded model, sound, font and music file is listed in `ASSETS_LICENSES.md` with its license and source. Only bundle or auto-download things whose license permits redistribution.
* Clean-room check: grep the tree and docs for other games' names, trademarks, assets and identifiers; remove them from the game, branding, trailer and marketing text. The README must not claim or imply affiliation with any other game or company.
* Pinned toolchain: `rust-toolchain.toml`; `cargo xtask` for common tasks (build, run, test, lint, package, render-trailer, docs); a devcontainer and an optional Nix flake for contributors.
* Remove dead code, stale TODO lists and internal notes that would confuse contributors; move agent working files (`PROGRESS.md`, `PLAN.md`, `MIGRATION*.md`, `DECISIONS.md`) into `dev/` and reference them from `CONTRIBUTING.md` as the project's design history.

1.2 Community files
`README.md` (§9.1), `GUIDE.md` (§9.2), `LICENSE-MIT`, `LICENSE-APACHE`, `LICENSE-ASSETS` (CC BY 4.0), `CONTRIBUTING.md` (setup, architecture tour, coding standards, how content data works, how to add a species/plant/process/era, tests, PR checklist), `CODE_OF_CONDUCT.md` (Contributor Covenant), `SECURITY.md` (private reporting, supported versions; network code and WASM sandbox are in scope), `PRIVACY.md` (§1.4), `CHANGELOG.md` (Keep a Changelog format), `ROADMAP.md` (implemented vs coming later, by era and system), `.github/` issue templates (bug, crash, feature, content realism correction, performance), PR template, labels including `good first issue` and `realism`, and `CODEOWNERS` placeholder for the owner.
1.3 CI and releases (GitHub Actions)

* CI on every push and PR across Windows, macOS and Linux: `fmt`, `clippy -D warnings`, tests, `hearth content lint`, `cargo deny`, docs build, headless smoke test (create world, simulate a game day, save, reload), the multiplayer bot-client test at small scale (§3.10), and a guide-accuracy check (§9.3). Cache builds. Network-dependent tests use mock providers only; CI never calls real AI services.
* Release workflow on version tags (`v0.1.0`…): builds and attaches
   * Windows x86_64: a `.zip` (unzip and run) and an installer (MSI via WiX or Inno Setup, unsigned by default).
   * macOS universal (arm64 + x86_64): a `.dmg` with an app bundle. Code signing and notarization run only if the owner adds Apple credentials as repository secrets; otherwise ship unsigned with clear "how to open an unsigned app" instructions.
   * Linux x86_64: a `.tar.gz`, an AppImage, and a Flatpak manifest in the repo.
   * Dedicated server binaries for all three platforms and a Docker image published to GitHub Container Registry, with a `docker-compose.yml` example.
   * SHA-256 checksums, an SBOM (CycloneDX), auto-generated release notes from the changelog.
* Versioning: SemVer 0.x. Save format and network protocol versions are separate numbers, shown in the main menu and the server browser, and checked on connect (§3.8).
* Update check: opt-in on first run; when enabled, checks the GitHub Releases API at launch and shows "a new version is available" with a link. No auto-install.

1.4 Privacy and safety defaults

* No telemetry. Crash reports are written locally with a "copy report" and "open GitHub issue" button that pre-fills a template; nothing is uploaded automatically.
* Microphone, AI providers and voice providers are off until the player turns them on, with a plain explanation of where data goes. `PRIVACY.md` documents all data flows: what's sent to which provider when configured, what a server receives, what's stored on disk.
* API keys are stored in the OS keychain (`keyring` crate), or read from environment variables; never written to saves, logs, config files shared with servers, crash reports or screenshots, and never sent to other players.

1.5 Easy setup (players who've never used GitHub)

* Download from the Releases page, unzip or install, double-click. No Rust, no command line.
* First-run wizard: language → hardware check (GPU and VRAM detection, picks a graphics preset, runs a 10-second benchmark flythrough and offers to adjust) → controls overview → optional AI setup (§4.6) → optional microphone setup (§5.6) → character creator → main menu. Every step is skippable and reachable later in Settings.
* System requirements (minimum/recommended), measured on real configurations and stated in the README; Linux runtime dependencies (Vulkan driver, ALSA/PipeWire, udev) listed with one-line install commands per major distro.
* Troubleshooting built in: a Settings → Diagnostics page shows GPU/backend in use, driver info, audio devices, network test to a server, AI/voice provider test buttons, and "copy diagnostics".
* For developers: `git clone`, `cargo xtask run` works on all three platforms; `CONTRIBUTING.md` lists anything else needed.

2. Multiplayer: overview of requirements
A full multiplayer mode, playable on LAN and over the internet, with the whole simulation (planet, ecosystems, realistic humans, physics, building) running authoritatively on a server. Targets:

* Host & Play (listen server) from the main menu: open your single-player world to friends in two clicks.
* Dedicated server: headless binary and Docker image with a config file, for always-on worlds.
* Join by address, by invite code (§3.7), via LAN discovery, or from a community server list (opt-in).
* Windows, macOS and Linux play together.
* Realistic player counts on the reference machine (8-core CPU), measured and published in `BENCHMARKS.md`: aim for 8 players on a listen server and 32 on a dedicated server with players spread across the planet, given the heavy ecosystem and human simulation. More if they stay close together (their simulation areas overlap).

3. Multiplayer: design
3.1 Architecture

* The existing client/server split (v1 §2) becomes a real network boundary. Single-player runs an internal server on a background thread over an in-memory transport using exactly the same protocol as online play, so single-player and multiplayer can't diverge.
* Transport: QUIC via `quinn`: TLS 1.3 encryption, reliable ordered streams for control and world data, unreliable datagrams for movement and voice. One UDP port (default configurable).
* Messages: versioned schemas (serde + `postcard` or `bincode`), compressed (`zstd`) for world data, with a protocol version handshake.
* Server-authoritative: the server owns all simulation state. Clients send intents (move, act, speak, craft, build); the server validates them (reach, line of sight, knowledge, tool, materials, rate limits) before applying them. Nothing a client claims is trusted.

3.2 World synchronization

* Worldgen and LOD generation are deterministic from the seed (v1 §6.1), so clients generate terrain and LOD locally and the server sends only modifications: per-cube deltas since generation, with a content hash per cube so clients detect mismatches and request a full cube. Player-modified regions send downsampled LOD updates so distant builds and clearings show up for everyone.
* Version- and mod-mismatched clients are refused with a clear message (§3.8).
* Plants, animals and humans near each player are streamed through an interest management system: what's relevant to a client depends on distance, line of sight and audibility, with priority and bandwidth budgets per client.

3.3 Entities and feel

* Snapshot + delta compression for entity state, interpolation on clients, client-side prediction and reconciliation for the local player's movement and actions so controls feel immediate at 100–150 ms ping.
* Lag compensation for hits (thrown spears, arrows, melee): the server rewinds entity positions to the shooter's view time within a bounded window.
* Animations, emotions, speech acts and sounds replicate as events. Long-running processes (crafting, building, tanning) run on the server with progress replicated.

3.4 Simulation with many players

* Each player has a full-tier simulation region (v2 §3.7, V2.1 §17). Overlapping regions merge. Budgets are enforced per region, and the server degrades gracefully (lower update rates for far agents, fewer instanced ambient animals) instead of dropping below 20 TPS. A server performance panel (admin) shows tick time by system and by region.
* The household and demographic tiers run once for the whole world; Observer mode (V2.1 §15.4) is available to admins and spectators only, and time-control is admin-only.

3.5 Time, sleep and death in multiplayer

* Sleep policy (server setting): All sleeping (default; time accelerates only when every online player is asleep or resting), Majority, or No acceleration (sleep is a fade that restores fatigue while time passes normally). Players who sleep while others are awake see a brief "resting" fade and wake when they choose.
* Players' characters are Persons in the world (V2.1). When a player logs off, their character stays where they were by default as a sleeping person (server option: disappear, or stay in a safe camp state). Their body is still in the world: weather, hunger and danger apply at a reduced, server-configurable rate.
* Death follows the world's death rules (v2 §9.8, V2.1 §16) per player.

3.6 Players, people and society

* Each player is a distinct Person with their own genome, knowledge, relationships and reputation. Agents remember each player individually.
* New players choose their spawn on the globe (v2 §16) if the server allows; otherwise the host sets spawn regions. Hosts can let new players join an existing player's group as an arriving adult.
* Player pair bonds require explicit mutual confirmation from both players through a UI prompt, follow V2.1 §1.3 (abstracted, no sexual content), and can be dissolved by either player. Children of player pairs are agents raised in the world; players can later continue as their adult children (V2.1 §16).
* PvP setting: Off / Consent (both players must enable, e.g., for duels or alliances at war) / On. In every setting, children can never be harmed (V2.1 §1.2).
* Text chat: proximity chat (spoken in-world, subject to the speaker's language like speech; §5.4) and an optional out-of-character global channel (server setting).

3.7 Hosting and connecting

* Direct connect by IP/hostname and port.
* LAN discovery via mDNS.
* UPnP/NAT-PMP automatic port mapping on Host & Play, with a clear success/failure message and an explanation of manual port forwarding.
* Invite codes: short codes that encode the address (and, if a relay is used, the relay route) for easy sharing.
* Relay (optional, self-hostable): ship an open-source `hearth-relay` binary for players behind strict NAT. No official relay is assumed: the project has no infrastructure. Document how to run one.
* Community server list (opt-in): an open, documented protocol and a tiny reference list server any community can host; the client can add list URLs. Off by default.
* Identity: no central accounts. Each player has a local Ed25519 keypair as their identity (shown as a short fingerprint), plus a display name. Servers use allowlists, passwords and bans by key.

3.8 Mods and versions

* On connect, the server sends its required data packs, resource packs and WASM mods with hashes and sizes. The client downloads missing ones after asking the player, verifies hashes, and keeps them per server. WASM mods run server-side; client-side mods are limited to resource packs and UI-only mods. Mismatched game or protocol versions get a clear explanation and a link to the right release.

3.9 Administration and safety

* Roles: owner, admin, moderator, player, spectator. Commands and an in-game admin panel: kick, ban (by key), mute (text and voice), allowlist, teleport, time and weather (admin), world backup now, view reports, server performance.
* Per-player block and mute for every player (text and voice), and report (writes a report with recent chat and voice metadata, not audio, to the server's admin log).
* Rate limits on actions and chat; flood protection; optional text filter (server setting).
* Anti-cheat through authority: movement validation, action validation against knowledge, reach, materials, timing; no client is trusted with simulation results. Saves and backups are server-side only.
* Server config `server.toml` documented in the guide's hosting section with every option, defaults and an example.

3.10 Multiplayer testing

* Bot clients: headless clients that play scripted sessions (walk, gather, craft, build, talk to agents, fight animals) used in CI (4 bots, short) and soak tests (32 bots, hours).
* Network conditions: a simulated-network layer for tests (latency, jitter, loss, reordering, bandwidth caps).
* Desync detection: periodic state hashes between server and clients for replicated state; mismatches log diagnostics and force resync.
* Determinism tests from V2.1 also run with a networked server.

4. Connecting your own AI model or agent to the realistic humans
V2.1 §10.4 defined an optional `ConversationBackend`. This milestone turns it into a full, provider-agnostic AI Bridge that's easy to set up and safe.
4.1 What AI can do (capability levels, chosen per world)

1. Off (default): built-in speech-act system and templated lines (V2.1 §10). The game is fully playable.
2. Dialogue: the AI phrases agents' speech acts as natural, era-appropriate lines, and turns the player's typed or spoken words into speech acts (V2.1 §10.4).
3. Deliberation (experimental): for agents near players, at low frequency, the AI may choose among the agent's own candidate goals and speech acts that the built-in mind has already generated and ranked (it can't invent actions outside that list), and may write short reflections that become memory entries in the agent's own words. Everything the AI decides is validated, recorded in the world's decision journal and replayed from it when the save is reloaded, so worlds stay consistent. Worlds that use Deliberation are labeled "AI-influenced".

4.2 Providers
Built-in adapters, each configured with endpoint, model name, key (if needed) and limits:

* OpenAI-compatible Chat Completions API: covers many hosted services and local servers (e.g., Ollama, LM Studio, llama.cpp server, vLLM).
* Anthropic Messages API.
* Ollama native API (for model listing and pulling).
* External agent via the Agent Bridge protocol (§4.3), for any agent program or framework. Don't hard-code model names. Fetch model lists where the provider supports it and let the user pick.

4.3 Agent Bridge protocol (for "your agent of choice")

* A documented, versioned JSON-RPC over WebSocket protocol (and the same tools exposed as an MCP server mode), so any agent framework, script or AI assistant can attach to the game and act as the mind behind agents' dialogue and deliberation.
* The game sends a request containing only what the agent is allowed to know (§4.4) and a JSON schema of allowed responses; the external agent replies with a structured result; the game validates it.
* The bridge listens on localhost by default; remote connections require explicit enabling and a token. In multiplayer, only the server host can attach a bridge.
* Ship examples in `examples/agent-bridge/`: a minimal Python client, a minimal TypeScript client, and an MCP configuration example. Document in `docs/ai/agent-bridge.md`.

4.4 Guardrails (mandatory; extend V2.1 §10.4)

* Knowledge-limited context: the AI receives only the agent's own knowledge, beliefs, personality summary, emotional state, relationships with those present, culture, era vocabulary and recent memories. Never world truth the agent doesn't know.
* Structured output validated against a JSON schema; invalid or slow responses fall back to the built-in system silently.
* Filters: an anachronism and knowledge filter (no words, concepts or technology outside the agent's knowledge state and era) and a content filter enforcing V2.1 §1 (no sexual content, nothing involving harm to children, no slurs or real-world group stereotypes, no cruelty). Prompt templates also state these rules.
* The simulation changes only through validated speech acts and choices from the agent's own candidate list. Free text never changes state.
* Prompt templates are data files in `data/hearth/ai/` (moddable, versioned), tested with recorded fixtures and a mock provider in CI.

4.5 Cost, speed and reliability

* Per-provider limits: requests per minute, concurrent requests, max tokens, monthly token budget with a warning; an estimate of usage in the AI settings screen.
* Latency budget (e.g., ~1.5 s for a dialogue line; configurable): if exceeded, use the template line and update if the AI reply arrives in time to still make sense.
* Caching of repeated phrasing, batching of low-priority requests, priority to agents talking with players.
* Works on low-end setups: recommend small local models in the guide, and keep Dialogue useful with small models.

4.6 Setup UX
Settings → AI screen (also in the first-run wizard): choose provider from a list, paste key or point to a local server, click Test (sends a harmless sample request and shows the reply and latency), pick capability level, set budgets. A "Use a local model" helper detects Ollama/LM Studio running locally. A clear note explains that hosted providers receive the agent context described in §4.4 and that this is subject to the provider's terms.
5. Voice: proximity chat, talking to agents, agent voices
5.1 Player-to-player proximity voice

* Capture with `cpal`; noise suppression (`nnnoiseless`, RNNoise-based), automatic gain, voice activity detection; echo cancellation where a suitable crate works on all platforms (otherwise document "use headphones" and keep the interface ready for it).
* Codec: Opus (mono, 16–32 kbps, 20 ms frames), with a jitter buffer and packet-loss concealment.
* Transport: QUIC datagrams to the server, which forwards voice to clients within hearing range of the speaker's in-world position (no voice for players far away). Positions come from the server, so nobody can eavesdrop from across the world.
* Spatial audio: HRTF spatialization in the existing audio engine, realistic distance roll-off, occlusion through blocks (the v1 muffling), reverb in caves and halls, underwater muffling. Your voice sounds like it comes from your character's mouth.
* Loudness is in-world noise: speaking volume (measured from the mic level) sets how far your voice carries, and animals hear it too (v2 stealth). Whisper and shout modes (keys) make this explicit; shouting carries far and scares wildlife.
* Controls: push-to-talk (default key V, rebindable) or voice activation with a threshold; mic level meter and loopback test in settings; input and output device selection; per-player volume, mute and block.
* Indicators: speaking icons over characters and in the player list; your own "mic live" indicator always visible when transmitting.
* Server settings: voice on/off, maximum range, allow shouting, admin mute.

5.2 Talking to agents with your voice

* Speech-to-text runs locally by default (`whisper-rs` / whisper.cpp with a small model downloaded on request, sizes and licenses shown), or via a configured provider. Only the transcribed text is sent to the server, never the raw audio of your voice to AI providers unless you choose a hosted STT provider.
* The text is parsed into a speech act (Dialogue level) or matched to the speech-act wheel (Off level).
* Realism rule: you speak in your real language, but the agent understands you only as well as your character knows the agent's language (V2.1 §10.3). With poor language knowledge, agents hear fragments and rely on gestures and tone.

5.3 Agent voices

* A `VoiceBackend` trait for text-to-speech, chosen in Settings → Voice:
   * Vocalizations (default): wordless vocal sounds with emotion and age (sighs, laughs, calls, gasps), plus subtitles.
   * Local TTS: e.g., Piper or eSpeak NG, with phoneme input, so agents speak their generated language aloud with its own sounds (V2.1 §10.1). Subtitles show the translation to the extent the player character understands.
   * Hosted TTS via a configured provider, or through the Agent Bridge.
* A stable voice for every person: voice parameters derived from the Person (age, sex, body size, a genetically inherited timbre seed so relatives sound alike), with emotional prosody from the agent's current emotions (V2.1 §5.2). Children sound like children; elders sound like elders.
* Accessibility option: "Hear speech in my language when my character understands it."
* In multiplayer, the server sends speech acts and text; each client synthesizes speech locally with its own voice backend (deterministic voice parameters per agent), so nobody streams agent audio and each player chooses their own setup.

5.4 Text chat as speech
Proximity text chat counts as speaking aloud in-world: it's heard only within range, appears as subtitles at the speaker, and agents perceive it like voice.
5.5 Voice rules (non-negotiable)

* No cloning of real people's voices. Agent voices are synthetic. The game never records, stores or uses other players' voices to train or clone a voice. Voice chat isn't recorded by default; reports contain metadata only.
* Mic off by default; clear consent at setup; obvious indicators when transmitting.
* Only bundle or auto-download speech models whose licenses allow it; show each model's license before download.

5.6 Setup UX
Settings → Voice & Microphone (also in the first-run wizard): choose devices, test mic with a level meter and playback, pick push-to-talk or voice activation, choose STT (local model download with size shown, or provider), choose agent voice backend and preview voices.
6. Accessibility and quality-of-life for a public release
Subtitles and captions for all speech and important sounds (on by default), colorblind-safe UI options, text size, remappable everything (already in v1), hold-to-toggle options, reduced motion, screen-reader labels for menus where the UI framework allows, and a Guided HUD and Hardy realism preset promoted in the first-run wizard for new players.
7. Performance and stability for release

* Re-run all v1/v2/V2.1 benchmarks with networking, voice and AI enabled; publish results in `BENCHMARKS.md` with the hardware used.
* Long soak tests: 8-hour single-player run, 4-hour 32-bot dedicated server run, with memory and tick-time graphs; fix leaks and spikes.
* Crash handling: panics on the server never corrupt saves (atomic writes, backups); the client shows a friendly crash screen with the local report.

8. Branding and press kit (for the release page and trailer)

* An original logo (vector, from the game's own visual style), an icon set for all platforms, a color palette and a font pairing (open-licensed), in `branding/` with `BRANDING.md`.
* A press kit in `press/`: short and long descriptions, feature list (implemented features only), 12–20 high-quality screenshots captured by the screenshot suite across eras, biomes, seasons and times of day, GIFs for the README, key art, the trailer files (§10), system requirements, and license notes.

9. The README and the plain-language guide
9.1 `README.md` (the front door)
In this order:

1. Logo, one-line pitch, a short GIF or trailer thumbnail linking to the trailer.
2. What it is in three or four sentences.
3. Download & play: big links to the latest release per platform, then three steps.
4. Screenshots (6–8).
5. What you can do (bullet list of implemented features; "coming later" list separate).
6. Play with friends (Host & Play, joining, dedicated server in one paragraph each, with links).
7. Bring your own AI and voice (one paragraph, link to the guide section).
8. System requirements.
9. How the world works: a short overview with links into `GUIDE.md` sections.
10. Modding, Contributing, Building from source, License, Privacy, Credits.

9.2 `GUIDE.md` (how every system works, in simple terms)
A complete player's guide, also built into a docs website (mdBook, deployed to GitHub Pages by CI) and shipped in-game as a searchable Field Guide from the pause menu.
Writing rules: plain language, short sentences, everyday words, about an 8th-grade reading level (CI checks the readability score per section and fails above the target); explain what it is, why it matters, how to use it, and tips; start every section with an "In short:" one-liner; use concrete examples ("a naked person in cold rain gets hypothermia within a couple of hours of game time"); screenshots or GIFs from the screenshot suite; no jargon without a one-line explanation; label anything not implemented as "Coming later".
Sections (one per system, in this order):

1. Getting started (first hour: what to do, what to avoid).
2. Controls (generated table, §9.3).
3. The planet: globe, latitude, wrapping world, poles, time zones, choosing where to start.
4. Time and seasons: day length, the calendar, the two time scales explained simply, sleeping.
5. Weather and climate: why places are hot, cold, wet or dry; storms, fog, snow, wildfire.
6. Your body: hunger and nutrition, thirst and safe water, body temperature, sleep and fatigue, stamina, injuries and illness, healing, death and continuing as a new person or your child.
7. Carrying and clothing: hands, belts, containers, weight, clothing warmth.
8. Making things: processes, knapping and other crafts, workstations, tool quality.
9. Learning and knowledge: discovery, the journal, skills, learning from others.
10. The technology path: era by era, what each step needs and why it's hard (from stone flakes to iron).
11. Fire, food and cooking: making fire, cooking, preserving food.
12. Rocks and resources: reading the land, where things are found and why, prospecting, mining.
13. Plants and trees: species, seasons, edible and poisonous plants, felling trees, regrowth.
14. Animals and ecosystems: food webs, herds, predators and how to deal with them, hunting and tracking, domestication.
15. The sea and fresh water.
16. Building: support and strength, shelters that actually keep you warm and dry.
17. People: genes and families, personality, emotions, culture, language, how strangers are treated, joining a group, reputation and norms, family life.
18. Eras and history: choosing an era, what people are like in each, Observer mode.
19. Multiplayer: hosting, joining, invite codes, LAN, dedicated servers (`server.toml` reference), relays, roles and moderation, PvP settings, sleep rules.
20. Voice and microphone: proximity chat, whisper/shout, talking to agents with your voice, agent voices, privacy.
21. AI: what connecting a model does, capability levels, step-by-step setup for a local model and for a hosted provider, the Agent Bridge for developers, costs and privacy.
22. Settings and performance: graphics presets, render and LOD distance, what to lower on slower PCs.
23. Modding (short, links to `docs/modding/`).
24. FAQ and troubleshooting (GPU, audio, mic, network, AI connection, unsigned apps on macOS and Windows SmartScreen).

9.3 Keeping the guide true

* Tables of controls, settings, server options, AI providers, eras and implemented species/technology counts are generated from the game's data and code by `cargo xtask docs` and committed; CI fails if they're stale.
* Every guide claim that references a feature carries an anchor comment (`<!-- feature: id -->`); a CI check verifies each id exists in the feature registry and is marked implemented, so the guide can't describe things the game doesn't do.
* Screenshots are regenerated by the screenshot suite on release.

10. The trailer
Produce trailers that make people want to download and play, built entirely from real in-engine footage captured with a reproducible, scriptable cinematic system. Never show features that aren't implemented; footage must reflect real gameplay and visuals (a disclaimer card: "All footage captured in-engine").
10.1 Cinematic capture system (`hearth_cinema`)

* Shot scripts in RON: camera keyframes on splines (position, rotation, FOV, focus, depth of field, motion blur), easing, time-of-day and weather control, seed and location, scripted agent and animal direction (e.g., a herd crossing, a family at a fire, a predator stalk), player-character actions, and audio cues.
* Offline render mode: fixed timestep, deterministic simulation, supersampling and accumulation for motion blur, render to an image sequence at 3840×2160, 60 fps (configurable), with offline audio rendering of the game mix (positional ambience, animals, footsteps, fire, weather) in sync.
* Edit and encode: an edit decision list (EDL) in data cuts shots to music beat markers (a cue file), adds title cards (game font, branding), crossfades and a final logo; `cargo xtask render-trailer` produces everything with `ffmpeg`: H.264 and H.265/AV1 masters, 4K60 and 1080p60, loudness-normalized to about −14 LUFS integrated (web video standard), plus burned-in and `.srt` captions.
* If the build environment has no GPU or can't render 4K in reasonable time, render a 1080p version, and document the one command the owner runs on their own machine to produce the 4K master.

10.2 Music
An original score made with the project's audio tools or a clearly licensed CC0 / CC BY track (license and attribution recorded in `ASSETS_LICENSES.md` and the video description). Because the edit is beat-mapped from a cue file, the owner can swap in a different track and re-render, and the cuts follow the new beats.
10.3 Deliverables

1. Launch trailer (~90–120 s): the main excitement piece.
2. Systems deep-dive trailer (~4–6 min): an in-depth tour of how the world works, with short captions; shows each major system in action (planet, seasons, body, crafting and discovery, ecosystems and predators, building, realistic humans, eras, multiplayer and voice, AI).
3. Short cuts: 30 s (16:9) and 15 s vertical (9:16) for social media.
4. Thumbnail and key art (high contrast, readable at small size), and a text file with title, description (features, download link placeholder, license and music credit) and tags for video platforms.
5. README GIFs from the same shots. All in `trailer/` (scripts, EDL, cue files, captions) with the rendered videos attached to the GitHub release (not committed to git).

10.4 Launch trailer structure (optimize for excitement)
Principles: hook within the first 3–5 seconds; show, don't tell; text cards of five words or fewer; cut on the beat; build from quiet to huge; contrast intimate moments with vast ones; end on the strongest shot, then logo and a clear call to action; readable with sound off (captions, big visuals).
Beat sheet (adapt to what looks best once you see real footage; record changes in `trailer/STORYBOARD.md`):

1. 0–5 s, hook: extreme close-up of hands striking a flint flake in silence, sparks/fragments, then a hard cut to a camera rising from a lone figure in a loincloth on a cliff to reveal an enormous horizon with the planet's curve and distant mountain ranges above the clouds.
2. 5–20 s, the world: sweeping shots across contrasting places on one planet (glacier fjord, rainforest canopy at dawn, desert dunes, savanna with a migrating herd, a coral shallows sunbeam, a starlit snowfield). Card: "One whole planet."
3. 20–35 s, survival: breath fog in a snowstorm, shivering by a struggling fire, drinking at a river, a hunt through tall grass downwind of deer. Card: "Survive like we did."
4. 35–50 s, danger: a predator stalk sequence (eyes in brush, a wolf pack flanking, a bear rearing, a crocodile at a water hole), cut fast on the beat; the music drops to silence for one beat.
5. 50–70 s, progress: a time-compressed sequence of discovery: flake → fire by friction → hafted spear → sewn furs → pottery fired in a kiln → glowing copper in a crucible → iron hammered on an anvil. Card: "From stone to iron."
6. 70–85 s, people: realistic humans: a child learning to knap by watching an elder, a family at a fire telling stories, a stranger greeted warily, a village in a valley at dusk. Card: "Real people. Real history."
7. 85–100 s, together: friends in multiplayer building a shelter, proximity-voice moment (on-screen voice indicator, a whispered line, then a shout echoing across a canyon), Observer-mode globe showing cultures spreading. Card: "Play with friends."
8. 100–115 s, finale: the most beautiful shot in the game (e.g., a sunrise through fog over a forested valley with a herd and smoke from a camp), music peaks, cut to black.
9. Logo + call to action: name, "Free and open source", "Download on GitHub", platforms. Small card: "All footage captured in-engine."

10.5 Review loop
Render a low-resolution draft first; review every shot frame-by-frame for glitches (LOD pops, z-fighting, clipping, animation errors, UI leaking into frames), fix the engine or the shot, re-render. Write `trailer/REVIEW.md` with what was fixed. Only then render the masters.
11. Data and code layout additions

```
crates/
  hearth_net        transport (QUIC), protocol, replication, interest management, prediction
  hearth_dedicated  headless dedicated server binary
  hearth_relay      optional self-hostable relay
  hearth_voice      capture, DSP, Opus, jitter buffer, spatial voice, STT/TTS backends
  hearth_ai         AI Bridge: providers, Agent Bridge protocol (WebSocket + MCP), guardrails, decision journal
  hearth_cinema     shot scripts, offline render, EDL, trailer pipeline
xtask/              build, package, docs generation, trailer rendering, rename
data/hearth/ai/     prompt templates, response schemas, filters
docs/               mdBook source: guide, hosting, ai, voice, modding, architecture
examples/agent-bridge/  Python, TypeScript and MCP examples
branding/  press/  trailer/

```

12. Tests and checks added by this amendment

* Protocol round-trip and versioning tests; network-condition tests; bot-client CI test; desync hash checks; dedicated-server smoke test in Docker.
* Voice: codec round-trip, jitter buffer under loss, spatial range gating (no audio delivered beyond range), push-to-talk and mute behavior, no-recording check.
* AI: mock-provider tests for every adapter; schema validation; guardrail tests (attempted knowledge leaks, anachronisms, prohibited content, actions outside the candidate list are all rejected); decision-journal replay determinism; latency fallback.
* Docs: generated tables fresh, feature anchors valid, readability targets met, links not broken.
* Release: CI builds all artifacts on a test tag in a fork-safe way; installers launch the game to the main menu in CI on each OS (headless where possible).

13. Milestones
Run these only after the game is complete (§0.1), in this order: R1 → R2 → R3 → R4 → R5 → R6 (Phase R-A, online features), then R0 → R7 → R8 → R9 → R10 (Phase R-B, release). They're listed below by number. Each milestone: implement, document, test, run all checks, update `PROGRESS.md`, commit.
R0 — Repository audit and open-source foundation. §1.1–1.2, §1.4, license files, `cargo deny`/`cargo about`, secrets scan, clean-room scan, `xtask`, devcontainer, `scripts/rename`, `RELEASE_PLAN.md`. Accept: a fresh clone builds and runs on all three platforms in CI; no secrets or foreign trademarks in the tree; licenses complete.
R1 — Networking core. `hearth_net` with QUIC, protocol and versioning, the in-memory transport for single-player, server-authoritative intents, handshake and identity keys. Accept: single-player runs through the network layer with no regressions in the v1/v2 benchmarks.
R2 — Replication and world sync. Seed-plus-deltas cube sync with hashes, LOD updates, interest management, snapshots and deltas, prediction and reconciliation, lag compensation, replicated processes, plants, animals and humans. Accept: 4 bot clients play for an hour under 150 ms latency and 2% loss without desyncs; controls feel immediate.
R3 — Hosting, joining and administration. Host & Play, dedicated server and Docker image, `server.toml`, LAN discovery, UPnP, invite codes, relay, community list protocol, roles and admin panel, block/mute/report, PvP and sleep settings, mod sync, multiplayer rules for logging off and death. Accept: a 32-bot soak test on a dedicated server meets budgets; join flows work across platforms.
R4 — AI Bridge. Providers, capability levels, Agent Bridge (WebSocket + MCP) with examples, guardrails, decision journal, budgets and fallbacks, AI settings UI and wizard step. Accept: §12 AI tests; a local-model setup takes under five minutes following the guide; the game behaves identically with AI Off.
R5 — Proximity voice. Capture, DSP, Opus, transport, server range gating, spatial audio with occlusion and reverb, whisper/shout and in-world noise, controls and indicators, settings and wizard step. Accept: §12 voice tests; two players across a cave wall hear each other muffled; a shout scares a nearby herd.
R6 — Voice for agents. Local and hosted STT, speech-to-speech-act with language-knowledge gating, `VoiceBackend` with vocalizations, local phoneme TTS speaking generated languages, hosted TTS, per-person stable voices with emotional prosody, client-side synthesis in multiplayer. Accept: talking to an agent by voice works end-to-end with local models only; relatives sound alike.
R7 — Packaging, CI and releases. §1.3, §1.5 (first-run wizard, diagnostics), update check, system requirements measured. Accept: a test tag produces all artifacts; each installs and runs on a clean VM per OS; a non-developer can go from the Releases page to playing in under five minutes.
R8 — README, guide, docs site and in-game Field Guide. §9 in full, with generated tables, feature anchors and readability checks. Accept: all docs checks pass; every implemented system has a guide section; nothing unimplemented is described as available.
R9 — Branding, press kit and trailers. §8 and §10: cinematic system, all trailer cuts, thumbnail, descriptions, review loop. Accept: trailers rendered and reviewed; `trailer/REVIEW.md` complete; licenses recorded.
R10 — Launch readiness review. Fresh-machine install tests on all three platforms, full guide walkthrough by following it literally, security review of network code, AI bridge and WASM sandbox, privacy review against `PRIVACY.md`, performance re-check, and a final `dev/LAUNCH_REPORT.md` listing what's ready, known issues, and the owner decisions from §0.5 still to confirm. Prepare (but don't publish) the v0.1.0 release draft and changelog.
After R10, the project is in maintenance and future-era mode: any new milestone (e.g., the planned Medieval and later eras) must keep multiplayer, the guide, the docs checks and the trailer shot scripts current as part of its definition of done.
14. Resume protocol additions
Keep all earlier protocols. Also keep current: `RELEASE_PLAN.md`, `BENCHMARKS.md`, `docs/`, `trailer/STORYBOARD.md` and `trailer/REVIEW.md`, and a Release Status table in `PROGRESS.md` (platform builds, multiplayer features, AI providers, voice features, docs coverage, trailer status). On restart, also run the bot-client multiplayer test and the docs checks.
Begin with §0.2: if the game isn't finished yet, file this amendment and go back to building the game. The bar for this amendment: a stranger finds the GitHub page, watches the trailer, downloads it, plays within five minutes, invites a friend, talks to them by voice around a fire, and plugs in a model so the people of the valley talk back, all without ever opening a terminal.
