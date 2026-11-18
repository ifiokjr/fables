+++
title = "{{title}}"
author = "{{author}}"
year = {{year}}
star = "{{star}}"
moral = "[One line: how the smallest thing comes all the way back round.]"
blurb = "[The back-cover blurb: one tiny cause, seven consequences, and the same image at the end.]"

[cover]
setting = "garden"
tint = "sunflower"
pose = "x=0.5 grin wave"

[video]
cast-voices = true

[flow]
name = "Round We Go"
about = "One tiny cause sets off seven consequences, each causing the next, and lands back on the opening image."
good-for = "A chain a child can follow, where the fun is seeing the next link coming."
pages = [8, 12]
words = [180, 320]
needs = "1"

[[flow.beats]]
name = "The opening image and a tiny cause"
pages = 1
words = [15, 25]
note = "Make one memorable composition and note it exactly: the last pages will return to it."

[[flow.beats]]
name = "Consequences, each causing the next"
pages = 7
words = [20, 30]
note = "One new setting a page; end each page by handing the cause off to the right, so the turn is the chain's next link."

[[flow.beats]]
name = "Back to the opening image, one thing changed"
pages = 1
words = [15, 25]
note = "The same setting, light and framing as page one, with exactly one thing different."

[[flow.beats]]
name = "The tiny cause, again"
pages = 1
words = [5, 10]
note = "The same framing once more, and the smallest possible line; iris out on the joke."

+++

// Round We Go: one tiny cause, seven consequences, and the same image at the end.
// Replace every [bracketed] line with the story, keep the directions that fit, and run
// `fables proofs` to see it.

// Beat 1: The opening image and a tiny cause. One page, 15–25 words.
// Note this framing exactly: setting, light, shot and where everyone stands. Page 9 returns
// to it. The cause is at the edge of the frame, in plain sight.
# garden day shot=close on={{star}}
@{{star}} x=0.4 sleepy
@dog.a x=0.85 sleepy
// 15–25 words: the opening image, and the tiny thing about to happen.
[The opening image, and the tiny cause.]

// Beat 2: Consequences, each causing the next. Seven pages, 20–30 words.
// One setting a page, pushed into; the last direction on the page hands the cause off
// rightward.
# street day transition=push shot=medium on={{star}}
@{{star}} x=0.3 shocked
@bird.a x=0.55 happy
@bird.a x=0.9 exit=right
// 20–30 words: the first consequence, and how it passes on.
[The first consequence.]

# park golden transition=push shot=full on={{star}}
@{{star}} x=0.3 worried
@dog.b x=0.55 wag
@dog.b x=0.9 exit=right
// 20–30 words: the second consequence, caused by the first.
[The second consequence.]

# shop indoor transition=push shot=medium on={{star}}
@{{star}} x=0.35 grin
@bird.b x=0.6 happy
@bird.b x=0.9 exit=right
// 20–30 words: the third consequence, and it is getting bigger.
[The third consequence.]

# town day transition=push shot=wide on={{star}}
@{{star}} x=0.3 sad
@worm x=0.55 happy
@worm x=0.9 exit=right
// 20–30 words: the fourth, out in the open where everyone can see.
[The fourth consequence.]

# hilltop dusk transition=push shot=wide on={{star}}
@{{star}} x=0.35 dreamy
@flower.1 x=0.6 happy
@flower.1 x=0.9 exit=right
// 20–30 words: the fifth, and the light is going.
[The fifth consequence.]

# beach night transition=push shot=medium on={{star}}
@{{star}} x=0.3 happy
@tree x=0.6 happy
@tree x=0.9 exit=right
// 20–30 words: the sixth, far from where it started.
[The sixth consequence.]

# forest dawn transition=push shot=close on={{star}}
@{{star}} x=0.4 shocked
@bird.c x=0.6 happy
@bird.c x=0.9 exit=right
// 20–30 words: the seventh, coming back round.
[The seventh consequence, coming back round.]

// Beat 3: Back to the opening image, one thing changed. One page, 15–25 words.
// Same setting, light, shot and places as page 1; change one thing only.
# garden day shot=close on={{star}}
@{{star}} x=0.4 happy
// 15–25 words: the opening image again, one thing changed.
[The opening image, one thing changed.]

// Beat 4: The tiny cause, again. One page, 5–10 words.
// The same framing, the smallest line in the book, and the iris.
# garden day shot=close on={{star}} transition=iris
@{{star}} x=0.4 sleepy
@dog.a x=0.85 sleepy
// 5–10 words: the cause again, and out.
[The tiny cause, again.]
