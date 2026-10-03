# Ground rules for people

*V2.1 §1 — non-negotiable. Each rule says what it means here and how the code and the content
lint hold it. Rules are enforced from the milestone that first makes them reachable (in
brackets); until then they are true by absence.*

## 1. Genes make individuals, not peoples

Genes give **individual** variation in temperament, aptitudes and health within every population.
No behavioural, cognitive or moral trait has a distribution that differs by ancestry, population
or appearance. Physical traits may differ between populations where real selection explains it
(skin pigmentation with UV, lactase persistence with dairying, altitude adaptation), and those
differences have no behavioural consequences. Differences in behaviour between groups come from
culture, environment, history and circumstances only.

*Held by* [H1]: behavioural loci (temperament, aptitude) carry one species-wide allele frequency
set, and the gene-pool schema has no field that could hold a per-pool value for them; the content
lint fails if any gene pool, era or culture entry names a behavioural locus or trait; a test
draws people from every gene pool of a deep-time run and finds their temperament and aptitude
distributions the same within sampling error. No code path reads a person's ancestry or
appearance when computing behaviour.

## 2. Children are never targets

The player cannot harm children, and no system lets the player or anyone else target them.
Childhood illness and death exist demographically, as they did, but happen off-screen and are
told with respect (a family in mourning, a small grave), never shown.

*Held by* [H3]: every attack, throw, hunt, raid and predator target choice filters out persons
below their culture's age of adulthood (the player as a child included); the player's strikes
and throws pass through children without effect; childhood deaths are resolved at the household
tier and never while the child is in sight; tests try each targeting path against a child.

## 3. Reproduction is abstracted

Courtship and pair bonds are social relationships. Conception happens off-screen; pregnancy and
birth are life events, not shown. There is no sexual content of any kind. A pair bond forms only
by mutual relationship — both persons' preferences and the relationship's state — with no
coercion mechanics, the player included.

*Held by* [H3, H4]: the pair-bond proposal is a speech act that succeeds only when the other
person's own appraisal accepts it; there is no action that forces one; attraction exists only
between adult peers; tests.

## 4. Violence without cruelty; institutions soberly

Conflict, raiding and war exist as historical dynamics, simulated and shown without gratuity.
There are no torture or cruelty mechanics. Hierarchy, inequality and servitude in later eras are
social structures (status, obligations, who may leave), described soberly, never a reward loop.

*Held by* [H4, H12]: the action and speech-act catalogues contain no cruelty; the content lint
rejects processes and speech acts whose target is a person and whose effect is harm beyond the
combat model's; institutions are data with rules, not player rewards.

## 5. Fictional peoples only

Generated peoples, languages and belief systems never imitate or caricature real ethnic groups or
religions; their structures follow cross-cultural patterns.

*Held by* [H5]: every generated name (peoples, languages, persons, places, deities) is checked
against a deny-list of real ethnonyms, language names and religious names and redrawn on a match;
authored culture and belief templates are structural (kinship systems, subsistence, ritual forms)
and the lint rejects real names in them.

## 6. Gender from culture, never fixed in code

Divisions of labour and roles by sex come from culture data, which varies between cultures, and
individuals vary within any culture. Culture constrains agents; it never restricts what the
player can do.

*Held by* [H1, H5]: genetic sex differences are only in body size and strength (dimorphism); the
lint fails on any behavioural trait given a sex-specific baseline; no code branches a person's
choices on sex except through the culture's role data; the player's actions are never gated by
sex.
