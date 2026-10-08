# 3. People: one person model, thought scaled by attention

*Amendment E §10.3.*

## The Person record
One record for everyone at every level of detail; levels add state, they never fork it.

| Part | Holds | Source |
|---|---|---|
| Identity | id, names (in their language), sex, birth date and place, parents | History Engine, lazy zoom |
| Appearance and genome | the genome (diploid loci, the archive's genetics generalised) and the appearance it gives | `hearth_character::Appearance` (E5, E7); genetics from the archive |
| Body | the same `hearth_body` state players have | `hearth_body` |
| Household and kin | household id, kin links, relatedness | the archive's kinship |
| Role and status | occupation, rank, offices | History Engine, institutions |
| Traits and values | temperament (heritable), values (cultural, upbringing) | genetics; culture |
| Skills and knowledge | known nodes and practice | `hearth_craft::KnowledgeState` |
| Relationships | the closest few in detail (affection, trust, debts, history); the rest as aggregates | people loop |
| Possessions | things owned, claims | the world's items and claims |
| Life history | events lived (ids into the event store) | event store |
| Beliefs | World Bible references plus private beliefs, which may be false | minds |
| Goals and memories | in summary at low levels; full streams at C3–C4 | minds |

## Cognitive levels of detail
| Level | Who | How they think | Scale |
|---|---|---|---|
| **C0 statistical** | everyone, as populations | demographic and economic models | billions over history; millions alive |
| **C1 sketch** | people instantiated in active regions | records plus routines by culture and role, resolved statistically | 10⁵–10⁶ per active region |
| **C2 procedural mind** | people near players | needs, emotions, a deterministic planner, routines, social rules; no language model per tick | ~10³ |
| **C3 reflective mind** | notable people, the player's kin, people whose day matters | C2 plus periodic slow thinking by a small, fast language model | ~10² |
| **C4 conversational mind** | people face to face with a player | C3 plus real-time conversation and voice with a stronger model | a handful at once |

## The attention allocator
Every second, under the compute budgets of `08-compute.md`, it scores each instantiated
person by proximity to players (distance and line of sight), interaction (being spoken to,
traded with, fought), ties to players (kin, friends, enemies), narrative importance (notable
roles, unfolding events) and urgency (danger, a decision pending), and assigns levels by
score with **hysteresis** (a person rises at a higher score than they fall at, and stays at
least a minimum time) so no one flickers between levels.

## Demotion and promotion
- **Demotion** consolidates: slow thinking writes a short reflection (what happened, what it
  meant, what they now want) into the record; detailed memory streams are trimmed to their
  budget; the body's state is kept.
- **Promotion** rehydrates: goals, recent memories and mood are retrieved from the record and
  the World Bible; a C3 or C4 mind starts from that summary, never from nothing.
- A C1 person who has never been C2 is promoted from their routine's expected state at the
  hour (where they are, what they carry), generated deterministically.
