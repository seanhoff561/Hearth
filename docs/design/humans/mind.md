# The mind

*V2.1 §6; milestone H2. Code: `crates/hearth_people/src/mind.rs` (choosing), `memory.rs`,
`plan.rs`, and the step in `sim.rs` (deciding and acting). The psyche that tilts it:
[psyche.md](psyche.md).*

**In short:** people act on what they perceive and remember, never on what the world knows.
Each step a person senses what is about them by daylight and hearing, catches the feelings of
those they see, and — when its last choice has run its time, danger interrupts or it has nothing
to do — chooses again among what its needs, feelings, tendencies and the moment offer, a plan's
next step among them when it means to make something.

## Perception (§6.2)

- **Sight** reaches 120 m by day and a quarter of that by moon and starlight, between them at dawn
  and dusk; a crouching, crawling player is seen nearer. A hunter beyond sight is not reacted to:
  a leopard fifty metres off in the dark goes unseen (`tests/memory.rs`).
- **Hearing:** an alarm call carries 300 m and frightens whoever hears it.
- **The others:** those of its band in sight are seen where they are, the others where it last saw
  them; the young keep to their mothers. Their shown feelings are caught within 25 m.
- Food is seen within 40 m; water, sleeping trees and anvils are known from memory.

## Memory and belief (§6.3)

- **The mental map.** Each person remembers places of its range — water, sleeping trees, food,
  anvils — and where it saw danger. A band's people start from what the band knows of its range;
  thereafter each map grows by its own days: a place come to is remembered by the one who came and
  by those of the band who saw it (within 30 m), and the band keeps it too. Places unvisited lose
  their hold over weeks (food soonest), and are let go when faint.
- **Beliefs can be wrong.** A remembered food tree may be bare when it gets there: it is let go and
  the person chooses again. Where a hunter was seen is believed dangerous — its water and food
  passed over — for a couple of days, fading as nothing more is seen there.
- **People known.** Everyone seen is known by sight, more familiar with each hour in each other's
  company (H4's relationships grow from it); players too, by identity.
- **Episodes.** What mattered — a fright, a hurt, a loss, first meeting a player, a feast — is kept
  with its weight (what was felt), the weightier the longer; the lightest are let go when there are
  too many. The defining ones go into the life history (`Met`, `Hurt`, `Mourned`), which stays
  when the episodes fade.

Memory is part of the person record (people format 4) and bounded: a dozen places of a kind, two
dozen episodes, four dozen people.

## Choosing (§6.1)

The layers, cheap to dear:

1. **Reflexes** — danger interrupts whatever is under way at once: flight up a tree or away,
   facing a hunter with the others, the alarm.
2. **Routines** — the day's shape: nests at dusk and down at dawn (camps and shelters for those
   who do not nest come with H3).
3. **Utility selection** — the needs (thirst, hunger, weariness), the feelings (a low mood or grief
   weighs toward rest), the tendencies (diligence toward work, sociability toward company), the
   moment's offers (a process at hand, food in sight) and the plan's next step, scored.
4. **Planning** — see below.
5. **Social reasoning** — with H4.

## Planning

A person with a goal — to have a thing — plans the steps to it on the player's process engine, as
deep as its species plans (*Australopithecus* one process deep, *Homo sapiens* six): a thing is had
if carried, else picked up where it lies in sight, else made by a process it knows — each of the
process's tools and inputs a thing to have in turn, its target a place to go to (a scatter of
stones, a pine for its resin, a stand of nettles). The plan's steps (go, take up, do) are done one
by one, each checked by the engine as it is done; a failed knapping or a thing gone makes the plan
again from where things stand, and three plans that come to nothing give the goal up.
