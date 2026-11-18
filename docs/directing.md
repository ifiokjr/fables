# Directing

A director's guide to fables: the rules of craft that make a page work, a scene read and a
cartoon hold together, each tied to the script vocabulary that carries it out. The `guide`
tool serves these pages by topic; `fables review` applies them to a script and reports what
it finds.

Everything here is a rule of thumb. The engine will build whatever parses; these rules are
about building something a child asks for again.

## Overview

Directing a fables book is three passes over one script:

1. **Write the story** (`story` below): a want, an obstacle, a change — one idea a page, in
   words that are good to say out loud.
2. **Stage it** (`staging` and `acting`): every page gets a picture worth looking at, and
   every beat a direction that lands.
3. **Shoot it** (`camera` and `light`): let the automatic director carry the load, and write
   `@camera` and `light=` only where the story asks for more.

Then look before you render video: `story_review` for the script, `storyboard_render` for
the shots, `video_frames` for moments in time. Video is the slow, expensive output; do not
use it to discover problems the review already names.

The topics: `story`, `staging`, `camera`, `light`, `acting`, `mixing`, `workflow`.

## Story

A picture book is a want, an obstacle, and a change.

- **A want the reader can hold.** State it in the first two pages, in the star's own words or
  actions. `@mo x=0.4 worried talk` then a line about what he wants.
- **An obstacle that grows.** Each attempt fails worse than the last; the low point is the
  darkest page (`light=` it accordingly).
- **A change, not a lesson.** The last page shows the star doing something they could not do
  on page one. End on an image — the wind settling, two characters flying one kite — and let
  the `moral = "…"` line in the front matter say the lesson only if the picture already
  shows it.

**One idea per page.** If a page's text contains an "and then", it is two pages. Turn pages
on the story's beats: attempt, failure, turn, attempt, change.

**The rule of three.** Two attempts establish a pattern; the third breaks it or fulfils it.
Three guesses, three knocks, three warnings. Break the pattern on the page turn — the reveal
that lands is the one the reader turns to.

**The page turn is a reveal.** Write the setup at the bottom of the text card, and let the
next page's picture answer it. `text=bottom` on the setup page puts the words next to the
reader's thumb.

**A refrain a child can join.** Repeat one line, the same words, at least three times, and
change it once at the end. Shout it (`**WORD**`) where the story peaks; the video shouts it
too.

**Read-aloud rhythm.** Say every page out loud before you keep it. About 45 words a page is
a breath; over that, a reader starts skipping. Whole books land between 250 and 900 words:

| Age band | Words a page | Words a book |
| --- | --- | --- |
| Under 3 | 5 to 15 | up to 200 |
| 3 to 5 | 20 to 40 | 250 to 500 |
| 5 to 7 | 30 to 45 | 400 to 900 |

`fables check` reports the word count; `fables review` flags pages over the limit and books
outside the range.

**The world's recipe.** `worlds/<world>/recipe.md` is a twelve-beat outline `fables new`
starts a book from. Map your want-obstacle-change onto its beats; they are already in page
turn order. Do not keep a beat the story does not need — delete pages, do not pad them.

**Story flows.** Nine laid-out shapes are built into fables: `fables flow list` prints them and
`fables flow show <name>` prints one's beats. Start a book with
`fables new "<Title>" --star <id> --flow <name>`, and add `--friend <id>` for a flow that
stages two or a crowd. A world can add its own in `flows/<name>.md`, found before the built-ins.
A fresh scaffold is full of `[bracketed]` lines; `fables review` says how many are left rather
than critiquing lines nobody has written.

- **Three Tries.** A want said early, three tries that fail bigger, then a fourth that uses
  what the failures taught. Choose it when the ending is already visible and the pleasure is
  the trying. The trap is giving the fourth try a brand-new idea: it must use something the
  first three showed.
