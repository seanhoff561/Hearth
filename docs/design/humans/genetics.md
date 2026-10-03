# Genetics

*V2.1 §4 and Addendum A; milestone H1; D171–D173. Data: `data/hearth/humans/genetics/`. Code:
`crates/hearth_people/src/genome.rs` (the architecture, meiosis, phenotypes), `lineage.rs`
(kinship), `looks.rs` (the figure's looks), `birth.rs`; the player's birth in
`crates/hearth/src/born.rs`. Ground rule 1 (genes make individuals, not peoples) is enforced
here.*

**In short:** every person carries two copies of an abstract genome — a few hundred loci on 23
chromosome pairs — one from each parent, shuffled by crossing over. The genome, the person's
development and chance make what they look like, how tall they grow, how healthy they are, their
temperament and their knacks; children look like a mix of their parents, and the traits run in
families about as much as they do in life.

## The genome

- **Chromosomes.** 22 autosomes and X/Y, each with its genetic length (cM) after the human
  linkage maps (chromosome 1 about 270 cM down to 21 about 60, X about 180; 3,600 cM in all,
  sex-averaged). A woman's meioses cross over about 1.7 times as often as a man's, so her map is
  scaled up and his down. X recombines only in women; a man passes his X or his Y whole.
- **Loci.** About 460, each on a chromosome at a position, in functional groups: pigmentation
  (skin, hair, eyes), hair form, height, build, health (general resistance to infection, an
  immune-diversity region of six HLA-like loci, thirty recessive conditions, longevity,
  fertility), metabolism (lactase persistence, altitude, cold, starch digestion), temperament
  and aptitude. A trait is either a few named loci of large effect (eye colour's major locus, red
  hair's recessive) or *polygenic*: many loci of small effect laid out from its architecture in
  the data (how many loci, the spread of their effects, any dominance) under a fixed seed, so
  every world of a game version shares one genetic architecture.
- **Alleles** are small integers; a person's genome is the two copies (mother's, father's) of
  each locus, a few hundred bytes.

## Inheritance

- **Meiosis.** For each chromosome a parent passes, the number of crossovers is drawn from its
  length in Morgans for that parent's sex (at least one per pair, as chiasmata require),
  their places at random along it; the gamete starts on one copy at random and switches at each
  crossover. The child is one gamete from each parent; its sex is the father's X or Y.
- **Mutation.** Each allele passed mutates at a small rate (2·10⁻⁴; far above a base's, as a
  locus here stands for a gene's worth of sequence): a two-allele locus to its other allele (a
  recessive condition's working allele to its broken one), a locus of many alleles to another.
  It replenishes what drift wears away.
- **Inbreeding.** The recessive conditions are many rare broken alleles; a child of close kin is
  more often homozygous for one, and is frailer for it (a band whose numbers fall loses its
  frailest first; the life course weighs it in health and survival, H3), never shown graphically.
  A band's children are fathered by grown males who are not the mother's close kin (kinship under
  an eighth), as apes and people avoid them; cultures come to forbid close marriages (H5).
- **Lineage.** Persons record their parents; the **kinship coefficient** between any two is
  reckoned from the pedigree (founders unrelated): a half for oneself, a quarter for a parent and
  child or full siblings, an eighth for half-siblings, a sixteenth for first cousins. It weighs
  kin altruism (H4) and gives each child's inbreeding coefficient.

## From genotype to phenotype

Each trait is the sum of its loci's effects (with dominance where biology has it), plus the
person's development and chance:

- **Calibrated heritability.** For each polygenic trait the data names a heritability target with
  its source; at load the genetic variance the architecture gives at the loci's own (species-wide)
  frequencies is reckoned and the environmental spread set so that genes account for that share
  of the variation. Every pool is read on that one scale, so pools that differ in physical loci
  differ visibly (D171). Tests measure it the way twin and family studies do: the
  offspring–midparent regression and the correlation of siblings over a simulated population.
- **Appearance** (drives the character figure, so families look alike): skin pigmentation
  (polygenic with a few larger loci, heritability about 0.8), its undertone; hair colour from
  eumelanin and pheomelanin (a recessive red); eye colour from a major locus with modifiers (two
  brown-eyed parents can have a blue-eyed child); hair form (straight to coily); facial hair's
  density; height (heritability about 0.8 under good nutrition; childhood nutrition and illness
  shift it, H3); build. Faces are one shape until the figure takes facial proportions; then they
  inherit too.
- **Health:** resistance to infection, immune diversity (heterozygosity at the HLA-like loci),
  recessive conditions, longevity, fertility (read by the life course and illness, H3).
- **Temperament** on the **HEXACO** model's six factors — honesty-humility, emotionality,
  extraversion, agreeableness, conscientiousness, openness — plus narrower dimensions the mind
  uses (behavioural inhibition, stress reactivity, impulsivity against self-control, novelty
  seeking, sociability, the threshold for aggression, empathy), heritability about 0.3–0.5, the
  rest development and chance (D172).
- **Aptitudes:** small differences in learning rate by domain (motor, spatial, social, verbal,
  memory) and in stamina: a tenth or so either way, small beside practice and teaching.
- **Sex:** average differences in size and strength only (dimorphism, by species); no behavioural
  difference is written into genes (ground rules 1 and 6).

Phenotypes unfold: what is reckoned at birth is the genetic value and a draw of chance; growth,
nutrition, illness and stress adjust it through life (H3). The inspector shows the chain:
genotype score → development → phenotype now.

## Gene pools

A population's **gene pool** holds the allele frequencies of the *physical* loci only —
pigmentation drifting toward what its sunlight favours, lactase persistence where herds are
milked, altitude and cold adaptations where they are lived with. The behavioural loci
(temperament, aptitude) have **one** set of frequencies for the whole species, in every pool:
the schema has no place to give a pool its own, the content lint fails any entry that tries, and
a test draws people from pools across the world and finds their temperaments and aptitudes alike
(ground rule 1). Until H8's deep time makes populations, a place's pool is the species pool with
its pigmentation and hair form set from the place's sunlight, cos(latitude)³ (altitude and cloud
join with H8): the tropics' people dark-skinned, dark-eyed and curly- or coily-haired, the far
north's pale, often fair and blue-eyed, every gradation between (D173).

## The player's genome (Addendum A)

The player is born a child of two parents: the genome is the meiosis of theirs, drawn when the
birth is confirmed; only daughter or son (or chance) is chosen, which the father's gamete is
drawn to match. Until families live in the world (H3), the parents are drawn from the gene pool
of the place chosen and shown on the birth screen — the mother, the player (grown, twenty) and
the father side by side — and the appearance editor is gone: a new world asks only a name, the
sex or chance, and the loincloth first worn (D173).

## Tests (V2.1 §18)

Mendelian ratios on single-locus test traits; recombination fractions against distance
(Haldane's map) and crossovers per meiosis by sex; mutation rates; heritabilities within tolerance
of their targets from offspring–midparent regressions and sibling correlations; inbreeding's
homozygosity and its health cost; kinship coefficients over constructed pedigrees; the ancestry
lint and the pools' alike temperaments; a three-generation family drawn for screenshots
(`hearth_people/tests/genetics.rs`, `src/lineage.rs`, `tests/persons.rs`;
`tools/shots/h1_families.shots`).
