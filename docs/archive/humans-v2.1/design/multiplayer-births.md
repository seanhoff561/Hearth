> **Superseded by Amendment E** (`docs/spec/amendments-e-q.md`, 2026-10-08): the simulated humans were removed in E0 and are planned anew as Phase F (`docs/design/future/humanity/`). Kept for reference; nothing here is built, tested or linted.

# Births in multiplayer

*V2.1 Addendum B §3–4 (user direction, 2026-10-03); D168. Milestone: R3 (Amendment R, after the
game is finished), on records and rules the H milestones keep per player and on the server's side
from the start (D166). Supersedes Amendment R §3.6's spawning and arriving as an adult.*

**In short:** friends can start a world together as twins, siblings, cousins or neighbours and
grow up together; a player who joins a world already running is born too, living their
childhood as memories of a real young person of the world.

## Why it differs from single-player

In single-player, a childhood moves the whole world forward years at a time (Addendum A,
[player-birth.md](player-birth.md)). A shared world has one clock, so that can only happen while
everyone online is a child at once. Hence two ways to be born — a **shared start** and a
**remembered childhood** — and family links between players.

## Shared start

- When a world is created (or the host opens a new one) the host may choose **Start together**.
  Each player joining it picks a family arrangement in a short lobby:
  - **Twins:** born together into one household; fraternal by default, *identical* (one genome,
    one sex) only as an explicit choice — a liberty, real identical twins being rare.
  - **Siblings:** one household, born one to four years apart, spaced as the era's births are.
  - **Cousins:** children of siblings, in one group.
  - **Neighbours:** different households of one band or village.
  - **Apart:** each chooses their own place on the globe.
- Each still picks daughter / son / chance; looks still come only from the parents' genes and are
  never shown before.
- The births are drawn from **real households** fitting the arrangement; where none fits exactly
  the closest is taken, and the lobby says so. History generation makes room for them (the
  expected player count, below).
- **Shared childhood:** the server runs Addendum A's childhood for the whole group — the moments
  played **together** (exploring, learning from the same elders, into the same trouble), the
  years between passing for the whole world. It ends when the youngest player comes of age, or
  sooner if the host ends it (the children left then grow up at the world's pace).
- A player who misses a session stays a child of their family, cared for as any child is (a
  child agent following its routines); coming back resumes at the age reached.
- A player joining during the shared childhood may be born into the group at the next jump (a
  younger sibling, say) and goes on with the rest of the schedule.

## Remembered Childhood

For a player joining a world already running (or choosing to be born again in multiplayer):

1. **Choose a place** on the globe, within the host's allowed regions.
2. **Choose from two to four births** of real households there, made about thirteen to sixteen
   years ago (the culture's age of coming of age): each a real young person of the world's
   records who **no player has met** — no player or player's character has a relationship with or
   a memory of them — and who is of no other player's immediate family unless that player
   consents. Daughter / son / chance filters them; looks are not shown.
3. **Play their childhood as memories:** short playable vignettes (about 45–90 minutes in all by
   default; configurable; skippable) rebuilt from the person's recorded life in the household
   tier (V2.1 §15.2, §17) — their family, their home, the seasons they lived, the people who raised
   them. Each vignette runs in a **private, isolated instance** of that moment of the past, seeded
   from the record.
4. **What is done in memories counts, within limits.** It shapes what the young person learned
   (knowledge, skills), how warm their relationships are, and small details of their history; it
   cannot contradict fixed facts — who lived and died, where the family lived, the great events,
   and **anything any player witnessed**. No one dies or leaves the region in a memory. The
   results are committed to the record when the memories end.
5. **Wake in the present** as that young person, at home, among people who have known them all
   their life. To everyone else, other players too, they have always been there.

"Skip memories" takes the young person's recorded upbringing as it was. Memory instances are
seeded and isolated; only the record committed at the end touches the shared world.

## Born into a friend's family later

- **A sibling or cousin:** one of the friend's **existing** brothers, sisters or cousins of a
  fitting age — people the friend's character knows — with the friend's consent, through
  Remembered Childhood, the memories kept true to what the two have shared.
- **The same group:** a never-met young person of the friend's group; no consent needed.
- **A player's child:** a player with a partner (a pair bond, by mutual consent as always) may
  invite someone to be born as their child. The one invited waits out the pregnancy (shown as the
  time left at the world's pace) spectating or doing anything else on the server, and is born a
  newborn whose childhood passes at the world's pace — the real time it takes shown plainly
  before, a long commitment chosen deliberately.

## Server settings

In `server.toml` and the host's screens (R3): **birth regions** (anywhere / host-chosen regions /
near existing players), **shared start** (on / off), **Remembered Childhood**'s length and
whether it can be skipped, **family links between players** (allowed / same group only / off),
the **expected player count** (history generation makes enough fitting households — in Wild
Earth a few more wandering families, still far apart unless players choose to be born together),
and the death settings ([life-after-death.md](life-after-death.md)).

## Other multiplayer details

- **Logging off.** An adult character stays in the world asleep or resting somewhere safe (as
  Amendment R has it), or, by a new option, **lives on quietly** under the game's own mind: its
  routines, but no great decisions (no pair bond, no leaving the group, no risky hunt, no fight it
  can avoid). A child player character is always cared for by its family as any child is.
- **Child safety.** A player whose character is a child can be harmed by no one — player, agent
  or animal — and no adult player can target a child character in any PvP setting.
- **Families between players are real families:** genes (siblings look alike through their
  parents), kinship, inheritance and obligations work as for anyone; agents treat players'
  brothers and sisters as such.
- **Voice as a child:** proximity voice works as usual, and a child picks up the local language
  faster (V2.1 §7.3), the character's understanding included.

## What the H milestones keep for this

So R3 is wiring and not a rewrite, the H milestones keep, per player and on the server's side:
each player's own person record and its control (H3); which persons any player has met — a
relationship with or a memory of a player (H2, H4); the facts players witnessed, recorded with
their witnesses (H2, H8); and each young person's recorded upbringing in the household tier, the
stuff of their memories (H7, H8).

## Tests (Addendum B §6)

- Remembered Childhood never contradicts fixed facts or anything a player witnessed, never offers
  a person any player has met, and is deterministic for the same seed and choices.
- A shared start makes the family asked for (or the closest, reported), and the shared
  childhood's jumps advance the world alike for every connected client.
- A child player character can be harmed by no source in any PvP setting.
