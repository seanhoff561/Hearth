# Social life (V2.1 §8; H4)

How the simulated people live together: who they are to one another, the hearths that share
food, the ties between them and what they owe, what they share, what they think of one another
and say about it, the norms they hold and the sanctions that keep them, how they decide together,
how quarrels rise and ease, and how they meet strangers. Written with H4 and extended part by
part; cultures (H5) will carry the norms, kinship systems and procedures that are the foragers'
here.

## Kinship and households (H4 (a))

Who another is to a person is read from the pedigree and the pair bonds (`kin.rs`): partner,
parent, child, sibling and half-sibling, grandparent and grandchild, aunt and uncle, niece and
nephew, cousin, and the in-laws (a partner's parents and siblings, a child's partner, a sibling's
partner). Each kin relation has a nearness used to order kin and to choose among them; cultures
will name and group them as their kinship systems do.

A **household** is the hearth group that shares food: a woman with her partner and the children
she raises, the unpaired grown staying with their mother while she lives in the band, the rest
each their own. Every band's members are settled into households when it is drawn out or its
course reckoned; a pair keeps a hearth of its own when it bonds, taking the children either raises
who are not yet paired; a child is born into its mother's; when the last grown one of a household
dies, the young left go to their nearest kin's in the band; a band splits by households.

## Relationships and obligations (H4 (b))

Each person keeps **ties** to those it knows (`ties.rs`): affection, trust, respect, fear and
rivalry (0–1), and a **ledger** of what it has given and what it owes. Ties begin from kinship —
a partner, parent or child at 0.75, a sibling 0.6, a grandparent or half-sibling 0.5, an aunt or
uncle 0.4, an in-law 0.35, a cousin 0.3, a band-mate 0.15, others 0.05 — and that is where they
settle without contact. Time within eight metres draws people closer slowly, grooming much more;
a gift moves both ledgers and warms the one given to. Untended ties fade back monthly, the ledger
more slowly (old debts forgiven); the dead are let go (their memory kept in what happened); a
person keeps at most a hundred and fifty ties, the weakest dropped. Band-mates all know one
another.

## Cooperation (H4 (c))

One carrying food who sees one hungry within fifteen metres brings it to them — its household's
first, then kin and those it is fondest of, the generous (their **Generosity** value) the more
readily, never while hungry itself — and the gift is eaten and both ledgers move. One of a
person's own who is hurt is stayed by, a child by its mother too: their fear eases and the tie
warms. Kin altruism follows from the ties (kin begin closer), and reciprocity from the ledger.

## Reputation, gossip, norms and sanctions (H4 (d))

What people believe of one another (`repute.rs`) — how **generous** (−1 stingy … 1) and how
**honest** (−1 a thief … 1), and how **sure** — comes from deeds they see by daylight within forty
metres (taking another's things, keeping food from one hungry near, sharing food, staying by the
hurt) and from **gossip**: in close company, now and then, one tells another what it thinks of a
third that is most worth telling, and the hearer comes round to it as far as it trusts the teller,
less sure than one who saw (six tenths), the telling changed a little in the passing. Views fade
monthly, the heard sooner than the seen.