- **The Pile-Up.** One sound gathers a newcomer a page, each page restates the whole chain,
  and the noise brings the house down. Choose it for a book a child will ask for again and
  can join in with. The trap is restaging only the newcomer, so the chain vanishes behind
  them; repeat the whole line every page, and give the crash a heap to laugh in.
- **There and Back.** A reason to leave, three places that get stranger, a turn on one face,
  then a quick way home to a changed star. Choose it when the star has something to find.
  The trap is a return as long as the journey out: the way home is short, and the last page
  shows the souvenir rather than explains it.
- **Round We Go.** One tiny cause, seven consequences that each cause the next, and the
  opening image again with one thing changed. Choose it for a chain a child can see coming
  and enjoy predicting. The trap is an "and then" chain where nothing causes anything; every
  page must hand the cause to the next.
- **The Odd Couple.** Two opposites meet, are forced together, clash three times, miss each
  other, and each needs the other's way. Choose it for character comedy and dialogue. The
  trap is making them merely different: they must be opposite in shape, tempo and volume,
  and both change only a little.
- **Wrong End of the Stick.** The reader sees the truth, the star misreads half of it,
  three helpful acts make it worse, and the page turn reveals it. Choose it when the pictures
  can carry a joke the words hold back. The trap is letting the words mention the truth, or
  punishing the star for being wrong; being wrong is funny and costs nothing.
- **The Goodnight Tour.** A small evening problem settles, the star says goodnight to one
  place a page, and the last line is a whisper. Choose it for bedtime, a calm ending and a
  child naming their world. The trap is raising the stakes after dusk or cutting where a
  dissolve belongs; the word count falls with the light.
- **The Gag Short.** One want, three gags that fail bigger, a twist with no anticipation, and
  a button. Choose it for a video-first sixty to ninety seconds where timing is the story.
  The trap is explaining the joke in the text or easing into the twist: keep one locked camera
  per gag and cut into the surprise.
- **The Habit Tale.** The world's rule, the kid, the NOPE, the habit showing, three
  consequences, a name, a cost, a choice, a magic, and the callback. Choose it when the
  change is a choice the star makes and the habit can be seen on the outside. The trap is
  stating the moral, or making the cost cruel; the cost is social, gentle and real, and the
  last page answers the first refusal.

## Staging

One clear action per beat.

- **Move, then speak.** The `@` direction goes above the paragraph it belongs to, so the
  move lands in the narrator's first words and the line arrives on a face that is already
  set: `@mo point cross talk` before the quoted line.
- **Who is on stage, and why.** Everyone on a page is there for the star's story. An extra
  who only stands there is a distraction; `fables review` calls them an idle arrival. Give
  them a reaction (`@dog shocked`, `@mum laugh`) or take them off.
- **Silhouettes.** A reader should recognise who does what from shape alone at proof size.
  Space people apart (`x=0.3`, `x=0.7`; less than 0.15 apart and they read as one shape),
  separate their levels (`size=`, `sit`, `stand`), and let facing tell the story: `face=`
  towards whoever matters.
- **Eyelines.** Everyone looks at whoever matters — the engine aims listeners at the speaker
  for you. Break it deliberately: the one character looking away is the one hiding
  something. `face=left`/`right`/`front` and where they look is the only tell a still has.
- **Entrances and exits are story beats.** `enter=left` walks somebody on in the video and
  pops them into the still; give the arrival a beat to land (a reaction, a line) before the
  page's business starts. Exits (`exit=right`) say goodbye the same way.
- **Let a reaction land.** After a big line, the listener's face is the story. Give it a
  second: `@pause 0.8`, or the next direction on a new line, before the next paragraph. A
  `shocked` in the same `@` line as a `talk`, with no pause before the next line, is a
  reaction nobody sees.
- **Contrast between pages.** No two facing pages should look the same: change the setting,
  the `light=`, the distance (`shot=`), or the staging. A page whose stage is identical to
  the last one's has nothing to look at; more than three pages in one place under one light
  is a sequence, so make it one on purpose.

## Camera

What each shot size says:

