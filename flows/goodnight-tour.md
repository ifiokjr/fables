+++
title = "{{title}}"
author = "{{author}}"
year = {{year}}
star = "{{star}}"
age = "3-5"
moral = "[One line: the quiet thing the star kept for last.]"
blurb = "[The back-cover blurb: a bedtime walk that says goodnight to the whole world, one place at a time.]"

[cover]
setting = "bedroom"
tint = "night"
pose = "x=0.5 sleepy wave"

[video]
cast-voices = true

[flow]
name = "The Goodnight Tour"
about = "A small evening worry settles, then the star says goodnight to one place and creature a page until only the last line is left."
good-for = "A bedtime book where the falling light and the falling word count do the work."
pages = [11, 15]
words = [200, 380]
needs = "1"

[[flow.beats]]
name = "Evening, and a small problem"
pages = 2
words = [30, 35]
note = "Golden light and a calm camera; the problem is small enough to be solved by going to bed."

[[flow.beats]]
name = "The problem settles gently"
pages = 2
words = [25, 30]
note = "Dusk, and no mood bigger than worried: nothing in a bedtime book should spike."

[[flow.beats]]
name = "Goodnight to each place and creature"
pages = 7
words = [10, 25]
note = "One setting a page, each quieter than the last, moonlit, dissolved; only slow pushes, never a cut."

[[flow.beats]]
name = "Home and into bed"
pages = 1
words = [10, 15]
note = "Candle light and a medium: the smallest, safest frame in the book."

[[flow.beats]]
name = "The whisper line"
pages = 1
words = [3, 8]
note = "Three to eight words, no more; the music stops and the page fades."

+++

// The Goodnight Tour: a bedtime walk that says goodnight to the world, one place a page.
// Replace every [bracketed] line with the story, keep the directions that fit, and run
// `fables proofs` to see it.
// Every "quoted" line needs its speaker on the line above it: `@{{star}} talk`.

// Beat 1: Evening, and a small problem. Two pages, 30–35 words.
// Golden light, and a calm camera; golden already picks the calm cue.
# town golden shot=wide on={{star}}
@{{star}} x=0.4 worried
// 30–35 words: the evening, and the small problem.
[Evening, and the small problem.]

# town golden shot=medium on={{star}}
@{{star}} x=0.45 worried talk
// 30–35 words: the problem, said quietly, once.
[The problem, said once.]

// Beat 2: The problem settles gently. Two pages, 25–30 words.
// Dusk, and no mood bigger than worried.
# street dusk shot=medium on={{star}}
@{{star}} x=0.45 worried
// 25–30 words: the problem settles a little.
[It settles a little.]

# street dusk shot=close on={{star}}
@{{star}} x=0.45 sleepy
// 25–30 words: and a little more.
[And a little more.]

// Beat 3: Goodnight to each place and creature. Seven pages, 25 words down to 10.
// One setting a page, quieter each time, moonlit, dissolved; only slow pushes, never a cut.
# garden moonlit transition=dissolve shot=medium on={{star}}
@{{star}} x=0.45 sleepy
@bird.a x=0.78 sleepy
@camera move=push over=3
// 22–25 words: goodnight to the first place and its creature.
[Goodnight, garden.]

# park moonlit transition=dissolve shot=medium on={{star}}
@{{star}} x=0.4 sleepy
@dog x=0.75 sleepy
@camera move=push over=3
// 19–22 words: goodnight to the second place and its creature.
[Goodnight, park.]

# street moonlit transition=dissolve shot=medium on={{star}}
@{{star}} x=0.45 sleepy
@camera move=push over=3
// 17–19 words: goodnight to the street.
[Goodnight, street.]

# forest moonlit transition=dissolve shot=medium on={{star}}
@{{star}} x=0.4 sleepy
@tree x=0.78 sleepy
@camera move=push over=3
// 15–17 words: goodnight to the trees.
[Goodnight, forest.]

# hilltop moonlit transition=dissolve shot=wide
@{{star}} x=0.45 sleepy
@camera move=push over=3
// 13–15 words: goodnight to the hill and the sky over it.
[Goodnight, hilltop.]

# beach moonlit transition=dissolve shot=wide
@{{star}} x=0.45 sleepy
@bird.b x=0.78 sleepy
@camera move=push over=3
// 11–13 words: goodnight to the sea.
[Goodnight, sea.]

# shop moonlit transition=dissolve shot=medium on={{star}}
@{{star}} x=0.45 sleepy
@camera move=push over=3
// 10–11 words: the last goodnight, the shortest.
[Goodnight, last place.]

// Beat 4: Home and into bed. One page, 10–15 words.
// Candle light picks the sleepy music box, and the smallest, safest frame in the book.
# bedroom candle shot=medium on={{star}}
@{{star}} x=0.45 sleepy
// 10–15 words: home, and into bed.
[Home, and into bed.]

// Beat 5: The whisper line. One page, 3–8 words.
// Three to eight words; the music stops and the page fades.
# bedroom candle shot=close on={{star}} text=bottom transition=fade
@{{star}} x=0.45 dreamy
@music none
// 3–8 words: the whisper.
[The whisper.]
