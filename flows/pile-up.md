+++
title = "{{title}}"
author = "{{author}}"
year = {{year}}
star = "{{star}}"
age = "3-5"
moral = "[One line: what the hush after the noise was for.]"
blurb = "[The back-cover blurb: one small sound, then the whole street joining in.]"

[cover]
setting = "street"
tint = "sunflower"
pose = "x=0.4 grin wave"

[video]
cast-voices = true

[flow]
name = "The Pile-Up"
about = "One small sound gathers a newcomer a page until the whole street is in it, and the noise brings the house down."
good-for = "A book built to be read again, where a child can join the chain and shout the last line."
pages = [9, 13]
words = [250, 450]
needs = "many"

[[flow.beats]]
name = "The quiet start, one actor"
pages = 1
words = [15, 20]
note = "One figure left of centre, facing right, in a wide shot: the space on the right is the invitation."

[[flow.beats]]
name = "First link"
pages = 1
words = [15, 25]
note = "The second character enters from the right and joins the sound; keep the camera locked wide."

[[flow.beats]]
name = "Links 2 to 6, each repeating the whole chain"
pages = 6
words = [25, 45]
note = "One entrance a page, always from the right, and repeat the whole chain before the new link so the join-in is easy."

[[flow.beats]]
name = "The topple"
pages = 1
words = [3, 8]
note = "Three to eight words and every effect you have: this is the page the whole book was building."

[[flow.beats]]
name = "Aftermath, everyone in a heap"
pages = 1
words = [10, 20]
note = "Go quiet after the bang, hold the heap for a beat, then let everybody laugh."

[[flow.beats]]
name = "Button that starts the chain again"
pages = 1
words = [10, 15]
note = "The opening framing, with one new arrival at the edge: the chain is ready to start again."

+++

// The Pile-Up: one sound gathers a newcomer a page, then the whole street is in it.
// Replace every [bracketed] line with the story, keep the directions that fit, and run
// `fables proofs` to see it.

// Beat 1: The quiet start, one actor. One page, 15–20 words.
// One figure left of centre facing right; the empty right half is where the story will go.
# street day shot=wide
@{{star}} x=0.16 face=right happy
// 15–20 words: one actor, one small sound, and a lot of empty street.
[One small sound in an empty street.]

// Beat 2: First link. One page, 15–25 words.
// The newcomer comes in from the right; the camera stays wide and locked.
# street day shot=wide
@{{star}} x=0.10 happy
@{{friend}} x=0.34 enter=right grin
// 15–25 words: the first newcomer joins in, and repeats the first sound.
[The first newcomer joins in.]

// Beat 3: Links 2 to 6, each repeating the whole chain. Six pages, 25–45 words.
// One entrance a page, always from the right. Restate the whole chain on every page, in
// the directions and in the words, so a child can join in. Keep the line spread across the
// page: the star and friend stand back, and the small critters fill the gaps in front.
# street day shot=wide
@{{star}} x=0.10 grin
@{{friend}} x=0.34 grin
@camera move=push over=3
// 25–35 words: the chain so far, twice as loud.
[The two of them, louder.]

# street day shot=wide
@{{star}} x=0.10 grin
@{{friend}} x=0.34 grin
@bird.a x=0.22 y=0.62 z=3 size=0.85 enter=right happy
@camera move=push over=3
// 25–40 words: the next link enters from the right and the chain repeats.
[The next link enters from the right.]

# street day shot=wide
@{{star}} x=0.10 grin
@{{friend}} x=0.34 grin
@bird.a x=0.22 y=0.62 z=3 size=0.85 happy
@dog x=0.60 enter=right wag
@camera move=push over=3
// 30–45 words: the dog joins, and the whole chain repeats again.
[The dog joins in.]

# street day shot=wide
@{{star}} x=0.10 grin size=0.9
@{{friend}} x=0.34 grin size=0.9
@bird.a x=0.22 y=0.62 z=3 size=0.8 happy
@dog x=0.60 wag size=0.9
@bird.b x=0.47 y=0.58 z=3 size=0.8 enter=right happy
@camera move=push over=3
// 30–45 words: one more from the right; the chain is getting long now.
[One more from the right.]

# street day shot=wide
@{{star}} x=0.10 grin size=0.9
@{{friend}} x=0.34 grin size=0.9
@bird.a x=0.22 y=0.62 z=3 size=0.8 happy
@dog x=0.60 wag size=0.9
@bird.b x=0.47 y=0.58 z=3 size=0.8 happy
@worm x=0.73 y=0.66 z=4 size=0.7 enter=right happy
@camera move=push over=3
// 30–45 words: the smallest link gets the biggest entrance.
[The smallest link joins.]

# street day shot=wide
@{{star}} x=0.10 grin size=0.9
@{{friend}} x=0.34 grin size=0.9
@bird.a x=0.22 y=0.62 z=3 size=0.8 happy
@dog x=0.60 wag size=0.9
@bird.b x=0.47 y=0.58 z=3 size=0.8 happy
@worm x=0.73 y=0.66 z=4 size=0.7 happy
@flower.1 x=0.86 enter=right happy size=0.95
@camera move=push over=3
// 30–45 words: the last link, and the whole street singing at once.
[The whole street is in it.]

// Beat 4: The topple. One page, 3–8 words.
// The wide shows the line come down; the hold and the stopped music are the silence before
// the bang, and the close after the shout catches the star's face.
# street storm impact=1 shot=wide
@camera shake=0.6
@{{star}} x=0.10 shocked size=0.9
@{{friend}} x=0.34 shocked size=0.9
@bird.a x=0.22 y=0.62 z=3 size=0.8 shocked
@dog x=0.60 shocked size=0.9
@bird.b x=0.47 y=0.58 z=3 size=0.8 shocked
@worm x=0.73 y=0.66 z=4 size=0.7 shocked
@flower.1 x=0.86 shocked size=0.95
@hold 0.8
@music none
@sfx bonk
// 3–8 words: the bang, and nothing else.
**SHOUT!**
@camera shot=close on={{star}}

// Beat 5: Aftermath, everyone in a heap. One page, 10–20 words.
// Go quiet after the bang, hold the heap for a beat, then let everybody laugh.
# street overcast shot=full
@{{star}} x=0.30 laugh
@{{friend}} x=0.48 laugh
@bird.a x=0.16 y=0.62 z=3 size=0.8 laugh
@dog x=0.68 wag
@flower.1 x=0.88 sleepy
// 10–20 words: the hush, the heap, then the laugh.
[Everyone in a heap, laughing.]

// Beat 6: Button that starts the chain again. One page, 10–15 words.
// The opening framing, with one new arrival at the edge.
# street day shot=wide
@{{star}} x=0.10 proud
@{{friend}} x=0.34 happy
@bird.c x=0.86 y=0.62 z=3 size=0.85 enter=right happy
// 10–15 words: the quiet again, and one new arrival at the edge.
[Quiet again. Then, at the edge, somebody new.]
