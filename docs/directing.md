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
the shots and their text shot list, `storyboard_pin` to make the plan editable, `animatic_render`
for the timing, and `video_frames` for moments in time. Video is the slow, expensive output; do
not use it to discover problems the review already names.

The topics: `story`, `staging`, `camera`, `light`, `acting`, `mixing`, `sound`, `workflow`.

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
a breath; over that, a reader starts skipping. Whole books land between 250 and 900 words.
The band's own limits live in one table, `Age::READ_ALOUD` in `src/story.rs`, so a future
review can vary its figures by band without a second copy:

| Age band | Words a page | Words a book |
| --- | --- | --- |
| `under-3` | 1 to 25 | up to 250 |
| `3-5` | 15 to 45 | 250 to 900 |
| `5-7` | 25 to 60 | 400 to 1400 |

`fables check` reports the word count; `fables review` flags pages over the limit and books
outside the range. The band also sets how fast the narrator reads; see `sound`.

**The world's recipe.** `worlds/<world>/recipe.md` is a twelve-beat outline `fables new`
starts a book from. Map your want-obstacle-change onto its beats; they are already in page
turn order. Do not keep a beat the story does not need — delete pages, do not pad them.

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
hold that earns nothing is dead air — the storyboard's shot list gives every shot's
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
written, both modes. The shot list shows the whole plan; `storyboard_pin` (or
`fables storyboard --pin`) writes it into the script as `@camera` lines marked `// pinned` to
edit, and `unpin` removes them again. A pinned script reproduces the plan.

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

## Sound

Sound is what brings a young child's eyes back to the screen. Use it as punctuation, not as
wallpaper.

- **Choose the cue by the scene's feeling, not by the page number.** `play` is the bouncy
  default; `calm` (slower, sparser, major) for golden light and safe moments; `tense` (minor,
  a low pulse) for storms and the beat before a fright; `sad` for the low point; `sleepy` (a
  music box) for bedtime, moonlight and candlelight; `bright` for a celebration, and `none`
  for a page that should be heard as a place rather than a tune. A page's `light=` picks one
  for you; `music=` on the heading or `@music` mid-page says otherwise, and `[video] music =
  "play"` holds one loop for the whole film.
- **Put silence before a peak.** A shouted line from a character whose mood has just turned
  cross, defiant, shocked or laughing gets half a second with no music before it and a breath
  after it: the line lands in air instead of on top of a strum. Do not stack a `@sfx` on the
  same beat; let the silence do the work.
- **One sound idea a beat.** An entrance pops, a faint bonks, a hop boings, and the picture
  gets those for free. Add `@sfx` only where the beat needs a sound the picture does not
  already have: `@sfx tada` for the win, `@sfx drum-roll` before the reveal, `@sfx heartbeat`
  for the quiet scare, `@sfx thud` for the fall that hurts. Two effects on one beat is a
  cartoon clatter, not a story.
- **Let ambience carry the quiet pages.** A setting's `data-ambience` and the scene's light
  give the film birds, breeze, waves, rain, wind, crickets, a room tone or a crowd, mixed 25 dB
  under the narration. On a page with little text, set `music=none` and let the place be
  heard; `@scene ambience=none` silences it when the story wants nothing at all.
- **Stop for a look.** `hold=1.2` on a heading, or `@hold 1.5` mid-page, is a dead stop: the
  voice, the music and the ambience all rest, and the performance stops fidgeting. Use it for
  a reveal that needs a beat, never for a page that is already quiet.
- **Pace it for the listener.** `age = "under-3" | "3-5" | "5-7"` in the front matter sets the
  narration rate (about 125, 135 and 150 words a minute) and the air around the words: longer
  gaps and tail, and a page in a new place is seen for at least a second and a half before it
  is talked over. A narrator with an explicit `rate` keeps it, and a book with no `age` keeps
  the older, brisker timing; `fables check` suggests one.

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
6. **Storyboard.** `storyboard_render` (or `fables storyboard`) draws one panel per shot and
   returns the shot list as text: page, script line, start and duration, shot size, subjects,
   move, entry, who chose it and the words spoken. Read it even when you cannot see the board.
7. **Pin and edit.** `storyboard_pin` (or `fables storyboard --pin`, optionally
   `--pin page=3,5`) writes the plan into the script as `@camera` directions marked
   `// pinned`. Edit, delete or reorder them like any direction; pages that already direct
   their own camera are left as written. `storyboard_pin` with `unpin` removes them again.
8. **Animatic.** `animatic_render` (or `fables animatic`) holds each panel for its real
   duration over the soundtrack, labelled and timecoded. Judge pacing here, then fix the
   script — not the render.
9. **Fix, then page renders.** Change the script, not the renders: every finding names a
   line, every shot names a setup.
10. **Video last.** `video_render` a preview first; the full render is minutes, the preview
    is seconds. The voice cache means re-renders cost nothing new. Then `fables all` for the
    book.
