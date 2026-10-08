# 9. Eras

*Amendment E §10.9.*

## An era is a starting condition and a date
| Part | What it sets | Where it lives now |
|---|---|---|
| Date | when the History Engine stops and play begins | `data/hearth/eras/eras.ron` (names and one line today) |
| Climate | temperature and sea-level offsets (a glacial maximum lowers the sea about 120 m), ice | to add to the era's data with F11 |
| Species | flora and fauna by their dates (`first_appearance_ya`, `extinction_ya` on each species) | `hearth_content::schema::{flora, fauna}` |
| Peoples | the species and populations of people, their ways of life and where they start | F1–F2, from the archive's era profiles |
| Knowledge | the baseline each people knows | the knowledge graph's eras (`data/hearth/knowledge/`) |
| Social forms | household, band, village, chiefdom, state: the institutions that exist | F8 |
| Material culture | building forms, dress, tools: the vocabulary people build and make from | process and construction data |
| Plan libraries | how things are done there (`04-mind.md`) | F4–F8 |

"Born a prince or a commoner, or anything that existed then" comes from the households the
History Engine produces for the date; nothing is staged.

## Order of rollout
1. **Upper Paleolithic** first: the vertical slice of Phase F (F11), bands of *Homo sapiens*
   across the world with blades, needles and art.
2. **Lower and Middle Paleolithic:** *Homo erectus*; Neanderthals in the last glacial
   (the archive's bodies, life tables and proto-languages adapted).
3. **Neolithic**, **Bronze Age**, **Iron Age / Classical**.
4. Later eras as the technology graph's planned nodes (v2 §12.2) are built.

Each era ships with an **authenticity review** (`11-evaluation.md`) before it is playable;
until then it stays "Coming soon" on the era list (E §6.4).
