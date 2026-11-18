+++
title = "{{title}}"
author = "{{author}}"
year = {{year}}
star = "{{star}}"
moral = "[One line: what two opposite ways made together.]"
blurb = "[The back-cover blurb: two characters who cannot agree, and the day they have to.]"

[cover]
setting = "plain"
tint = "sky"
pose = "x=0.5 grin wave"

[video]
cast-voices = true

[flow]
name = "The Odd Couple"
about = "Two characters built as opposites meet, clash three times, miss each other, and find that each needs the other's way."
good-for = "A funny two-hander where the joke is character, and the ending is a swap rather than a lesson."
pages = [13, 17]
words = [280, 480]
needs = "2"

[[flow.beats]]
name = "Meet A in their world"
pages = 2
words = [25, 35]
note = "Give A a shape, a size and a tempo of their own before B exists, so the contrast has something to push against."

[[flow.beats]]
name = "Meet B, the opposite"
pages = 2
words = [25, 35]
note = "B is the opposite in every dial: shape, size, speed, volume. Do not balance them yet."

[[flow.beats]]
name = "Forced together"
pages = 1
words = [20, 30]
note = "A always screen-left and B screen-right from here; the line between them is the story."

[[flow.beats]]
name = "Clash ×3, escalating"
pages = 3
words = [25, 35]
note = "The same gag pattern three times, bigger each time, shot and reverse shot across the line."

[[flow.beats]]
name = "Apart, missing each other"
pages = 2
words = [20, 30]
note = "Mirror the same composition, in two different lights: the absence should look like the argument."

[[flow.beats]]
name = "A needs B's way, and B needs A's"
pages = 2
words = [25, 35]
note = "A medium two-shot where each reaches for the other's habit; let them look at each other."

[[flow.beats]]
name = "Together, both changed a little"
pages = 2
words = [20, 30]
note = "Wide and side by side, each doing a small version of the other's way."

[[flow.beats]]
name = "Button: a habit swapped"
pages = 1
words = [10, 20]
note = "The first clash's pose again, now shared: the same picture says how far they came."

+++

// The Odd Couple: two opposites, three clashes, and a swap.
// Replace every [bracketed] line with the story, keep the directions that fit, and run
// `fables proofs` to see it.

// Beat 1: Meet A in their world. Two pages, 25–35 words.
# shop indoor shot=medium on={{star}}
@{{star}} x=0.35 proud
// 25–35 words: A in their own place, doing the thing A always does.
[Meet A.]

# shop indoor shot=full on={{star}}
@{{star}} x=0.4 hips happy
// 25–35 words: A's shape and tempo, in one action.
[A's tempo.]

// Beat 2: Meet B, the opposite. Two pages, 25–35 words.
# park day shot=medium on={{friend}}
@{{friend}} x=0.65 grin
// 25–35 words: B in their own place, doing the opposite.
[Meet B.]

# park day shot=full on={{friend}}
@{{friend}} x=0.6 cheer happy
// 25–35 words: B's opposite shape and tempo, in one action.
[B's tempo.]

// Beat 3: Forced together. One page, 20–30 words.
// From here A is screen-left and B screen-right; the line between them is the story.
# plain day shot=wide
@{{star}} x=0.28 cross
@{{friend}} x=0.72 worried
// 20–30 words: the two of them in one place, and neither happy about it.
[Forced together.]

// Beat 4: Clash ×3, escalating. Three pages, 25–35 words.
// The same gag pattern three times, bigger each time; shot and reverse shot across the line.
# plain day shot=medium on={{star}}
@{{star}} x=0.28 cross talk
@{{friend}} x=0.72 laugh
@camera shot=medium on={{friend}}
// 25–35 words: the first clash, small.
[Clash one.]

# plain day shot=medium on={{friend}}
@{{friend}} x=0.72 laugh talk
@{{star}} x=0.28 shocked
@camera shot=medium on={{star}}
// 25–35 words: the second clash, bigger, same pattern.
[Clash two.]

# plain day shot=full
@{{star}} x=0.28 shocked talk
@{{friend}} x=0.72 cross talk
@camera shake=0.2
// 25–35 words: the third clash, the biggest, and both of them lose.
[Clash three.]

// Beat 5: Apart, missing each other. Two pages, 20–30 words.
// Mirror the same composition, in two lights.
# park dusk shot=wide
@{{star}} x=0.72 sad
@{{friend}} x=0.28 sad
// 20–30 words: apart, in the same composition, mirrored.
[Apart.]

# park moonlit shot=wide
@{{star}} x=0.72 sad droop
@{{friend}} x=0.28 sad droop
// 20–30 words: still apart, later, and missing the other.
[Later, and missing each other.]

// Beat 6: A needs B's way, and B needs A's. Two pages, 25–35 words.
// A medium two-shot where each reaches for the other's habit.
# kitchen indoor shot=medium on={{star}},{{friend}}
@{{star}} x=0.3 worried talk
@{{friend}} x=0.7 worried
// 25–35 words: A needs B's way, and says so.
[A needs B's way.]

# kitchen indoor shot=medium on={{star}},{{friend}}
@{{friend}} x=0.7 worried talk
@{{star}} x=0.3 worried
// 25–35 words: B needs A's way, and says so.
[B needs A's way.]

// Beat 7: Together, both changed a little. Two pages, 20–30 words.
# garden golden shot=wide
@{{star}} x=0.3 grin
@{{friend}} x=0.7 grin
// 20–30 words: together, side by side, each doing a small version of the other's way.
[Together.]

# garden golden shot=wide
@{{star}} x=0.3 laugh
@{{friend}} x=0.7 laugh
// 20–30 words: both changed a little, and neither keeping score.
[Both changed a little.]

// Beat 8: Button: a habit swapped. One page, 10–20 words.
// The first clash's pose again, now shared; a close at the end says the change is theirs.
# plain day shot=medium on={{star}}
@{{star}} x=0.28 hips laugh
@{{friend}} x=0.72 hips laugh
// 10–20 words: the first clash's pose again, now shared.
[The first clash's pose, shared.]
@camera shot=close on={{star}}
