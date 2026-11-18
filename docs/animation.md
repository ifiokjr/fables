# Animation

A fables video performs its book. The script stages the beats; the engine animates between
them using the rules animators have used since the thirties. This page says which rules the
engine uses, what the script controls, what the engine adds on its own, and how to stage a
beat so it reads.

Everything here is video-only. A book's pages are stills and never change: a page draws the
moods, poses and facings its script directs, at their full strength.

## What the script controls

The writer stages. Every `@` direction lands as the narrator reaches it, and each one answers
"who does what":

- **Moods** (`happy`, `cross`, `shocked`, `fainted`…) set the face, and a change of mood is
  itself a performance — shock plays as a take, sadness sinks.
- **Poses** (`wave`, `point`, `arms-crossed`…) place the arms; **stances** (`sit`, `crouch`,
  `tiptoe`…) place the legs and spine.
- **Gaits** (`walk`, `run`) go to the actor's `x`, or to the `x` it is given next on the
  page. With nowhere to go they cycle on the spot for a treadmill shot.
- **`x=`** places an actor on the stage. Moving an `x` is how you make someone cross it —
  they walk there on their own feet.
- **`talk`** gives the next paragraph to an actor's mouth, lip-synced.
- **Shots** (`shot=`, `@camera`) frame it; the automatic director already covers entrances,
  walks and reactions.

## What the engine adds on its own

Ten rules run on every move between stage directions. Each is a dial in a style's `[motion]`
table, so every drawing style keeps its own way of moving. `[video] acting = "plain"` in a
script's front matter turns the whole layer off and reproduces the plain tween.

1. **Eyes lead.** When a figure's gaze, facing or position changes, the eyes go first — there
   within two frames of film — the head turn starts about 0.08 s later, and the torso and arms
   follow 0.08 s after that. A blink covers any head turn of more than a few degrees and any
   big gaze shift. Serves *timing* and *staging*. Dial: `lead = 1.0`.
2. **Overlapping action.** Parts do not arrive together. The hand furthest from its target
   arrives last, the head settles after the body, and loose parts (hair, clothes, already
   driven by follow-through) settle last of all; the offsets scale with how far each part
   travels. Serves *follow-through and overlapping action*. Dial: `overlap = 1.0`.
3. **Arcs.** A hand moving between two poses swings on an arc about its shoulder — the angle
   and reach from the shoulder are interpolated, not the hand's x and y — unless the straight
   line is shorter than a small threshold. A head turn nods through the turn rather than
   sliding level. Serves *arcs*. Dial: `arcs = 1.0`.
4. **Anticipation and settle by distance.** A blink-sized change gets no wind-up; a jump or a
   point gets the most. The `anticipation` and `overshoot` dials keep their meaning and scale
   this, and a held pose settles with one small damped after-beat. Serves *anticipation* and
   *slow in, slow out*.
5. **The take.** A mood change to `shocked`, `disgusted` or `fainted` plays as a classic
   take: a short squash down and in, a stretch up past the pose with eyes wide, then the
   settle. A change to `laugh` (or the `cheer` pose) bounces; a change to `sad`, `sleepy` or
   `droop` sinks slowly with no overshoot. Serves *squash and stretch* and *exaggeration*.
   Dial: `takes = 1.0`.
6. **Breathing and weight.** A standing figure breathes — a slow chest and shoulder rise
   about every 3.5 s, phased from the actor's seed, faster when shocked or laughing or just
   after running — and shifts its weight from foot to foot every several seconds, with a
   small hip and spine counter-tilt, never mid-line while speaking. Sitting and lying figures
   only breathe. The `idle` bounce and `moving-hold` stay and stack. Serves *secondary
   action*. Dials: `breath = 1.0`, `weight-shift = 1.0`.
7. **Speech performance.** While a character speaks, more than the mouth moves. Accents —
   peaks in the line's loudness — give small head beats, and the strongest peak of a line
   gets a hand beat in the current pose. Brows lift on a question and drop on a line spoken
   cross or grumpy; a `**shouted**` word leans the whole body in. Between sentences the
   speaker's gaze flicks away and back. Serves *secondary action* and *timing*. Dial:
   `gesture = 1.0`.
