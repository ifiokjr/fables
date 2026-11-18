+++
title = "Zia and the Pigeon"
author = "Ifiok Jr."
star = "zia"
age = "3-5"
flow = "gag-short"
moral = "Somebody else's bell is always louder."
blurb = "A pigeon will not move off the tram line, whatever Zia rings at it. The tram has its own idea."

[cover]
setting = "square"
tint = "sunflower"
pose = "x=0.5 grin wave"

[video]
cast-voices = true
+++

// A Gag Short: one want, three gags that fail bigger, and the tram that solves it without
// a word. The band is the square's critters, because the report's brass band has no cast
// here yet.

# square day shot=full
@zia x=0.4 happy
@bird.a x=0.78 sleepy
A pigeon sat on the tram line, right in the middle of the square.

# square day shot=close on=bird.a
@zia x=0.3 worried
@bird.a x=0.78 sleepy
Zia rang her bicycle bell. The pigeon did not move.

# square day shot=full
@zia x=0.4 grin
@bird.a x=0.78 sleepy
@hold 0.6
@sfx pop
Ding! Nothing.
@pause 0.5

# square day shot=full
@zia x=0.4 cross
@bird.a x=0.78 sleepy
@sfx boing
Ding ding! Still nothing.
@pause 0.5

# square day shot=medium on=zia
@zia x=0.44 shocked
@bird.b x=0.12 cheer
@dog x=0.28 wag
@bird.a x=0.78 sleepy
@pause 0.8
Zia fetched the whole brass band. The pigeon yawned.

# square day transition=cut shot=wide
@zia x=0.44 happy
@bird.b x=0.12 cheer
@dog x=0.28 wag
@bird.a x=0.78 cheer
The tram came without a sound and stopped beside them. The pigeon hopped aboard.
@pause 0.5

# square day transition=iris shot=close on=zia
@zia x=0.4 grin
@bird.a x=0.78 happy
@sfx ding
The tram dinged its own bell, and left.
