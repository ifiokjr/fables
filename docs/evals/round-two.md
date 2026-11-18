# Round two: nine runs after the ease-of-use work

18 November 2026. The task is [`task.md`](task.md). Each run had a clean checkout, the
documentation, `--help`, and an empty studio. Runs 1 to 6 were on the same build; runs 7 to 9
came after the fixes that runs 1 to 6 asked for.

## The runs

| Run | Model | Style | Flow | Failed commands | Steps finished (of 6) |
| --- | --- | --- | --- | --- | --- |
| 1 | Haiku 5.5 | `cartoon` | `three-tries` | 1 | 6, but the video file was corrupt |
| 2 | Haiku 5.5 | `silhouette` | `round-we-go` | 2 | 6 |
| 3 | Haiku 5.5 | `rubber-hose` | `odd-couple` | 1 | 6 |
| 4 | Sonnet 5.5, low effort | `paper-theatre` | `there-and-back` | 5 | 6 |
| 5 | Sonnet 5.5, low effort | `anime` | `wrong-end-of-the-stick` | 2 | 6 |
| 6 | Sonnet 5.5, low effort | `soft-flat` | `goodnight-tour` | 3 | 6 |
| 7 | Haiku 5.5 | `cartoon` | `three-tries` | 1 | 6 |
| 8 | Haiku 5.5 | `chibi` | `pile-up` | 1 | 6 |
| 9 | Sonnet 5.5, low effort | `mid-century` | `habit-tale` | 0 | 6 |

## Against the baseline

| | Baseline (3 runs) | Round two (9 runs) |
| --- | --- | --- |
| Finished every step | 3 of 3 | 9 of 9 (one video unusable) |
| Failed commands per run | 4 to 10 | 0 to 5, median 1 |
| Cast reads as one show | none | all nine |
| Story elements shown, not only told | every run dropped some | shown where a prop exists; still dropped where none does |
| Models | three small free models | the two least capable current models |

The models differ between the rounds, so the failed-command numbers are not a clean
like-for-like. What can be said plainly: the baseline's commonest failures (starting a
character before the cast file exists, `--out` on commands that did not take it, front matter
that did not parse) did not happen once in round two.

## What the pictures showed

Every contact sheet, cast sheet and video was looked at.

- **Casts hold together.** Starting each character from the style's template gives two people
  who belong to the same drawing. The redrawn hair is usually the weakest part of a
  character.
- **Props carry the story when they exist.** Runs 4, 5 and 9 named things the tool can draw
  and the pages showed them. Run 1's puddle, run 2's boat and run 7's ribbon have no prop, so
  the narration mentions them and the picture does not.
- **Three casts had an orange lead.** Runs 1, 3 and 7 made a character without `--skin` and
  got the same orange. Fixed after the round.
- **Close-ups often crop the second character.** A close-up on a two-person page keeps one
  face and half of another.
- **An empty "Meet the folk" page** closed every book made in a new world. Fixed after the
  round.
- **The videos run about three and a half minutes**, longer than the storyboard's estimate of
  about three. The stand-in voice is slower than the estimate assumes; with a real voice the
  two should agree. Nobody has checked that by ear.

## What tripped them, and what was done

Fixed between runs 6 and 7:

| Trip | Fix |
| --- | --- |
| `on=` naming a spot with nobody on it passed `check` and drew nothing | `check` reports it and suggests the nearest name |
| Colour names had to be guessed | the palette is in `--help`; a wrong name suggests the nearest |
| `fables flows`, `setting list`, `character check` did not exist | they do now |
| A new character's sheet came out in the default style, not the world's | sheets and rigs use the world's style |
| The scaffold's `@who talk` lines were unexplained | the scaffold comments them |
| Two renders to one file corrupted the video (run 1) | the video is written to a partial file and renamed |
| The camera left a speaker for the heading's subject mid-line | a heading's framing holds while its subject speaks |

Fixed after run 9: a character made without `--skin` gets a skin tone picked by its id; the
`habit-tale` scaffold explains its two scene lines; a world with nobody to meet gets no empty
page.

Runs 7 to 9 failed two commands between them. One was `check` catching the `on=` mistake, the
tool doing its job. The other was `setting list` without `--world` in a checkout with many
worlds.

## Still in the way

- `--out` and `--world` have to be repeated on nearly every command.
- A story about a thing with no prop cannot show it. The review says so, but there is no way
  to draw a new prop short of writing a part file.
- The guidance on redrawing hair says how many curves but not what makes a part unreadable.
- The storyboard does not show walks.
- `check` has no word budget, and shots under a second and a half are not flagged.
- A render that is interrupted leaves a `.partial` file that looks finished.
- Several flows suggest more pages than this task's eight to ten. That is the task's fault,
  not the flows'.

## Honest limits

Nine runs, two models, one judge. Each style and flow was run once, so a style that looks easy
here may have been lucky. The by-eye judgements are one reviewer's, with no second opinion.
The sound was never heard.
