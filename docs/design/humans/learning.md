# Knowledge and social learning (V2.1 §11; H6)

How the simulated people come to know what they know: from those who know it — by watching, by
being shown, by following a master, by hearing stories — or by finding it out; and how a band
keeps or loses what its people know (the collective brain). Written with H6.

## Knowing (§11.1)

Each person knows what it has learned (its knowledge state, as the player's: known nodes,
insight toward others, skills, legends), and a band knows what its living members know. Persons
work only what they know: a process whose knowledge one lacks is never offered it, and a debug
assertion guards every work begun (the anachronism guard).

## Learning across a life (§11.2, §11.4; H6 (a))

Each year of a band's life course (`learning.rs`; data `humans/learning/transmission.ron`):

- **The young learn from those who know**: each, for each technique it lacks whose groundwork it
  has, learns it with the chance `1 − (1 − f)^k`, where `k` counts the band's knowers (ten and
  over) and its neighbours' within twenty kilometres as far as they meet (a quarter at their
  nearest), and `f`, a learner's chance a year from one knower, is `learn_year · e^(−depth_factor
  · depth)` — the simple quickly, the deep (a technique resting on others, resting on others)
  slowly, as a complex skill is copied less faithfully. The young (our kind's from five to
  twenty-five) learn fully; the grown at three tenths.
- **Trying things**: a grown one now and then tries something new — twice its curiosity times a
  small chance a month — gaining insight toward a technique whose groundwork it has and that can
  be found by trying, and in time finds it out (`Discovered` in its life).
- **The band's knowledge** is then what its living members know, and the processes that opens. A
  technique none of them knows any longer is **lost**: the processes go with it, and its grown
  keep it as a story — a legend, learned four times as fast when they meet it again.

The result is Henrich's (2004) collective brain: a technique's knowers grow when the learners
about them times `f` outpaces their deaths, so a deep technique needs many learners — a large
band in touch with its neighbours keeps and spreads it, a small one cut off loses it within a
few generations (the Tasmanians' losses, Jones 1977). Simple techniques are kept by any band.

## In play (H6 (b))

- **Shown how**: one at its work shows how to the one watching it — of its own band, or a guest
  it trusts — giving insight straight toward the work's knowledge (or the first of what it rests
  on) at `0.05 · teach` a minute, `teach` its people's (four times for our kind, once — no
  teaching — for the australopiths); it says so now and then ("see this", showing). Grown ones
  watch a work they do not know too, the curious the more.
- **Apprenticeship**: each year a young one (eight to sixteen) takes a master — the grown one of
  its band most skilled in what it does not yet know, of its own household and sex the more
  readily — watches its master's work the more readily, and is shown half as fast again.
- **Stories**: in the evening's company the grown now and then tell a story to those sitting near:
  a place they are sure of (water, food, danger), which the hearers come to know less surely and
  less exactly; or a technique of their people's one of them does not know, which the hearers keep
  as a legend.
- **The player is taught**: one of the people at its work within three metres in front of the
  player, who would teach the player (of the player's own band, or trusting it), shows the player
  how as it would its own; the player's knowledge gains the insight, its journal the lesson. By
  watching alone the player takes in what it sees at most once an hour.

## The acceptance (H6 (c))

`tests/culture.rs`: a deep technique known by every grown one of a small band alone is lost within
a hundred and fifty years, and kept by a large band with neighbours; shown how by one at its work,
the player learns a work within half an hour that watching alone does not teach it.
