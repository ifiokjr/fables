# You are testing a tool as a first-time user

`fables` is a command-line tool that turns a plain-text script into a children's picture book
and a narrated cartoon. Your job is to use it, as a newcomer would, to make one short original
cartoon from nothing, and to report honestly how it went. You are being evaluated on the
cartoon you make and on the accuracy of your report. The tool is being evaluated on how easy
you find it.

## Rules

- The tool is already built. Run it only as the program named in your task message (an
  absolute path ending in `/bin/fables`). Do not build it, and do not run `cargo`.
- Your current directory is a checkout of the tool's source. You may **read** its
  documentation: `readme.md`, `docs/directing.md`, `docs/animation.md`, and the tool's own
  `--help` output for every command. You may look at the example worlds under `worlds/` to
  learn the file formats. Do not read the Rust source to work out how something behaves; if
  the documentation and help do not tell you, that is a finding for your report.
- **Change nothing in the checkout.** Everything you create goes under the studio folder named
  in your task message. Create your world inside it at `worlds/<your-world>/`, and render
  with `--out <studio>/out`.
- Your story, world and characters must be your own: do not copy an example world's story,
  names or characters. You may start a character's drawings from a style's template (the
  tool has a command for that) and then change them.
- Do not ask questions; decide and carry on. Do not start subagents. Work until the task is
  finished or you are truly stuck, then write the report.

## The task

1. **A world** in the drawing style named in your task message, with a name and a sentence or
   two about it.
2. **Two original characters** in that world who look different from each other and from the
   style's plain template: at least their own colours, and for at least one of them a redrawn
   part (hair, or a top). Each has a one-line personality. Check them with the tool and render
   a sheet of the cast.
3. **A story** of eight to ten pages and 250 to 450 words, for children aged four to six, in
   which the main character wants something, something is in the way, and something has
   changed by the end. Use at least three different settings or lights, at least one
   close-up, at least one moment where a character walks across the page, and dialogue for
   both characters.
4. **Check and review it** with the tool until the check passes and the review reports no
   `fix` findings. Keep each version of the review output.
5. **Storyboard it**, look at what the storyboard tells you (its labels are text you can read
   in the tool's output or infer from your script; if you can view images, look at it), and
   make at least one change to the script because of it.
6. **Render** the page proofs and a landscape preview video (`--preview`).

## The report

Write `<studio>/REPORT.md`, then repeat it as your final message:

- The world, the characters and a three-sentence summary of the story.
- Every command you ran that failed, in order, with its error message copied exactly, and
  what you did next. Count them.
- Everything you had to guess, could not find in the documentation or help, or found
  described wrongly. Be specific: quote the documentation line or say where you expected it.
- Anything the tool let you do that produced a bad result without warning.
- Which of the six steps you completed, with the paths of the files that prove it (the cast
  sheet, the story, the review outputs, the storyboard, the proofs' contact sheet, the
  video).
- The final `check` and `review` output, copied exactly.
- How long each step took you, roughly, in tool calls.
- Three changes to the tool or its documentation that would have saved you the most trouble,
  most valuable first.
