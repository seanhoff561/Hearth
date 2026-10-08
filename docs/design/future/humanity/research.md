# Research notes

*The prior work Amendment E §10.4 asks to study, checked against the papers' own pages
(2026-10-08), with what each lends the design. Details not confirmed from the source are marked.*

## Minds and agents
- **Generative agents** — Park, O'Brien, Cai, Morris, Liang, Bernstein, "Generative Agents:
  Interactive Simulacra of Human Behavior", UIST 2023 (arXiv 2304.03442). Agents keep a
  natural-language **memory stream**, distil it into higher-level **reflections**, and
  **retrieve** memories to plan; plans begin as a day's outline recursively broken into
  shorter steps. Retrieval scores recency, importance and relevance. *Lends:* `04-mind.md`'s
  memory, reflection and retrieval.
- **1,000-person agents** — Park et al., "Generative Agent Simulations of 1,000 People", 2024
  (arXiv 2411.10109). Agents built from two-hour qualitative interviews of 1,052 people
  answered the General Social Survey about 85% as consistently as the people did two weeks
  apart, and predicted personality and experimental outcomes; interview-based agents showed
  smaller accuracy gaps across groups than agents built from demographics. *Lends:* grounding a
  mind in a rich life history (the Person record's life events and beliefs) rather than in
  group labels, which also serves `10-safety.md` rule 1.
- **Project Sid** — Altera.AL, "Project Sid: Many-agent simulations toward AI civilization",
  2024 (arXiv 2411.00114). The PIANO architecture runs an agent's modules (cognition, planning,
  motor execution, speech) concurrently over one shared agent state, with fast non-language
  modules for reflexes and a cognitive controller as a bottleneck for coherence; 10 to over
  1,000 agents in one world took specialised roles, changed collective rules and passed on
  cultural and religious ideas (preliminary results). *Lends:* `04-mind.md`'s shared person
  state and controller, and the warning that parallel modules drift apart without one.
- **Voyager** — Wang et al., "Voyager: An Open-Ended Embodied Agent with Large Language
  Models", 2023 (arXiv 2305.16291; TMLR 2024). An automatic curriculum, a growing **skill
  library** of executable programs, and iterative prompting with feedback from the world and
  self-verification. *Lends:* plan libraries that grow with each successful new plan.
- **AgentSociety** — Piao et al. (Tsinghua), "AgentSociety: Large-Scale Simulation of
  LLM-Driven Generative Agents…", 2025 (arXiv 2502.08691). More than 10,000 agents and about 5
  million interactions; used to study polarisation, inflammatory messages, a universal basic
  income and hurricane shocks, with outcomes the authors find consistent with real-world
  experiments. *Lends:* the scale argument for C0–C1 statistics plus few language-model minds;
  societal tests for `11-evaluation.md`.

## Societies and history
- **War and the evolution of complex societies** — Turchin, Currie, Turner and Gavrilets, PNAS
  110(41): 16384–16389, 2013 (doi 10.1073/pnas.1308825110). A cultural-evolutionary model on a
  gridded map of Afro-Eurasia in which military technology spreading from the steppe intensifies
  war and selects for institutions that hold large societies together reproduced 65% of the
  variance in where large polities arose 1500 BCE–1500 CE, against 16% without the diffusion of
  military technology. *Lends:* `02-worldbuilding.md`'s polity model on the region graph.
- **Secular cycles** — Turchin and Nefedov, *Secular Cycles*, Princeton University Press, 2009.
  The demographic-structural framework (after Goldstone): population, elites and the state in
  long oscillations, tested against England, France, Russia and Rome. Later tests are mixed
  (supportive for Chile, not for the recent United States). *Lends:* structural-demographic
  pressure as one input to polities' instability, not a law.
- **Demography and cultural evolution** — Henrich, *American Antiquity* 69: 197–214, 2004. In a
  model where learners copy the most skilled and fall short by a random amount, a population
  below a critical size loses complex skills while keeping simple ones (the Tasmanian case);
  critiques (Read 2006; Vaesen et al. 2016) argue the assumptions are strong and the record
  ambiguous. *Lends:* technology diffusion and loss tied to connected population size, as one
  factor with its uncertainty recorded.
- **Gravity models** — Tinbergen (1962) for trade (flows grow with the sizes of both ends and
  fall with distance), with earlier forms (Isard 1954, Ravenstein, Reilly) and archaeological
  interaction models (distance decay often squared; Renfrew and Level's XTent). *Lends:* flows
  between places before traders are instantiated.

## Planning and movement
- **HTN planning** — Nau et al., "SHOP2: An HTN Planning System", JAIR 20: 379–404, 2003.
  Tasks decomposed into subtasks down to executable operators, steps produced in execution order
  so the current state is always known. *Lends:* the planner of `04-mind.md`.
- **Continuum crowds** — Treuille, Cooper and Popović, SIGGRAPH 2006 (ACM TOG 25(3)): one
  dynamic potential field handles routing and avoidance for large crowds. *Lends:* crowds and
  armies.
- **ORCA** — van den Berg, Guy, Lin and Manocha, "Reciprocal n-body collision avoidance",
  Robotics Research (ISRR 2009), STAR 70: 3–19, 2011: each agent takes half the responsibility
  for avoiding each pairwise collision, its velocity from a small linear program; thousands of
  agents in milliseconds (RVO2). *Lends:* close-quarters avoidance.
