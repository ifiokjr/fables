+++
title = "Pim and the Parcel"
author = "Ifiok Jr."
year = 2026
star = "pim"
flow = "gag-short"
age = "3-5"
moral = "The thing in your way might be the help you need."
blurb = "Pim has a parcel for Dot, who is just across the square. In the middle of the square sleeps a very large dog."

[cover]
setting = "park"
tint = "sunflower"
pose = "x=0.5 grin holds=parcel hold"

[video]
cast-voices = true

+++

// A Gag Short on a paper stage: one want, three gags that fail bigger, and a twist.

# park day shot=wide
@pim x=0.2 happy hold holds=parcel face=right
@dog.a x=0.52 sleepy
@dot x=0.84 happy wave face=left
Pim had a parcel for Dot. Dot was just across the square.

# park day shot=close on=dog.a
@pim x=0.2 worried hold holds=parcel face=right
@dog.a x=0.52 sleepy
But in the middle of the square slept a very large dog.

# park day shot=full on=pim,dog.a
@pim x=0.3 worried tiptoe hold holds=parcel face=right
@dog.a x=0.56 sleepy
@hold 0.6
@sfx thud
Pim tiptoed. The dog's tail went thump.

# park day shot=full on=pim,dog.a
@pim x=0.3 cross hop hold holds=parcel face=right
@dog.a x=0.56 sleepy
@sfx boing
Pim hopped. The dog's ear went up.

# park day shot=medium on=pim
@pim x=0.36 shocked run cheer holds=nothing face=right
@dog.a x=0.6 shocked
@pause 0.8
Pim ran, and tripped, and the parcel flew!

# park day transition=cut shot=wide
@pim x=0.2 shocked face=right
@dog.a x=0.6 happy wag
@dot x=0.84 laugh hold holds=parcel face=left
The dog caught it, and took it to Dot.

# park day transition=iris shot=full on=dot,dog.a
@pim x=0.3 grin cheer face=right
@dog.a x=0.6 happy wag
@dot x=0.84 proud hold holds=parcel face=left
@dot talk
@sfx ding
"Special delivery," said Dot.