8. **Listening.** Everyone looks at the speaker already; now they also nod once or twice
   through a long line (over 2.5 s), glance at whatever the speaker points at (`point`,
   `reach-out`), and flinch a beat late when the speaker shouts. Their blink timing stays
   their own. Serves *staging* and *secondary action*. Dial: `listen = 1.0`.
9. **Walking is travelling.** When an actor's `x` changes and it has a figure body, it walks
   — or runs, if the script says `run` or the distance is long for the time available —
   facing the way it is going, taking steps whose length matches the distance covered so
   planted feet hold still, easing in and out of the walk over half a step, and turning back
   to its staged facing on arrival. `enter=` and `exit=` walk on and off for figures
   (critters keep their own entrances). The on-the-spot `walk` and `run` gaits stay for a
   treadmill shot. Serves *slow in, slow out*. Dial: `travel = "walk"` or `"slide"`.
10. **Hold lengths.** After about 4 s with no direction, speech or reaction, an on-screen
    figure does one small piece of idle business picked by its seed and mood — looks around,
    scratches, shifts, sighs or adjusts its stance — never during someone else's shouted
    line. Serves *appeal* and *secondary action*. Dial: `business = 1.0`.

## What a plain script means

Before any of those rules run, one pass reads each page's directions and works out what
they mean on screen, so a script that only names moods and places still gets a performance.
`[video] acting = "plain"` skips it with the rest.

- **A take finishes.** `shocked`, `disgusted` and `laugh` hit, hold their peak for about a
  second, then relax into a quieter version of the same feeling: the mouth closes most of
  the way on a blink, the brows ease, and breathing, blinks and glances carry on. A faint
  stays out cold. Dial: `hang = 1.0`, the seconds at the peak; `0` never relaxes.
- **A reaction waits for its cause, and looks at it.** A `shocked`, `disgusted` or `laugh`
  directed before somebody or something arrives later on the page is not played early. The
  actor holds a mild version (`worried`, `grumpy`, `happy`), looks to the side the newcomer
  comes from as they start to arrive (their `enter=` side, or at them), and takes about a
  second later, when they are on stage. A take directed on the same line as an entrance
  waits the same way. A page whose cause never arrives plays the take where it is directed.
  This builds on the eyes-lead rule and on `notice`; a `see=` the script directs is kept.