- `wide` — the world; the star is small in it. Open sequences and establish scale.
- `full` — head to toe: movement, entrances, walks, the body's comedy.
- `medium` — the waist up: conversation, two people trading lines.
- `close` — head and shoulders: a decision, a feeling held back.
- `extreme-close` — the face alone: the turn, the gasp, the thing only the reader notices.

**Cut or hold.** Cut when something new must be seen at once; hold when a feeling grows. A
hold that earns nothing is dead air — the storyboard's panel captions give every setup's
length; `fables review` flags pages that flash past under two seconds.

**Shot and reverse shot.** In a two-way exchange, alternate who is framed; the automatic
director already does this for you when the script says nothing.

**The reaction shot.** The line is half the joke; the listener's face is the other half. On
a page whose biggest event is spoken, frame the person who is *not* speaking:
`@camera shot=close on=mo`.

**The establishing shot.** After any big change of place or time, let the reader see where
they are before going in close. The automatic director opens every page wide or full for
exactly this.

**Moves that mean something.** `push` tightens attention; `pull` gives the character room to
breathe after a peak; `pan-left`/`pan-right` reveal; `tilt-up`/`tilt-down` measure height;
`track` stays with a walk. A move with no reason is a wobble: one deliberate move in a scene
beats three idle ones. `shake=` is an impact, not a style.

**Transitions say how time passes.** `cut` is the same moment, another angle. `dissolve`
is a little time. `fade` is a chapter's end. `wipe` is a page turn, the book's default.
`iris` is old-fashioned attention. `push` chases the story forward. When the text jumps in
time ("later", "the next day"), say so: `transition=dissolve`, not another wipe.

**Portrait versus landscape.** The 9:16 portrait video frames one or two people and
favours close shots and high captions; the 16:9 landscape carries groups and movement.
Storyboard both before rendering video if the Shorts cut matters.

**When to trust the automatic director.** `[video] direction = "auto"` (the default) already
establishes, alternates shot and reverse shot, widens for entrances and walks, gives a close
to a strong mood on somebody silent, and never cuts faster than one setup each 1.2 s. Trust
it for whole scenes; write `@camera` only where it cannot know better — a specific reveal, a
held look, a shake on an impact. A page with any `@camera` direction is shot exactly as
written, both modes.

## Light

The colour script: one light per sequence, changing at the story's turns.

- Give each sequence a `light=` that says its time of day and its mood: morning attempts in
  `day`, the golden plan in `golden`, the low point in `storm` or `night`, the turn in
  `dawn`, the ending in `golden` or `moonlit`.
- Change light at a turn, not at random: the page where the story changes direction is the
  page that changes light. `@scene light=dusk` mid-page eases the change over about a
  second in the video — evening falling while somebody speaks.
- Contrast the low point and the ending. If the darkest page and the last page have the same
  light, the ending has no arrival. `light=` is the cheapest emotional special effect in the
  script.
- The presets feel like this: `day` plain and open; `dawn` cool with a warm horizon, the
  start of hope; `golden` honeyed, nostalgic, the day at its best; `dusk` violet and
  settled; `night` dark and closed; `moonlit` silver and calm, the world asleep; `overcast`
  flat and waiting; `storm` the darkest, for the worst page; `indoor` ordinary, at home;
  `candle` small and safe, one person's worth of light. `light_list` serves this list.
- If the words move through the day (morning to night, dawn to sunset) and every page keeps
  the same light, the pictures contradict the words; `fables review` flags it.

## Acting

Moods are an arc, not a wallpaper. The star's mood should move with the story, and the last
page's mood should not equal the first — that is the change, drawn on a face. Fifteen moods
exist; they are listed in the readme's direction table, and `@mo shocked` is a direction, on
a line, at the moment it happens.

- **Poses and stances that show the mood.** Do not narrate a feeling and draw an idle body:
  pair `worried` with `hold` or `droop`, `proud` with `cheer` or `hips`, `defiant` with
  `arms-crossed`. A stance can contradict a mood on purpose — `grin` over `sit` reads as
  smug.
