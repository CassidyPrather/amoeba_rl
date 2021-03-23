# Wishlist

Map system should index based on:
* Coordinates for: Actor, item
* Belongs to faction

Add OnSeen() trigger to things which can be placed on the map; mainly useful for adding "delay" animations and adding things to the memory layer.
Unify ICell and Point, replace most functions which refer to this ambiguously with a new type. Add implicit casts where it makes sense.
Hunters should cause hit organelles to retreat rather than be destroyed where possible.
An exclamation mark should appear over enemies that just destroyed slime tiles if they are out of line of sight.
A scheduling system wide "player speed + player's next turn" event should be available.
Use flood-fill library instead of own path-finding algirthms wherever possible
Make smart core only work in own cytoplasm
Remove stacking of quantum core from having both in and out speed bonus
Refactor organelle inheretence and upgrade tree. Maybe make the upgrade tree a separate data structure which can be bidirectionally navigated.
Need a player buff to offset haste nerf
Fix page up + page down
Better win condition + Cities rework
Encourage player mass sticking between 40-120?
Come up with a solution for annoying 1-tile corridors
Standardize command system pre/post move triggers
Standardize nucleus scheduling
Add colors to message log
Collect more stats
Add a visual indicator for working vs. non-working cultivators. On that note, rework cultivators altogether
Visual indicator for NPCs rescued in the previous turn (because they don't act until a full turn has passed)
Some mechanism to make "surrounding" units more useful
Make terror mechanic less weird
Add organelle area of effect system (useful for butchers, etc); propagate via either range or connected mass
Organelle active upkeep; cytoplasm tanks pay for upkeep before random cytoplasm is eaten
Keyword & Attribute system like TGGW
Extended examine window with keyword descriptions
New map generators; prefab handling
Items should be visible out of LOS once explored
Make map smaller or more dense
Glossary of keywords and keyword construction system for actors.

## Fun stuff

Floor loot collector organelle
Organelle organization organelle
Global cytoplasm effect skill tree
Pipe-drilling organelle

## Keywords
Armor 0
Armor 1
Spikes 1
Spikes 2
Delay [1-16]