**Norms** are data (`humans/social/norms.ron`, the foragers' until cultures carry their own):
another's things are theirs (taking weighs 0.6) and food is shared with the hungry (withholding
weighs 0.25 — seen when one carrying food lets three chances pass to feed one hungry near). A
breach stirs anger in its victim and indignation in those who see it, lowers their trust in its
doer and worsens their view of it by the norm's weight. As its doer's name worsens among them,
they apply the norm's **sanctions**, each from how bad a name: mockery to its face while the
indignation lasts (it feels shame and is the less fond of its mockers), keeping away from it,
food withheld from it (but for their own household's young), and at last — when the band's grown
think worst enough of it on the whole — casting it out to a band of its own some kilometres off.

**Property**: what a player puts down, lets go of, throws or cannot carry stays theirs, and the
people leave it be; what the people lay down is the band's to use.

## Status and group decisions (H4 (e))

A person's **standing** is the respect its band's grown hold it in (the respect in their ties),
earned by work done well before them and by sharing. A band of a people that keeps camps
(humans, not the tree-nesting early hominins) keeps a **camp**: those who do not nest in trees
sleep there, and go back toward it when they have wandered four hundred metres off.
Each evening a band whose camp has gone poor — half its grown hungry, or none of them remembering
food within two hundred metres or water within three hundred — holds **council** on where to keep
it (`council.rs`). The places argued for are the camp and the food places its grown remember; each
reckons each by what it remembers there (food, water, danger), the way there (the more for those
who carry young or are old) and, if not hungry, staying. In rounds of talk each place's supporters
make its case — what they remember it offers, heard at six tenths of what one knows oneself — and
each comes round toward the case made best, toward the many (the conforming) and toward the
respected (those who heed prestige). When three in five agree (or most, after six rounds) the band
moves its camp there, all of it; without agreement it stays. The council is remembered (the places
argued for, the support for each, the rounds, the choice) and shown in the inspector.

## Conflict (H4 (f))

A wrong — a thing taken, food kept from one hungry, a mocking, a warning off — leaves **rivalry**
in the wronged one's tie to the one who did it, the deeper the worse the wrong. Each second a
grown one holding a grievance against another grown within ten metres may **have it out** with
them: the likelier the deeper the rivalry, the angrier it is (its anger, and half its
indignation), the lower its threshold for aggression and the less it fears them; having had it
out, it lets the grievance rest a day. A quarrel climbs a ladder (`conflict.rs`): an
**argument** (the two face each other, voices raised), **threats** (closer, the body made big)
and, rarely, **blows** — a short scuffle in which a blow now and then lands as a bruise (an arm,
the chest, a leg, rarely the head). Most quarrels ease before then. The weaker (the less
dominant, the more afraid) **backs down** and goes off, fearing the other the more; one **of
standing** in the band or **kin to both**, near and at odds with neither, **steps in** and talks
them round, and is respected the more for it; the one in the wrong **makes amends** with food it
carries (the fair and the guilty the more readily); or the argument runs its time and the words
are spent. A rung that runs its time rises with a chance that grows with their tempers — an
argument to threats at half their heat, threats to blows at a third — else it ends. Each ending
eases the grievance by its measure: amends most, then mediation, then blows (the loser fears the
winner, whose grudge is spent), then talk and backing down. Children are never in a quarrel, and
one with a player never comes to blows (its threats run their time and end). A **feud** —
rivalry still deep after three quarrels — ends at the month's reckoning with the weaker side (the
less respected, else the younger) **leaving the band** with its household, for a country of their
own some kilometres off, where they keep their own camp (a band that splits as it grows does the
same); a household with a player in it stays and the other goes. Rivalry fades monthly, the
sooner in the forgiving, and a feud long eased is forgotten. How long each rung lasts and when a
feud parts a band are the people's ways (`humans/social/ways.ron`).

## Strangers (H4 (f))

One not of a band — nor its guest, nor its host — whom a person does not trust (below three
tenths) is a **stranger** to it (`strangers.rs`). Within eighty metres it is **watched**, the more
by the wary, and the young go to their mothers. Coming within thirty metres it is **met**: one of
the grown (the trusting and the sociable the more readily; one at a time) goes to it and
**greets** it, and the two take the first trust a greeting gives (three and a half tenths) — the
stranger is now the band's **guest**. People with ways with strangers do not run from a calm one
as animals do; only one running at them or hunting near them is a threat. A guest is let near
and **fed when hungry** as their own are, at the hospitality of their ways (six tenths of the
care for one of their own), and comes to be known: time near the band draws them together at
half the pace of band-mates, the deeds seen and the talk tell of it, and **gifts** above all — a
gift taken into one's keeping moves the ledger, makes the one given it fonder and more trusting
and sure the giver is generous (those who see it think so too), and makes its band the readier to
have a player near. Each evening a band weighs its guests: one gone off or dead is a guest no
more; one among them two days whom three in five of the grown trust to half is **taken in** — one
of them now, in a household, knowing them all. Where their country is **crowded** (more of their
kind about than the land feeds well, past what their ways bear), or the stranger has a bad name
with them or a grievance holds, it is **warned off** instead: one of the bolder goes up to it
shouting, the warning a wrong to it, and one still there when the warning is spent is threatened
as in a quarrel. A child is never warned off.

Group conflict among foragers is, so far, these: warnings in crowded country, and grudges between
persons of different bands, carried as anyone's. Kin married into another band are not strangers
to it (an alliance by marriage), and gifts make friends of strangers. Raids and organized conflict
come with the peoples that have them (H11–H12).

**The player among strangers.** The player is a person too: born into its family's band and a
stranger to every other. Met and greeted, it is their guest; what it hands one of them (E, looking
at them within reach with something in hand) is a gift like anyone's; taken in, it belongs to
that band (its family is still its family). Warned off, it is threatened but never struck. The
game tells the player in words when it is greeted, warned off, taken in or quarrelled with, until
speech comes (H5).

## The acceptance (H4 (g))

PLAN's acceptance for H4 is checked by `tests/social.rs`: word of a theft seen in the dusk by the one
robbed alone spreads through a band of nineteen by talk within half a day, those who heard it less
sure than the one who saw it (gossip spreads reputation plausibly); a theft angers its victim,
stirs indignation in those who see it, is told, and brings mockery, keeping away and at last
casting out (a norm violation produces gossip and sanctions); a band whose camp's country has gone
poor argues where to go, comes round, and moves its camp (a group debates and decides where to
move camp). Quarrels rising and easing and strangers met, taken in or warned off have their own
tests beside these.

Left for later: levelling among egalitarian foragers (the boastful deflated, the would-be
dominant checked) comes with cultures' values (H5); the people's own gifts to strangers and
partnerships of exchange with cultures too; raids and organized conflict with the eras that have
them (H11–H12).