- **A run goes somewhere.** `run` or `walk` with no `x` runs to the `x` the actor is given
  next on the page, setting off when it is directed (or as the page's first words start),
  and the actor stands when it arrives. A directed `run` lasts at least 0.9 s, so the legs
  are seen to go. A `jump` with an `x` runs up and leaps; a jump is a crouch, air, a landing
  with squash, and standing again.
- **Faces fit what is said.** A `**shouted**` line opens the eyes and the mouth whatever the
  mood, so a `proud` shout is not delivered with the eyes shut. A line spoken `sad`,
  `worried` or `sleepy` keeps the mouth small. A face whose eyes are shut in a smile
  (`proud`, `laugh`, `dreamy`) opens them while somebody else is speaking.
- **Two people talking face each other.** When a page has exactly two people and one of
  them speaks, both turn most of the way towards each other for the page. A `face=` in the
  script wins, and a lone speaker stays as staged.
- **A pose that fits the mood.** A standing figure with no pose directed and nothing in its
  hands takes its mood's pose:

  | Mood | Pose when none is directed |
  | --- | --- |
  | `happy`, `grin`, `dreamy`, `dizzy`, `disgusted`, `fainted` | `idle` |
  | `laugh` | `cheer` |
  | `sad`, `sleepy` | `droop` |
  | `grumpy`, `defiant` | `arms-crossed` |
  | `cross`, `proud` | `hips` |
  | `shocked` | `shrug` (both hands up and out) |
  | `worried` | `think` |

- **Nobody stands exactly alike.** Each figure carries its head a little differently, by
  its seed, on top of its own breathing, blink and weight-shift timing. It rides the
  `weight-shift` dial.

## The dials

Every style sets its own values in `[motion]`; these are the defaults (the cartoon look):

| Dial | Default | Turns |
| --- | --- | --- |
| `timing`, `modulate` | ones, off | how often drawings change |
| `easing` | eased | snap holds instead of eased moves |
| `anticipation`, `overshoot` | 1.0 | the wind-up and overshoot every move gets |
| `squash` | 1.0 | hop squash and the depth of takes |
| `follow-through` | 1.0 | hair and clothes swinging after a move |
| `moving-hold` | 0.0 | a slow drift while a pose is held |
| `smears` | 0.0 | speed lines behind fast moves |
| `limbs` | jointed | hose limbs, one curve, no elbows |
| `blink-every` | 3.0 | seconds between blinks |
| `mouth-rate` | 0 (follows the voice) | mouth flaps a second |
| `idle`, `idle-amount`, `idle-tempo` | none | the standing bounce |
| `lead` | 1.0 | eyes first, head, then body |
| `overlap` | 1.0 | parts arriving one after another |
| `arcs` | 1.0 | hands on arcs, heads nodding through turns |
| `takes` | 1.0 | the depth of mood-change takes |
| `hang` | 1.0 | seconds a strong mood holds its peak before it relaxes (0 never relaxes) |
| `breath` | 1.0 | breathing |
| `weight-shift` | 1.0 | weight moving foot to foot |
| `gesture` | 1.0 | speech beats, brows, the shout lean |
| `listen` | 1.0 | nods, glances, the flinch |
| `business` | 1.0 | idle business on long holds |
| `travel` | walk | walking on your own feet, or sliding |

What each style makes of them:

| Style | The way it moves |
| --- | --- |
| `cartoon`, `anime` | the defaults: everything on, gently |
| `rubber-hose`, `rubber-hose-1930s` | exaggerated: `takes` 1.5, `overlap` 1.5, on top of its big wind-ups and idle bounce |
| `manga`, `manga-colour`, `manga-shonen` | limited: moves on threes snap between held poses, so `overlap`, `breath`, `weight-shift` and `business` sit low or off |
| `mid-century` | the most limited of all: snapping keys, `weight-shift` and `business` off |
| `manga-shojo` | gentle: soft takes, light breath and weight, poses that drift |
| `chibi` | springy: deeper takes, more overlap, never still for long |
| `clear-line` | restrained: half-depth everything, slightly quieter arcs |
| `silhouette` | a jointed shadow puppet: on twos, `squash` 0 and `breath` 0 (card neither stretches nor breathes), small wind-ups, shallow `takes`, a little `weight-shift` |
| `paper-theatre` | card on sliders: on twos, snapping between held keys like `mid-century`, with `squash`, `breath`, `weight-shift` and `business` off |
| `soft-flat` | the cartoon's motion turned down: on ones, eased, with gentler wind-ups, overshoot and squash |

`[video] acting = "plain"` in a script's front matter switches the whole layer off: one
easing curve per move, entrances slide, no performance. Useful for checking that a staging
problem is a staging problem and not the engine's idea of one.

## How to stage a beat

- **One idea per beat.** A direction is a beat: someone turns, someone points, someone walks.
  Two ideas in one direction fight each other.
- **Move, then speak.** Put the `@` direction before the paragraph, so the move has the
  narrator's first words to land in, and the line arrives on a face that is already set.
- **Let a reaction land before the next line.** The listener's nod, glance or flinch needs
  about a second; a direction that changes the subject straight after a line steals it.
- **Change `x` to make someone walk.** `@mo x=0.7` from `x=0.3` is a walk across the stage,
  on its own feet, and the camera widens to keep it in frame.
- **Use the camera last.** `shot=` and `@camera` frame what the staging already says; when a
  beat is unclear in a wide shot, the staging is the thing to fix.
- **Check with `plain`.** If a moment reads wrong, render once with
  `[video] acting = "plain"`: whatever still reads wrong is yours to stage differently.
