+++
title = "{{title}}"
author = "{{author}}"
year = {{year}}
star = "{{star}}"
age = "3-5"
moral = "[One line: what the journey found that home already had.]"
blurb = "[The back-cover blurb: why the star leaves, what they find far away, and the way home.]"

[cover]
setting = "plain"
tint = "sky"
pose = "x=0.5 grin wave"

[video]
cast-voices = true

[flow]
name = "There and Back"
about = "A journey out through three places and a quick way home, changed by one thing found on the way."
good-for = "A search with a clear reason to leave, and a home the reader is glad to come back to."
pages = [13, 17]
words = [280, 480]
needs = "1"

[[flow.beats]]
name = "Home and the reason to go"
pages = 2
words = [25, 35]
note = "Show home first and say why it is not enough, in the star's own words."

[[flow.beats]]
name = "Setting out"
pages = 1
words = [15, 25]
note = "Send the star off to the right; every outward step in the book moves rightward."

[[flow.beats]]
name = "Place 1"
pages = 2
words = [25, 35]
note = "A new place needs a new light and a wide shot before anyone speaks; dissolve into it."

[[flow.beats]]
name = "Place 2"
pages = 2
words = [25, 35]
note = "Another place, further and stranger, with a different light again."

[[flow.beats]]
name = "Place 3, farthest and strangest"
pages = 2
words = [25, 35]
note = "The darkest light, and the star small in a wide shot, so the world feels bigger than they are."

[[flow.beats]]
name = "The turn: what was found, or homesick"
pages = 1
words = [15, 25]
note = "Go close: the journey turns here, on one face, not on a crowd."

[[flow.beats]]
name = "The way home, quick"
pages = 2
words = [15, 25]
note = "Homeward steps move leftward, and the pages are shorter than the outward ones."

[[flow.beats]]
name = "Home, changed"
pages = 2
words = [20, 30]
note = "The same home in a later light; the place has not changed, the star has."

[[flow.beats]]
name = "Button"
pages = 1
words = [10, 20]
note = "The opening framing again, with the souvenir on show."

+++

// There and Back: out through three places, then home changed by what was found.
// Replace every [bracketed] line with the story, keep the directions that fit, and run
// `fables proofs` to see it.

// Beat 1: Home and the reason to go. Two pages, 25–35 words.
# town day shot=wide on={{star}}
@{{star}} x=0.4 happy
// 25–35 words: home, and the star in it.
[Home, and the star in it.]

# town day shot=medium on={{star}}
@{{star}} x=0.45 talk
// 25–35 words: the reason to go, in the star's own words.
"[Why the star must go.]"

// Beat 2: Setting out. One page, 15–25 words.
// Outward travel moves rightward. The exit leaves an empty road in the still.
# town day shot=full
@{{star}} x=0.3 face=right walk
@{{star}} x=0.85 exit=right
// 15–25 words: the leaving.
[The star sets out, to the right.]

// Beat 3: Place 1. Two pages, 25–35 words.
// A new setting and light, and a wide shot before anyone speaks; dissolve between places.
# hilltop dawn shot=wide transition=dissolve
@{{star}} x=0.35 enter=right happy
// 25–35 words: the first new place, seen wide.
[The first new place.]

# hilltop dawn shot=close on={{star}}
@{{star}} x=0.45 grin
// 25–35 words: what the star finds there.
[What the star finds.]

// Beat 4: Place 2. Two pages, 25–35 words.
# park golden shot=wide transition=dissolve
@{{star}} x=0.35 enter=right happy
// 25–35 words: a different place in a different light.
[The second place.]

# park golden shot=full on={{star}}
@{{star}} x=0.5 reach-out grin
// 25–35 words: the next thing found, and what it costs to get it.
[What the star finds this time.]

// Beat 5: Place 3, farthest and strangest. Two pages, 25–35 words.
// The star small in a wide shot, under the worst light in the book.
# forest storm shot=wide on={{star}}
@{{star}} x=0.5 shocked
// 25–35 words: the furthest place, and how big it is.
[The furthest, strangest place.]

# forest storm shot=full on={{star}}
@{{star}} x=0.45 worried
// 25–35 words: the worst of it.
[The worst of it.]

// Beat 6: The turn. One page, 15–25 words.
// The star looks back the way they came, leftward, before the thought lands.
# forest storm shot=close on={{star}}
@{{star}} x=0.45 notice see=left sad think
// 15–25 words: the one face the journey turns on.
[What the star realises.]

// Beat 7: The way home, quick. Two pages, 15–25 words.
// All homeward travel moves leftward, and the pages are shorter than the way out.
# hilltop night shot=wide transition=push
@{{star}} x=0.6 face=left walk see=left
// 15–25 words: the first step home, leftward.
[The way home.]

# park night shot=wide transition=push
@{{star}} x=0.4 face=left walk see=left
// 15–25 words: the second step home, quicker.
[Quicker now.]

// Beat 8: Home, changed. Two pages, 20–30 words.
// A `see=` holds over the page turn until it is changed; `see=front` looks out at the reader.
# town moonlit shot=wide on={{star}}
@{{star}} x=0.4 enter=left proud see=front
// 20–30 words: home in a later light.
[Home again, in a later light.]

# town moonlit shot=medium on={{star}}
@{{star}} x=0.45 grin talk
// 20–30 words: what is different now, and what is not.
[What is different.]

// Beat 9: Button. One page, 10–20 words.
// The souvenir is in a hand: change `parcel` to the prop your story brings home.
# town moonlit shot=wide on={{star}}
@{{star}} x=0.4 proud hold holds=parcel see=mine
// 10–20 words: the opening framing, with the souvenir on show.
[The opening framing, and the souvenir.]
