+++
title = "{{title}}"
author = "{{author}}"
year = {{year}}
star = "{{star}}"
age = "3-5"
moral = "[One line, only if the last gag needs one.]"
blurb = "[The back-cover blurb: one want, three gags, and the twist nobody saw coming.]"

[cover]
setting = "plain"
tint = "sunflower"
pose = "x=0.5 grin wave"

[video]
cast-voices = true

[flow]
name = "The Gag Short"
about = "A video-first short: one want, three gags that fail bigger, and a twist with no anticipation at all."
good-for = "A sixty-to-ninety-second cartoon where the timing is the story and the words are almost nothing."
pages = [6, 9]
words = [40, 120]
needs = "1"

[[flow.beats]]
name = "Normal"
pages = 1
words = [5, 15]
note = "One full shot, one locked camera: show the world as it is before the funny starts."

[[flow.beats]]
name = "The want, or the obstacle"
pages = 1
words = [5, 15]
note = "A close on the one thing in the way; the gag is the star trying to move it."

[[flow.beats]]
name = "Gag 1: setup, beat, payoff"
pages = 1
words = [0, 10]
note = "Hold the beat before the payoff, and land the sound effect just after the action, not on it."

[[flow.beats]]
name = "Gag 2, bigger"
pages = 1
words = [0, 10]
note = "The same framing as gag one, so the bigger failure reads as a variation, not a new joke."

[[flow.beats]]
name = "Gag 3, biggest, fails with a take"
pages = 1
words = [0, 10]
note = "A double-take, then the take, and leave a hole after the failure: the joke needs it."

[[flow.beats]]
name = "The twist: a payoff with no anticipation"
pages = 1
words = [0, 10]
note = "Cut into the surprise; easing into it gives the audience time to see it coming."

[[flow.beats]]
name = "Button, iris out"
pages = 1
words = [3, 8]
note = "Three to eight words, and the iris: end on the image, not the explanation."

+++

// The Gag Short: one want, three gags, and a twist with no anticipation.
// Replace every [bracketed] line with the story, keep the directions that fit, and run
// `fables storyboard` to see the timing before you render video.
// Every "quoted" line needs its speaker on the line above it: `@{{star}} talk`.

// Beat 1: Normal. One page, 5–15 words.
// One full shot, one locked camera.
# street day shot=full
@{{star}} x=0.4 happy
@bird.a x=0.75 sleepy
// 5–15 words: the world as it is.
[Normal, and one obstacle.]

// Beat 2: The want, or the obstacle. One page, 5–15 words.
// A close on the one thing in the way.
# street day shot=close on=bird.a
@{{star}} x=0.3 worried
@bird.a x=0.75 sleepy
// 5–15 words: the want, or the thing in the way.
[The obstacle, up close.]

// Beat 3: Gag 1, setup, beat, payoff. One page, 0–10 words.
// Hold the beat before the payoff, and land the sound effect just after the action, not on it.
# street day shot=full
@{{star}} x=0.4 grin
@bird.a x=0.75 sleepy
@hold 0.6
@sfx pop
// 0–10 words: the first gag, and no explanation.
[Gag one.]

// Beat 4: Gag 2, bigger. One page, 0–10 words.
// The same framing as gag one, so the bigger failure reads as a variation.
# street day shot=full
@{{star}} x=0.4 cross
@bird.a x=0.75 sleepy
@sfx boing
// 0–10 words: the second gag, bigger, same frame.
[Gag two, bigger.]

// Beat 5: Gag 3, biggest, fails with a take. One page, 0–10 words.
// A double-take: a glance, a look away, then `notice` plays the second look before the take.
# street day shot=medium on={{star}}
@bird.a x=0.75 sleepy
@{{star}} x=0.4 see=bird.a
@pause 0.5
@{{star}} see=front
@pause 0.4
@{{star}} notice see=bird.a shocked
@pause 0.8
// 0–10 words: the third gag fails, and the take carries it.
[Gag three, the biggest.]

// Beat 6: The twist, a payoff with no anticipation. One page, 0–10 words.
// Cut into the surprise; easing in gives the audience time to see it coming.
# street day transition=cut shot=wide
@{{star}} x=0.4 happy see=bird.a
@bird.a x=0.75 happy
// 0–10 words: the twist, and nothing setting it up.
[The twist.]

// Beat 7: Button, iris out. One page, 3–8 words.
# street day transition=iris shot=full
@{{star}} x=0.4 grin
@bird.a x=0.75 happy
@sfx ding
// 3–8 words: the button, and out.
[The button.]
