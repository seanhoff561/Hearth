# 4. A mind: System 1, System 2 and the body

*Amendment E §10.4. Prior work in `research.md`.*

## The body is an Actor
People use the body, action vocabulary, physics and animation players use (the Actor rule, E
§2.3; E7's body; Amendment R §9.3's interactions). The body executes **motor commands**:
- locomotion along planned paths;
- manipulation: gathering, work and crafting through the process engine (E6's strokes);
- social and combat actions: greet, wave, point, handshake, hug, kiss, comfort, carry, push,
  punch, kick, tackle, grapple;
- expressions: face, posture, gaze;
- speech playback with lip sync.
A player's input is one mind issuing intents to a body; a person's mind is another. Nothing a
person does is a special case of the engine.

## System 1: fast and reactive (about 5–20 decisions a second)
Chooses the next micro-action from the situation and the plan's current step: keep walking,
step aside, look at the speaker, flinch, return a greeting, block a blow, follow, comfort a
crying child.
- **A small learned action model:** a compact transformer over a tokenised situation (own
  state, nearby people and things, the plan step, emotion) emitting action tokens from the
  vocabulary; batched on the GPU for hundreds of people.
- Trained by imitation of designed behaviours, of rollouts annotated by System 2, and of human
  motion and social data.
- **Deterministic utility and behaviour rules** are the fallback and the safety net: always
  available, used when the model is absent, slow or proposes an action that fails a
  precondition.

## System 2: slow and deliberate (seconds to hours, event-driven)
A language model (local small or mid-size, or a hosted one the player configures) keeping the
person's goals, plans, beliefs, relationships and memories. It plans the day by role and need;
reacts to salient events (a death, an insult, a proposal, a rumour of war); decides what to say
and to whom; makes big decisions (move away, marry, quarrel, steal, build, go to war); reflects
at night.
- Outputs are **structured** (schemas): goals, commitments, dialogue intents and lines, belief
  updates, emotional appraisals.
- Each output is **validated** against what the person knows (the World Bible through their
  reach) and the world's facts; an invalid one is refused and asked again or replaced by the
  fallback.

## The planner, between the two
A hierarchical task-network planner over the game's real process, construction and movement
models turns System 2's goals into executable steps for System 1 and the body. **Language
models choose what to do; planners work out how**, so every plan is physically valid.
- Each culture and era has a **plan library** ("how we drive bison over the bluff", "how a
  feast is held") as HTN methods over the process engine; a new plan that succeeds is added (a
  skill library, as Voyager's).
- Plans are executed in order with the world's state tracked (SHOP2's forward decomposition),
  so a step's preconditions are checked against the world as it is when the step comes.

## Memory
- An **episodic stream** of observations and events with importance scores.
- **Retrieval** by recency, importance and relevance (Park et al. 2023).
- **Nightly reflection** into higher-level beliefs ("my brother can't be trusted with the
  herd"), triggered too when enough has happened.
- **Forgetting** and memory budgets by level (C2 keeps summaries, C3 a bounded stream, C4 the
  full recent stream).
- **Shared cultural memory** (myths, songs, law) held once in the World Bible and referenced.
- Beliefs can be false; people lie and are deceived; words never change world facts.

## Personality and emotion
Temperament is heritable (the archive's genetics, under rule 1 of `10-safety.md`) and shaped by
upbringing. Emotions come from **appraisals** computed deterministically (goal relevance,
congruence, agency, certainty) and feed both systems, the face and the voice.

## Coherence across modules
Running modules in parallel (motion, speech, planning) risks a mind saying one thing and doing
another (Project Sid's observation). One **shared person state** is the single source every
module reads and writes, and a **controller** (the planner's current commitment) gates which
proposals reach the body, so speech and action agree.