- **The take.** A change to `shocked`, `disgusted` or `fainted` plays as a classic take in
  video: squash, stretch, settle. Give it room — a `@pause` before the next line, or the
  direction on its own line — so the reader sees the face change.
- **Walking is intent.** `@mo x=0.7` from `x=0.3` is a walk across the stage on his own
  feet, facing where he is going, the camera widening to keep him. Entrances and exits walk.
  The on-the-spot `walk` and `run` gaits are for treadmill shots: running without arriving.
- **What the engine adds unasked.** Eyes lead turns, overlapping arrival, arcs, anticipation
  and settle, takes, breathing and weight shifts, speech beats and brow lifts, listening
  nods and flinches, idle business on long holds. Leave room for it: a pause, a held look, a
  beat with no direction. Do not stack a direction on every sentence; the engine performs
  between them, and it needs those gaps.
- **Checking.** If a moment reads wrong, render once with `[video] acting = "plain"`:
  whatever still reads wrong is yours to stage differently. `docs/animation.md` has the ten
  rules and their dials.

## Mixing

Guests — characters from other worlds in one story — are the whole point of a crossover, and
the risk is mush.

- **Invite them by name.** `guests = ["kiteharbour/hana"]` in the front matter; direct them
  by id (`@hana`), or by `@kiteharbour.hana` if the host world has a `hana` of its own. A
  missing world or character is an error naming the ones that exist.
- **Style contrast is a storytelling device.** A guest keeps their own construction, line
  and shading by default: a round cartoon kid beside a slender anime kid says "from another
  world" on every page, no words needed. `guest-style = "host"` re-inks guests with the
  host's `[line]` and `[shade]` when you want one book, one finish — their bodies never
  change. Choose per book, not per page.
- **Keep voices distinct.** Each world's cast keep their own voices; a crossover video with
  two characters alike to hear loses the "two worlds" instantly. `voice_audition` both stars
  on one line each and listen.
- **Scale between worlds.** Everyone stands at the height their own world gives them, so a
  guest reads as tall or small at home. Play against it deliberately: `size=` a giant down,
  a chibi up, and let the staging notice.
- **Give the guest a reason.** A crossover is not a parade: the guest's want must tangle
  with the star's. The best crossover pages are two styles reacting in one frame —
  `shot=medium` on both, or a close on each in turn.
- Motion is the host's: everyone on a page moves with the host story's `[motion]`, so a
  manga guest in a cartoon book moves on ones, not threes. Keep that in mind when a guest's
  way of moving is part of who they are.

## Workflow

The loop, from nothing to published:

1. **World and cast.** `world_list`, then `world_create` (or `fables new` for a book in an
   existing world). Design characters: `style_describe`, `character_create`,
   `character_set_parts`, `character_sheet`, repeat until they read as themselves. A person
   signs off every model sheet.
2. **Model sheets and lights.** `character_sheet` for the moods; `light_list` for the
   presets. Know what your star's face does before writing it.
3. **Script.** `story_write` the whole story.md — front matter, pages, directions. Sketch the
   colour script as you go: a `light=` per sequence.
4. **Check.** `story_check`: does it parse, rehearse and lay out? Fix what it names, by
   line.
5. **Review.** `story_review` (or `fables review`): does it *work*? Fixes first, then
   consider the considers. Re-run until the fixes are gone and you have argued with every
   remaining finding.
6. **See it.** `storyboard_render` for the camera plan, panel by panel; `video_frames` for
   moments in time without encoding a video; `page_render` for single pages; `book_render`
   for proofs and the contact sheet. Look at everything.
7. **Fix, then page renders.** Change the script, not the renders: every finding names a
   line, every panel names a setup.
8. **Video last.** `video_render` a preview first; the full render is minutes, the preview
   is seconds. The voice cache means re-renders cost nothing new. Then `fables all` for the
   book.
