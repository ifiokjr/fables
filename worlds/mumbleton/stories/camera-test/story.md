+++
title = "Camera Test"
subtitle = "Six Pages of Film Grammar"
author = "Ifiok Jr."
star = "mo"
moral = "Every story reads better with a good shot list."

[cover]
setting = "park"
tint = "sky"
pose = "x=0.5 grin wave"

[video]
cast-voices = true
# The book's default page turn; pages override it one by one below.
transition = "push"
+++

// A working script for the camera. Two characters talk, somebody walks in,
// somebody is shocked, every transition is used once, and one page directs
// its own camera. Read it with `fables storyboard worlds/mumbleton/stories/camera-test/story.md`.

# park
@mo x=0.4 grin talk
On a windy hill in Mumbleton, Mo was practising his loudest voice.
"HELLO, WIND!" he shouted, and the wind shouted right back.

// Entered with a push (the book's default above).

# park transition=wipe
@mo x=0.35 talk
@dog x=0.7
"Hi," said Mo.
@dog talk
"Bark," said the dog.

// Two very short lines: the director shares a two-shot so the cutting never
// gets frantic. The classic paper wipe turns this page.

# street transition=cut
@mo x=0.5 talk
So Mo practised the loudest whistle in all of Mumbleton.
@camera shot=extreme-close on=mo move=push over=1.5
"Watch THIS," said Mo, and the wind held its breath.
@camera shake=0.4

// This page directs its own camera: a hard cut to an extreme close-up that
// pushes in, then a shake on the whistle. Pages with an @camera are shot
// exactly as written.

# town transition=dissolve
@mo x=0.3
Into the square came the sound of very small paws on the stones.
@dog enter=right
Everybody turned to look.
@dog talk
"Bark," said the dog, politely.

// The director widens to keep the entrance in frame, then cuts to the new
// arrival. The previous page dissolves into this one.

# garden transition=fade
@mo x=0.3
@dog x=0.7 talk
Barky barked a reply so loud that Mo's hair stood straight up.
@mo shocked
"WHOA," said Mo, when he found his voice again.

// The dog's line ends and Mo's shock earns a close reaction shot before he
// speaks. This page goes through the paper colour.

# garden transition=iris
@mo x=0.45 grin
@dog x=0.75 wag
From that day on, Mo and Barky practised together, and the wind carried
every word exactly where it belonged.

// The iris closes on the page's star and opens on the last page's, and the
// book ends the way every good shot list does.
