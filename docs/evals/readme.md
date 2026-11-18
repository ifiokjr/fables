# Evals: can a newcomer make a cartoon?

The test of whether fables is easy to use is to hand it to a weak agent with nothing but the
documentation and ask for a cartoon.

| File | What it is |
| --- | --- |
| [`task.md`](task.md) | The task every run is given, word for word. |
| [`round-two.md`](round-two.md) | Nine runs after the ease-of-use work, judged by eye, compared with the baseline. |
| `baseline-*.md` | The three reports from before that work, as the agents wrote them. |

## Running one

1. Build the tool and copy the binary somewhere outside the checkout, at a path ending in
   `/bin/fables`.
2. Give the agent a clean checkout (documentation and `--help` only, no source reading) and an
   empty studio folder.
3. Send it `task.md` with a task message naming the binary, the studio, a drawing style and a
   story flow.
4. When it finishes, look at the cast sheet, the proofs' contact sheet and the video yourself.
   The agent's own report says what tripped it; only your eyes say whether the cartoon is any
   good.

Linux has no `say`. The runs here used a stand-in that writes a short tone for each word, so
their videos have the right timing and no speech.
