+++
title = "{{title}}"
author = "{{author}}"
year = {{year}}
star = "{{star}}"
age = "3-5"
moral = "[One line: what the star was sure of, and what was true.]"
blurb = "[The back-cover blurb: a half-heard sentence, a helpful star, and the truth in the background.]"

[cover]
setting = "garden"
tint = "moss"
pose = "x=0.5 grin wave"

[video]
cast-voices = true

[flow]
name = "Wrong End of the Stick"
about = "The reader sees the truth, the star half-hears it and misreads it, and three helpful acts make the mistake worse before the reveal."
good-for = "A joke the pictures tell before the words do, where being wrong is funny and easily put right."
pages = [11, 13]
words = [260, 450]
needs = "2"

[[flow.beats]]
name = "The reader sees the truth"
pages = 2
words = [20, 30]
note = "Draw the truth plainly where the reader sees it and the star does not; the words must never mention it."

[[flow.beats]]
name = "The star overhears and misreads"
pages = 1
words = [20, 30]
note = "A close on the star, with the truth behind them; one half-heard sentence is enough."

[[flow.beats]]
name = "Acts on the mistake ×3, escalating"
pages = 3
words = [25, 35]
note = "Keep the truth in the background of every page, and make each helpful act bigger than the last."

[[flow.beats]]
name = "Others are puzzled, and the mistake deepens"
pages = 2
words = [25, 35]
note = "Reaction closes: the other character's face, then the star's, so the gap between them is visible."

[[flow.beats]]
name = "The reveal on the page turn"
pages = 1
words = [10, 20]
note = "Set it up at the bottom of the text card, then reveal it with an extreme close and a pull."

[[flow.beats]]
name = "Laughing and putting it right"
pages = 2
words = [20, 30]
note = "Wide and laughing: the mistake costs nothing, and the putting right is a small, warm action."

[[flow.beats]]
name = "Button: someone else misreads"
pages = 1
words = [10, 15]
note = "A new close, a new misreading, iris out: the joke belongs to the whole cast now."

+++

// Wrong End of the Stick: the reader sees the truth, the star does not, and the mistake
// gets bigger before it gets funny.
// Replace every [bracketed] line with the story, keep the directions that fit, and run
// `fables proofs` to see it.

// Beat 1: The reader sees the truth. Two pages, 20–30 words.
// The truth is drawn where the reader sees it and the star does not: keep it on the
// star's far side, and keep the words away from it.
# garden day shot=wide
@{{star}} x=0.25 face=left happy
@{{friend}} x=0.75 happy
@bird.a x=0.92 happy
// 20–30 words: the ordinary morning, and the truth in the background.
[The ordinary morning, and the truth in the background.]

# garden day shot=close on={{friend}}
@{{friend}} x=0.7 happy
@bird.a x=0.9 happy
// 20–30 words: the truth up close, for the reader only.
[The truth, up close.]

// Beat 2: The star overhears and misreads. One page, 20–30 words.
// The star looks away from the truth while the half-sentence goes by.
# garden day shot=close on={{star}}
@{{star}} x=0.3 worried see=away
@{{friend}} x=0.75 talk
// 20–30 words: the half-heard sentence, and the star's wrong reading of it.
"[The half-heard sentence.]"

// Beat 3: Acts on the mistake ×3, escalating. Three pages, 25–35 words.
// Keep the truth in the background of every page.
// A `see=` holds over the page turn until it is changed; `see=front` looks out at the reader.
# kitchen indoor shot=full
@{{star}} x=0.3 proud see=front
@{{friend}} x=0.8 happy
// 25–35 words: the first helpful act, and the truth still going on behind it.
[First act.]

# bedroom day shot=medium on={{star}}
@{{star}} x=0.35 proud
@{{friend}} x=0.8 happy
// 25–35 words: the second act, bigger.
[Second act.]

# garden day shot=full
@{{star}} x=0.3 cheer talk
@{{friend}} x=0.8 happy
// 25–35 words: the third act, the biggest, and the star singing about it.
[Third act.]

// Beat 4: Others are puzzled, and the mistake deepens. Two pages, 25–35 words.
// A double-take: a glance, a look away, then `notice` plays the second look before the take.
# garden day shot=close on={{friend}}
@{{star}} x=0.3 proud
@{{friend}} x=0.7 see={{star}}
@pause 0.5
@{{friend}} see=front
@pause 0.4
@{{friend}} notice see={{star}} shocked
@pause 0.8
// 25–35 words: the other character's face, and no words to explain it.
[Puzzled.]

# garden day text=bottom shot=close on={{star}}
@{{star}} x=0.3 worried see={{friend}}
@{{friend}} x=0.7 worried see={{star}}
// 25–35 words: the star sees the puzzle and misreads it too; the text card sits low, ready
// for the page turn.
[The mistake deepens.]

// Beat 5: The reveal on the page turn. One page, 10–20 words.
// The setup above ends with its text card at the bottom; the reveal holds silently for two
// seconds before a word is said.
# garden dusk hold=2 shot=extreme-close on={{star}}
@{{star}} x=0.4 shocked
@camera move=pull over=2
@dog x=0.85 enter=right wag
@{{friend}} x=0.7 laugh
// 10–20 words: the reveal, and the smallest line that lets it land.
[The truth arrives at last.]

// Beat 6: Laughing and putting it right. Two pages, 20–30 words.
# garden dusk shot=wide
@{{star}} x=0.3 laugh
@{{friend}} x=0.7 laugh
@dog x=0.85 wag
// 20–30 words: everybody laughing, and the putting right.
[Laughing.]

# garden moonlit shot=wide
@{{star}} x=0.3 grin
@{{friend}} x=0.7 grin
@dog x=0.85 sleepy
// 20–30 words: the small warm action that puts it right.
[Put right.]

// Beat 7: Button: someone else misreads. One page, 10–15 words.
// A new close, a new misreading, iris out.
# garden moonlit shot=close on=dog transition=iris
@dog x=0.85 shocked
@{{star}} x=0.3 laugh
// 10–15 words: the new misreading, and out.
[Somebody else misreads it.]
