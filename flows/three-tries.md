+++
title = "{{title}}"
author = "{{author}}"
year = {{year}}
star = "{{star}}"
age = "3-5"
moral = "[One line: what the fourth try taught.]"
blurb = "[The back-cover blurb: the want, the three failures, the turn.]"

[cover]
setting = "plain"
tint = "sunflower"
pose = "x=0.5 grin wave"

[video]
cast-voices = true

[flow]
name = "Three Tries"
about = "A small problem met three times, each try failing bigger, until a fourth try uses what the failures taught."
good-for = "A want a child can hold and an ending you can already see, where the tries must earn it."
pages = [13, 17]
words = [300, 500]
needs = "1"

[[flow.beats]]
name = "Home and the want, said aloud"
pages = 2
words = [25, 35]
note = "Open wide so the reader knows the place, then let the star say the want in their own words."

[[flow.beats]]
name = "Try 1 fails"
pages = 2
words = [25, 35]
note = "Keep the same setting and stance on every try, so the reader can see exactly what changes."

[[flow.beats]]
name = "Try 2 fails bigger"
pages = 2
words = [25, 35]
note = "Make the second try bigger, and give the failure a beat to land before the next page."

[[flow.beats]]
name = "Try 3 fails biggest: the low point"
pages = 2
words = [20, 30]
note = "This is the darkest page: the star alone, in the worst light, with nothing left to try."

[[flow.beats]]
name = "Help or insight"
pages = 2
words = [25, 35]
note = "Something notices the star, or the star notices something; let the idea be seen before it is said."

[[flow.beats]]
name = "Try 4 succeeds, using what failed"
pages = 3
words = [25, 35]
note = "Push in on the attempt and pull out wide on the success, so the win has room."

[[flow.beats]]
name = "New normal and button"
pages = 2
words = [15, 25]
note = "Return to the opening staging with the star changed, and end on the refrain, altered."

+++

// Three Tries: three tries that fail bigger, then a fourth that uses what they taught.
// Replace every [bracketed] line with the story, keep the directions that fit, and run
// `fables proofs` to see it.
// Every "quoted" line needs its speaker on the line above it: `@{{star}} talk`.

// Beat 1: Home and the want, said aloud. Two pages, 25–35 words a page.
// Wide first, so the reader knows the place before anyone speaks.
# garden day shot=wide on={{star}}
@{{star}} x=0.4 happy
// 25–35 words: where we are, and what the star is doing.
[The place, and the star in it.]

// A medium on the star, talking, so the want is theirs.
# garden day shot=medium on={{star}}
@{{star}} x=0.45 talk
// 25–35 words: the want, said in the star's own words.
"[The star says what they want.]"

// Beat 2: Try 1 fails. Two pages, 25–35 words a page.
// The same setting and stance on every try, so the variation reads.
# garden day shot=full on={{star}}
@{{star}} x=0.45 reach-out
// 25–35 words: the first try, in one clear action.
[The first try.]

// The close reaction is half the page.
# garden day shot=close on={{star}}
@{{star}} x=0.45 sad droop
// 25–35 words: it fails, and how the star takes it.
[It fails.]

// Beat 3: Try 2 fails bigger. Two pages, 25–35 words a page.
// Give the failure a beat to land before the next page.
# garden day shot=full on={{star}}
@{{star}} x=0.45 think
// 25–35 words: the second try, bigger than the first.
[The second try, bigger.]

# garden day shot=close on={{star}}
@{{star}} x=0.45 shocked
// 25–35 words: the failure is bigger too.
[It fails bigger.]

// Beat 4: Try 3 fails biggest, the low point. Two pages, 20–30 words a page.
// The darkest light in the book, and the star alone in a full shot.
# garden overcast shot=full on={{star}}
@{{star}} x=0.45 reach-out
// 20–30 words: the biggest try, with everything behind it.
[The third try, the biggest.]

# garden storm shot=full on={{star}}
@{{star}} x=0.5 sad droop
// 20–30 words: the low point. Say less than you want to.
[Nothing works. The star is alone.]

// Beat 5: Help or insight. Two pages, 25–35 words a page.
// `notice` plays the look before the thought; aim `see=` at the thing that is noticed.
# garden dawn shot=close on={{star}}
@flower.1 x=0.8 happy
@{{star}} x=0.45 notice see=flower.1 worried think
// 25–35 words: something is noticed, and the star looks at it.
[Something is noticed.]

// A `see=` holds over the page turn until it is changed; `see=front` looks out at the reader.
# garden dawn shot=medium on={{star}}
@{{star}} x=0.45 think talk see=front
// 25–35 words: the idea, in the star's own words, before the plan.
[The idea the failures taught.]

// Beat 6: Try 4 succeeds, using what failed. Three pages, 25–35 words a page.
// Push in on the attempt, then give the success room.
# garden golden shot=medium on={{star}}
@{{star}} x=0.4 reach-out
@camera move=push over=2
// 25–35 words: the fourth try, using what the first three taught.
[The fourth try, using what failed.]

# garden golden shot=medium on={{star}}
@{{star}} x=0.45 cheer happy
// 25–35 words: it works, and the star feels it.
[It works!]

# garden golden shot=wide on={{star}}
@{{star}} x=0.45 proud
@camera move=pull over=2
// 25–35 words: the success, seen wide, with room around it.
[The success, seen whole.]

// Beat 7: New normal and button. Two pages, 15–25 words a page.
// The opening staging, the star now proud, and the opening line changed.
# garden golden shot=wide on={{star}}
@{{star}} x=0.4 proud
// 15–25 words: the new normal, in the framing the book opened with.
[The new normal.]

# garden golden shot=medium on={{star}}
@{{star}} x=0.45 proud talk
// 15–25 words: the button: the refrain, changed.
"[The opening line again, changed.]"
