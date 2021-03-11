# Amoeba Roguelike

7DRL 2021 submission by Vectis.

# How to Play

Arrow keys: Move
Space: Wait
A, D: Go to previous/next nucleus
W, S: Go to previous/next organelle in menu
Z: Toggle Organelle Descriptions or Message Log
X: Examine mode toggle
    Arrow keys (Examine mode): Move examine cursor
Esc: Quit
Reach 128 mass to win.

# TODO
Add context help to the organelle menu to show what pressing different keys will do.
Spawn loot at random, increase rarities.
Add "c" to "show items only"? Should items not spawn under slime? YES. ITEMS SHOULD NOT SPAWN UNDER SLIME.
IUpgradables in progress should add the things currently being used in their recipe to their UnSlime pool.
Give hunters a visual indicator of how close they are to firing

## 7DRL Agenda
Using some libraries to help
Day 1 (Saturday): Mapgen, movement, ~~map scrolling~~
Day 2 (Sunday): Slime physics, movement, growth
Day 3 (Monday): Enemies, organelles
Day 4 (Tuesday): 2 more enemies, crafting // Almost done
Day 5 (Wednesday): Inspection menu; playtest release (have a few more organs first?)
Day 6 (Thursday): Polish, balance
Day 7 (Friday): Polish

## Post-7DRL Agenda

Remove tutorial artifacts
Remove static references wherever possible
Optimize
More monsters
More organelles
More crafting materials
Environmental hazards and boons
More mapgen types
Overworld infinite-scroll map
Within-map pipes
Scratch this, too OP: ~~Enemies should panic before being engulfed if they can't pathfind out w/o running into slime.~~
See if we can build to .NET 5.0 because I like it.
Keybinding config would be nice

# Credits

Vectis you'd better fix this up by release.
.NET 5.0 by Microsoft
Engine by RLNet and RogueSharp
Font by RogueSharp