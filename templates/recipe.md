+++
title = "{{title}}"
subtitle = "A Tale from Kerfuffle"
author = "{{author}}"
series = "Tales from Kerfuffle"
year = {{year}}
star = "{{star}}"
dedication = "For everyone who has ever said NOPE."
moral = "Write the one-line moral here. It is read out at the end of the video."
blurb = "In the town of Kerfuffle, whatever you do shows on the outside. Write the back-cover blurb here."

[cover]
setting = "plain"
tint = "sunflower"
pose = "x=0.5 grin wave"

[voice]
name = "Daniel"
rate = 158

[video]
cast-voices = true
+++

// The Kerfuffle recipe: twelve beats, each a page or two. Replace every [bracketed] line with
// the story, keep the directions that fit, and run `fables proofs` to see it. The series bible
// (world/kerfuffle.md) explains each beat.

// 1. The town: same opening line in every book, with cameos showing their habits.
# town
@bogey x=0.26 pick grin
@grubby x=0.74 cheer happy
In the town of Kerfuffle, whatever you do shows on the outside.

// 2. The kid: meet them by their birth name, being lovable.
# bedroom
@{{star}} x=0.55 grin wave
This is [birth name].
~ [Something lovable about them.]

// 3. NOPE: a grown-up asks, the kid refuses. Twice.
# bathroom night
@mum x=0.72 point cross talk
@{{star}} x=0.3 arms-crossed defiant
"[The ask]!" said Mum.
@{{star}} talk
"**NOPE**," said [birth name].

// 4. It shows: the habit becomes visible.
# plain tint=moss zoom=1.5 look=0.5,0.6
@{{star}} x=0.5 shocked
[The first visible sign.]

// 5. Rule of three: three consequences, each bigger than the last.
# garden
@{{star}} x=0.28 wave happy talk
"[Hello to something]!"
@scene wilt=1
[Consequence one.]

# park
@{{star}} x=0.3 wave happy talk
@bird.a x=0.7 y=0.3 fly
"[Hello to someone]!"
@bird.a faint
[Consequence two.]

# street
@{{star}} x=0.28 wave happy talk
@parcel x=0.7 hold holds=parcel happy
"[Hello to a grown-up]!"
@parcel faint holds=nothing
[Consequence three, the biggest.]

// 6. The name: the town gives them their McName.
# town
@{{star}} x=0.5 shocked
@bogey x=0.2 shocked
@grubby x=0.8 shocked
From that day on, [birth name] was **{{title}}**.

// 7. The cost: friends won't play. Gentle, a little funny, never mean.
# park
@{{star}} x=0.4 sad droop
[Nobody wanted to play.]

// 8. The choice: alone, the kid decides to try. Just once.
# bedroom night
@{{star}} x=0.45 think worried talk
"Fine," said [their McName]. "Just *once*."

// 9. The magic: the good habit, drawn as the most beautiful moment in the book.
# plain tint=mint
@{{star}} x=0.5 cheer dreamy
@sfx sparkle
**WHOOSH!** [The magic moment.]

// 10. Callback: the same three consequences, reversed.
# garden wilt=1
@{{star}} x=0.28 wave grin talk
"[Hello again]!"
@scene wilt=0
[Everything blooms.]

// 11. The new name.
# town
@{{star}} x=0.5 cheer proud
They called them **[their new name]**.

// 12. The button: a last joke that answers the NOPE.
# bathroom night
@mum x=0.72 point happy talk
@{{star}} x=0.3 hips grin
"[The ask]!" said Mum.
@{{star}} talk cheer
"**ALREADY DONE!**"

# plain tint=sunflower
@{{star}} x=0.5 wave grin
**The End**
