> **Superseded by Amendment E** (`docs/spec/amendments-e-q.md`, 2026-10-08): the simulated humans were removed in E0 and are planned anew as Phase F (`docs/design/future/humanity/`). Kept for reference; nothing here is built, tested or linted.

# Language and communication (V2.1 §10; H5)

How the simulated people speak: generated languages with their sounds, words and grammar, how
languages part into families and borrow from each other, the names people are given, the speech
acts every exchange between persons is made of, and how the player comes to understand them.
Written with H5 and extended part by part.

## Languages (H5 (d))

Every culture of a people with language speaks one (`language.rs`; data in
`humans/language/generators.ron` and `meanings.ron`):

- **Sounds**: a language draws its consonants and vowels one by one, each as likely as the share
  of the world's languages that have it (PHOIBLE; Maddieson 1984) — so m, k, p, n, t, a, i, u are
  in nearly all and kh, ts, ë in few — until it has between eight and eighteen consonants and
  three and seven vowels. Sounds are written in a plain romanisation (ng, ny, ch, sh, kh, ' for a
  glottal stop, è and ò open, ë as in "about").
- **Syllables**: every language has consonant–vowel syllables; others (a vowel alone, a closed
  syllable, two consonants before the vowel) each in as many languages as the data has them, so
  one language's words sound alike and unlike another's.
- **Words**: a word for each of the core meanings (about a hundred and ten: Swadesh's list of
  people and kin, the body, the land and what lives on it, what one does and what things are
  like, with greetings, yes and no, thanks and sorry, "away"), each a few syllables long — the
  commonest meanings the shortest — and no two alike.
- **Grammar**: a word order (verb last or in the middle in most, first in some, by Dryer's
  counts), adjectives before or after the noun, a plural and a past suffix in about half.
- **Names**: everyone of a people with language is named in it at birth (two or three of its
  syllables), and those drawn out already are named when their band's culture is drawn. A
  player's own person keeps the player's name. Names stand beside the numbers in the inspector.

Species gate language (V2.1 §10.1): *Australopithecus* calls and gestures only; our kind full
language (*H. erectus*'s proto-language and the Neanderthals' with H8).

### Families, drift and borrowing

A daughter culture speaks a daughter of its parent's language — the same words, its own lineage —
and from then on each changes on its own, once a year of its life course:

- **Regular sound changes** (data, after the historical record's commonest kinds: p to f and k
  to h, voicing between vowels, t to s and k to ch before i and e, losses at a word's end, vowels
  raised or merged, l to r): each with a small chance a century, and when one happens it runs
  through every word at once, so related languages keep regular correspondences ("pana" here,
  "fana" there).
- **Replacement**: a word's chance a century to give way to a new one is a hundred and fiftieth —
  glottochronology's core-list retention of about 86 % in a thousand years (Swadesh 1955).
- **Borrowing**: neighbouring bands (within twenty kilometres) now and then lend each other a
  word, as they share customs.

Two daughters five hundred years apart still share most of their words as cognates (no more than
a third of their sounds apart), against a few chance likenesses with a language of another
people; after many thousands of years only traces remain.

## Speech acts (H5 (e))

Every exchange between persons is a **speech act** (`speech.rs`): what it does (V2.1 §10.2's
list: greet, inform, warn, offer, thank, apologise, insult, threaten, command, gossip, argue for
an option …), whom it is to, its words — meanings, and people's names — in the speaker's word
order, and the **gesture** that goes with it (pointing, beckoning, holding something out, waving
off, the threat display, the head bowed, open arms, hands held out), which needs no shared
language. Persons speak through what they already do, so an act's effects are what H4 computes:

| What happens (H4) | The act and its words |
|---|---|
| A stranger met | greet: "hello friend", answered "hello", with the culture's greeting |
| A stranger warned off | warn: "go away", waving it off |
| A grievance had out | insult: "you bad"; at the threat rung, threaten: "I hit you", drawn up |
| One backs down | apologise: "sorry", head bowed |
| One steps in | command: "stop enough" |
| Amends made | apologise: "take this sorry", holding it out; thanked |
| Food shared, a gift taken | offer: "eat this", holding it out; thank: "thanks" |
| Mockery | insult: the name and "bad" |
| Gossip | gossip: the name and "steal", "not share" or "good" |
| The council | argue for: "we stay here", or "we go food there", pointing |
| A guest taken in | accept: "yes stay here", arms open |

A step's acts are kept about half a minute for whoever hears them; a people without language
says nothing (the hominins' calls stay the animals' kind).

## Subtitles and the player (H5 (f))

What is said within twenty metres of the player is shown as a **subtitle**: who said it (and
whether to the player), the gesture, the words as they sound, and what the player makes of them —
each word known (seven tenths and more) in the player's own tongue, a half-known one with a
doubt ("hello?"), an unknown one as dots. A player born into a family speaks its language as
its mother tongue. Another is **learned by hearing it**: each word heard comes a tenth of the way
to known when spoken to the player and a twenty-fifth when overheard, twice as fast with a
gesture making its sense plain. A word of a language related to one the player knows is made out
by its cognate (six tenths as well as the cognate is known), so a daughter people's speech is
half understood from the first. The H4 notices (greeted, warned off, taken in) stay beside the
speech.

## The acceptance (H5 (g))

`tests/culture.rs`: a people grown past itself splits and its daughter goes far; two hundred
years of their lives later their cultures differ in values and customs, and their tongues are
plainly related — most words cognates — and not to another people's; and a newcomer from
another people, among hosts who have something to talk about, makes out little of their speech
at first and most of it after half an hour of hearing them.
