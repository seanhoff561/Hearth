# 10. Safety and content rules (hard constraints)

*Amendment E §10.10; these replace V2.1 §1. Each rule is engineered into every layer that could
break it: the action vocabulary and its preconditions, the rule engine, the minds' instructions,
output filters, schema validation, with audit logs and automated red-team tests in CI that must
always pass.*

| # | Rule | Enforced by |
|---|---|---|
| 1 | **Genetics and ancestry:** genes give individual variation; no behavioural, cognitive or moral trait differs by ancestry or population. Group differences come only from culture, environment and history. | the genome schema (no trait with a population-dependent mean other than pigmentation and body form); a lint over gene pools (the archive had one) |
| 2 | **Children** are never targets of deliberate violence by players or people; childhood's dangers are never shown (`07-lives.md`). Caregiving affection (holding, carrying, comforting, a goodnight kiss on the forehead) is its own non-romantic category of actions. | action preconditions on age; minds' instructions; filters; tests that every harmful action against a child is refused |
| 3 | **Nothing romantic or sexual ever involves anyone under 18**, at every layer. | action preconditions; relationship states; minds' instructions; output filters; tests |
| 4 | **Intimacy between adults** is consent-modelled (both people's genuine willingness; a person's own willingness toward a player). Holding hands, hugging and kissing are shown; sex exists only as an implied, off-screen life event (fade to black) recorded as a bond or a pregnancy, never explicit. **Sexual violence is not represented at all.** | the action vocabulary (no such actions exist); consent as a precondition; filters |
| 5 | **Violence** is realistic and non-gratuitous: fights, feuds and wars happen; there are no torture or mutilation mechanics. | the vocabulary; injury models without such outcomes |
| 6 | **Historical institutions** (hierarchy, servitude, war, inequality) are represented soberly as social structures, never as reward loops. | design reviews; no progression rewards tied to them |
| 7 | **Fictional cultures only:** no real ethnic groups, religions or real people imitated or caricatured. | culture generators; the Historian's style guides; filters on names and texts |
| 8 | **Minds stay in the world:** they never produce real-world harmful instructions, slurs or hate, and never claim to be real people. | instructions; output filters; red-team suites |
| 9 | **Voice:** no cloning of real people's voices; no recording or reuse of players' voices (Amendment R §5.5). | voice models built from parameters, not samples; no storage of player audio |

Every refusal is logged with the rule it enforced; the red-team suites (F12) grow with every
incident found in play.
