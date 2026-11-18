+++
title = "{{title}}"
author = "{{author}}"
year = {{year}}
star = "{{star}}"
moral = "[One line: the habit the star chose for themselves.]"
blurb = "[The back-cover blurb: the habit everybody can see, the name it earns, and the day the star chooses it.]"

[cover]
setting = "plain"
tint = "sunflower"
pose = "x=0.5 grin wave"

[video]
cast-voices = true

[flow]
name = "The Habit Tale"
about = "A twelve-beat tale: a habit shows on the outside, earns a name, costs a friend, and is chosen for the first time."
good-for = "A character-led book where the change is a choice the star makes, not a lesson the narrator gives."
pages = [13, 17]
words = [280, 480]
needs = "2"

[[flow.beats]]
name = "The place and its rule"
pages = 1
words = [20, 30]
note = "Open with the one rule of this world, in the same words every time, so the book has a world before it has a plot."

[[flow.beats]]
name = "Meet the kid"
pages = 1
words = [20, 30]
note = "Name the star and show one lovable thing, before the habit does any damage."

[[flow.beats]]
name = "NOPE: the ask and the refusal, twice"
pages = 1
words = [25, 35]
note = "A grown-up asks; the star refuses, twice, in the same words: the refusal is the refrain."

[[flow.beats]]
name = "It shows"
pages = 1
words = [15, 25]
note = "The habit becomes visible where everyone can see it, and the star is the last to know."

[[flow.beats]]
name = "Rule of three: three consequences"
pages = 3
words = [25, 35]
note = "Three consequences, each bigger than the last, all funny and none cruel; give each its own place."

[[flow.beats]]
name = "The name"
pages = 1
words = [20, 30]
note = "The town gives the star their new name, and the star is shocked by it."

[[flow.beats]]
name = "The cost"
pages = 1
words = [20, 30]
note = "The cost is social, gentle and real: nobody wants to play."

[[flow.beats]]
name = "The choice"
pages = 1
words = [20, 30]
note = "Alone at night, the star decides to try the good habit once; the want must be theirs, not the narrator's."

[[flow.beats]]
name = "The magic"
pages = 1
words = [15, 25]
note = "The good habit drawn as the most beautiful page in the book, one effect and no explanation."

[[flow.beats]]
name = "The callback: the three consequences, reversed"
pages = 1
words = [20, 30]
note = "Reverse the three visible consequences on one page, in the same order, so the reader feels the change."

[[flow.beats]]
name = "The new name"
pages = 1
words = [15, 25]
note = "The town names the star again, now by what they chose to do."

[[flow.beats]]
name = "The button"
pages = 2
words = [15, 25]
note = "The ask returns, and the answer answers it; then the end card, and out."

+++

// The Habit Tale: a habit shows, earns a name, costs a friend, and is chosen.
// Replace every [bracketed] line with the story, keep the directions that fit, and run
// `fables proofs` to see it.

// Beat 1: The place and its rule. One page, 20–30 words.
# town day shot=wide
@{{star}} x=0.4 happy
@{{friend}} x=0.72 grin
// 20–30 words: the place, and the one rule of this world.
[The place, and its rule.]

// Beat 2: Meet the kid. One page, 20–30 words.
# bedroom day shot=medium on={{star}}
@{{star}} x=0.5 grin wave
// 20–30 words: the star's name, and one lovable thing.
[Meet the star.]

// Beat 3: NOPE: the ask and the refusal, twice. One page, 25–35 words.
# kitchen night shot=medium
@{{friend}} x=0.72 point cross talk
@{{star}} x=0.3 arms-crossed defiant
// 25–35 words: the ask, and the first refusal.
"[The ask]!" said [the grown-up].
@{{star}} talk
// 10–15 words: the refusal, in the star's own words.
"**NOPE**," said [the star].

// Beat 4: It shows. One page, 15–25 words.
# plain tint=moss shot=close on={{star}}
@{{star}} x=0.5 shocked
@scene stink=1
// 15–25 words: the habit becomes visible, and the star sees it first.
[It shows.]

// Beat 5: Rule of three, three consequences. Three pages, 25–35 words.
# garden day shot=full on={{star}}
@{{star}} x=0.3 wave happy talk
@scene wilt=1
// 25–35 words: the first consequence, the smallest.
[The first consequence.]

# park day shot=full on={{star}}
@{{star}} x=0.3 wave happy talk
@bird.a x=0.75 happy
@bird.a faint
// 25–35 words: the second consequence, bigger.
[The second consequence.]

# street day shot=full on={{star}}
@{{star}} x=0.3 wave happy talk
@dog x=0.75 happy
@dog faint
// 25–35 words: the third consequence, the biggest.
[The third consequence.]

// Beat 6: The name. One page, 20–30 words.
# town day shot=medium on={{star}}
@{{star}} x=0.4 shocked
@{{friend}} x=0.72 shocked
// 20–30 words: the new name, and the star's shock.
[The new name.]

// Beat 7: The cost. One page, 20–30 words.
# park day shot=medium on={{star}}
@{{star}} x=0.4 sad droop
// 20–30 words: the cost, gently.
[Nobody wants to play.]

// Beat 8: The choice. One page, 20–30 words.
# bedroom night shot=close on={{star}}
@{{star}} x=0.45 think worried talk
// 20–30 words: alone at night, the star decides.
"Just once," said [the star].

// Beat 9: The magic. One page, 15–25 words.
# plain tint=mint shot=medium on={{star}}
@{{star}} x=0.5 cheer dreamy
@sfx sparkle
// 15–25 words: the good habit, drawn as the best page in the book.
**WHOOSH!** [The magic.]

// Beat 10: The callback, the three consequences reversed. One page, 20–30 words.
# garden day shot=full on={{star}}
@{{star}} x=0.3 wave grin talk
@scene wilt=0
@bird.a x=0.75 wake
@dog x=0.85 wake
// 20–30 words: the same three consequences, put right.
[The callbacks.]

// Beat 11: The new name. One page, 15–25 words.
# town day shot=wide
@{{star}} x=0.4 cheer proud
@{{friend}} x=0.72 happy
// 15–25 words: the new name, now earned.
[The new name.]

// Beat 12: The button. Two pages, 15–25 words.
# kitchen night shot=medium
@{{friend}} x=0.72 point happy talk
@{{star}} x=0.3 hips grin talk
// 15–25 words: the ask again, and the answer that answers it.
"[The ask again]!" said [the grown-up].
@{{star}} talk
// 5–10 words: the answer.
"**ALREADY DONE!**"

// The end card.
# plain tint=sunflower shot=medium
@{{star}} x=0.5 wave grin
// 3–8 words: the end.
**The End**
