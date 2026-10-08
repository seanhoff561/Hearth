> **Superseded by Amendment E** (`docs/spec/amendments-e-q.md`, 2026-10-08): the simulated humans were removed in E0 and are planned anew as Phase F (`docs/design/future/humanity/`). Kept for reference; nothing here is built, tested or linted.

# The optional conversation backend (V2.1 §10.4; H10)

How a language model may give the people's speech acts natural words and read what the player
types — and why nothing it says can change the world, or put a word in a person's mouth that the
person could not say. Written with H10.

The game never needs it. Off (the default), the people speak in speech acts rendered from
templates in their own language, shown as far as the player understands it (H5), and the player
speaks with the talk wheel and gestures (H9). Set up, a model does two things, and only two.

## Setting it up

Options → Conversation (`options.toml`'s `[conversation]`):

| Setting | What it is |
|---|---|
| Backend | **Off**; **a model on this computer** (Ollama, LM Studio, a llama.cpp server, vLLM …); **a provider's model** over the internet |
| API | the chat-completions API most servers speak, or the Anthropic Messages API |
| Server address | up to the API's version: `http://127.0.0.1:11434/v1` for Ollama, `https://api.anthropic.com/v1` for the Messages API |
| Model | the model as the server names it — typed, or chosen from the list the server gives (*List the server's models*); the game names none of its own |
| API key variable | the environment variable holding the key (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY` …); the key is read from it when the backend starts and is never written down. A local server needs none |
| Type to people | whether **T** opens a line to type to the person looked at |
| Time allowed a line | 2, 4, 8 or 15 seconds: a phrasing later than this is let go |

*Test* sends a harmless request (a greeting for a traveller) and shows the reply and how long it
took. The screen says plainly what a remote provider is sent: what a person who speaks to the
player knows of itself, and what the player types, under the provider's terms.

The adapters (`crates/hearth_ai/src/http.rs`) speak each API as documented: chat completions
(`POST {address}/chat/completions`, a bearer key if any, the system and user messages, the
model's choice of words read from `choices[0].message.content`; models from `GET
{address}/models`) and Messages (`POST {address}/messages` with `x-api-key` and
`anthropic-version: 2023-06-01`, the instructions as `system`, the text blocks of `content`
read). An address on this computer is asked directly, never through a proxy. Tests ask a mock
server only; no test calls a real service.

## Phrasing (the people's lines)

A line said **to the player and wholly made out by it** — nine tenths of its words known — is
phrased; a line overheard, when nothing else is waiting. A phrasing is a translation, so the
player is given none of a line it does not understand: a speaker of a tongue the player half
knows keeps its dots and doubts. The templated line is shown at once, as before; the phrasing,
if it comes within the time allowed and passes the filter, takes the place of the templated
sense on the same subtitle ("Tana to you: “ka nupi so” — Hello, friend. Come and sit by the
fire."). A speaker saying the same act to the same listener again says it the same way (the
last 256 phrasings are kept).

**What the model is told** (`hearth_people::converse`): the speaker's own state and nothing else
— its name and how it looks (sex, an age as plain to see), its nature (its marked tendencies in
words: "patient", "quick to anger"), what it feels strongly now, what it and its people hold to
(its marked values; its culture's hierarchy, reach, honour and tightness in words), the
techniques it knows by name, its kin (by name when it knows them), what has befallen it lately
(a threat seen, a hurt, a loss, a stranger met, a feast), whom it speaks to and how it stands
with them (kin, band or stranger; fond, wary, at odds, owed), the names it may say — its own,
the listener's if it knows it, those of the kin and happenings it is told, and those the act
itself carries — and the act: what it does ("a greeting", "talk of someone"), its words as
the speaker means them ("hello friend"), and its gesture. The prompt (`data/hearth/ai/
prompts.ron`) tells the model to say only what the act says, add no fact, use simple words and
of skills only those the person knows, say nothing of later ages, name no one else, say nothing
sexual, cruel, harmful to a child or slurring, and reply with the line alone in at most twenty
words. It is never told the player's typed words.

**The filter** (`hearth_ai::Lexicon`) then holds the line to what the speaker may say, whatever
the model did; a line refused is let go and the templated line stands:

- **Closed vocabulary.** Every word must be the speaker's: an *everyday* word any forager has
  (`data/hearth/ai/words/everyday.ron`: the small words of every sentence, people and the band,
  the body, feelings and the mind, what one does, the land and its creatures, food and the things
  of a camp, what things are like, time and place, greetings — some 1,800 plain forms), a gloss
  of its language's meanings, a word of a technique it **knows** (`techniques.ron`: each
  technique's words, theirs alone who know it — "spear", "tar", "needle", "pot", "kiln", "wheel"
  — and of a technique without a list, the words of its name that are no one's everyday words),
  or a name given it. The plain forms are read through English endings (-s, -ed, -ing, -er, -est,
  -ly, -ness, -ful, -less, un-, a doubled consonant, a dropped e) and contractions ("don't",
  "we'll", "let's"); irregular forms are listed. A word a technique the speaker does not know
  owns is refused as that technique's; a word of a later age (`later_ages.ron`: machines, metal,
  money, rule and war, towns and fields, writing and reckoning, the knowledge and the talk of our
  own day) as an anachronism; anything else as a word outside its words.
- **Names.** A name of anyone in the world not given the speaker is refused wherever it stands,
  and so is any capitalised word mid-sentence that is not a name given it ("Africa", "Gronk").
- **Kin.** A kin word ("sister", "husband", "grandson" …) is said only of kin the speaker has —
  "my sister" from one with a sister — except of kin in general ("a mother", "like a brother").
- **A spoken line.** Letters, spaces and the punctuation of speech only: no digits, markup,
  emoticons, parentheses (stage directions) or line breaks; thirty words and 240 characters at
  most. Wrapping quotation marks and a "Name:" put before the line are taken off.
- **Never said** (`later_ages.ron`'s second list): words no line holds, whoever says it.
- **The act's sense kept.** A refusal must say no (no, not, never, nothing, away …); a yes may
  not begin with one.

## Reading the player's typed words

With *Type to people* on, **T** opens a line to type to the person looked at (as the talk wheel
would speak to them, within eight metres). The model is told the acts a player may make — greet,
introduce oneself, thank, apologise, praise, joke, insult, ask to be shown, offer to show, ask to
stay, propose to pair, or a gesture — the techniques there are by name, whom the words are to as
the player knows them, and the words; it answers in JSON: an act, a technique, how sure it is,
and what else it may be. The answer is taken only as one of those acts, with a technique only
from those offered (one named that the game does not have is asked about as such: no one knows
it, so no one shows it); any other field in the answer is ignored. Plainly one act (seven tenths
sure) and not a proposal to pair or an insult, it is made exactly as if chosen on the wheel, and
answered as the wheel's choice is. Otherwise — unsure, weighty, unreadable or too late (twice the
time allowed a line) — the player is asked *What did you mean?* and offered the model's readings
and those the typed words' cue words point to (`data/hearth/ai/cues.ron`), at most three; nothing
is done until one is chosen.

## Why the world cannot change through it

- The model is asked on a thread of its own (`hearth_ai::Worker`); the people's decisions never
  wait on it or read what it says. Its jobs wait their turn — a player's typed words first — and
  one waiting longer than it may is let go unasked.
- What it returns reaches the world only as a speech act the player could have chosen: a
  phrasing is words for the screen; a reading is an act made through the same path as the
  wheel's (`server::speak`). Free text never touches the people's state.
- `context_for` reads the people and changes nothing. The conversation suite
  (`crates/hearth_ai/tests/conversations.rs`) lives two worlds of one seed side by side, the player
  saying the same in both — on the wheel in one, typed and read in the other, which also phrases
  every line said near the player — and their saves are the same byte for byte.

## The acceptance (H10)

- **Disabled, nothing changes** (`crates/hearth/tests/conversation.rs`): with no backend, typed
  words are not heard ("speak with the talk wheel"), a greeting is answered and heard as before,
  and no phrasing, choice or backend message is sent; and the two worlds of the suite end the
  same.
- **Enabled, no leak** (`conversations.rs`): every act said over eight minutes of a band's life
  with strangers near and talk of a theft — greetings, gossip, insults, refusals, lessons,
  apologies — and the answers to what the player says, is taken through the whole path: the
  prompt holds no name not given the speaker and no technique it does not know; ninety-one lines
  as a willing model phrases them all pass; 17,159 leaking lines — every technique word it does
  not know, words of later ages, names not given it, kin it lacks, digits and markup, words never
  said, a refusal made a yes — are all refused. Through the server
  (`crates/hearth/tests/conversation.rs`), a greeting is phrased and sent with its line's id; a
  phrasing naming a king of Rome and his iron is never sent; typed words plainly a greeting are
  answered as one; a proposal and gibberish are offered to choose from, not acted on.
- **The adapters** (`crates/hearth_ai/tests/adapters.rs`) against a mock server of each API,
  with refusals, malformed replies, a server too slow and one not there told apart, and the key
  read from its variable.
