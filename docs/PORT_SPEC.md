# AmoebaRL — Re-implementation Specification

Target audience: an engineer porting this game to Rust without access to the C# source.
Every constant in this document was read directly out of the C# at
`/home/user/amoeba_rl/AmoebaRL/`. Each section cites the files it was derived from so
values can be spot-checked.

Source version: `AmoebaRL.csproj` `<Version>4.0.0</Version>`, .NET 5.0, README says
"Post 7DRL patch (v2.1.0)". Engine dependencies: **RogueSharp 4.2.0** (map, FOV,
Dijkstra pathfinding, RNG, `Rectangle`, `Point`) and **RLNET5 2.0.2-beta** (console
window, colors, keyboard). Font: `terminal12x12_gs_ro.png` (libtcod), 12×12 px cells.

> **Critical RNG convention.** RogueSharp's `IRandom` is *inclusive on both ends*:
> `Next(max)` returns a value in `[0, max]` and `Next(min, max)` returns a value in
> `[min, max]`. Every `Rand.Next(0, list.Count - 1)` in this codebase is therefore a
> uniform pick over the whole list, and `Rand.Next(3) == 0` is a **1-in-4** chance.
> A Rust port using `0..n` exclusive ranges must add 1 everywhere.
> RNG is `DotNetRandom` seeded with `(int)DateTime.UtcNow.Ticks` (`Game.cs:196`).

---

## 1. Game overview

**Premise** (`README.md`). You are a giant amoeba in caverns beneath human cities.
Waves of humans pour out of city gates. You grow by engulfing and digesting them,
craft organelles out of their remains, and destroy the gates.

**Win condition** (`Core/Enemies/City.cs:178-203`). Walking a nucleus/organelle into a
`City` tile destroys it *only if* `DMap.PlayerMass.Count >= City.Armor`
(`Systems/CommandSystem.cs:357-368`). After `City.Destroy()` removes it, if
`Map.Cities.Count <= Game.GraceCities` the player wins:

```
"The humans try to trigger a cave-in without a blast charge, but you slip through just in time!
 You escape to the surface and live out the rest of your days in peace. "
"Final score: {PlayerMass.Count}. Time to win (A turn is 16 time units): {SchedulingSystem.GetTime()}."
"Thanks for playing!."
```

Then `CommandSystem.Win()` clears the schedule and drops a `PostMortem` actor in.
Otherwise:

```
"The humans trigger a cave-in, blocking off this exit to the surface!"
// if Cities.Count > GraceCities + 1:
"You were able to glimpse their stockpile: The humans have {Cities.Count - GraceCities - 1}
 blast charge{s} remaining..."
// else:
"The humans are out of blast charges! Now is your chance, find an exit!"
```

Cities required to destroy = `NumCities - GraceCities` (this is the number quoted in
the F1 help text, `Game.cs:228`).

**Loss condition** (`Core/Organelles/Nucleus.cs:120-130`). Checked after any Nucleus is
destroyed or unslimed. If **no `Nucleus` remains in `DMap.Actors`**:

```
"You lose. Final Score: {PlayerMass.Count}."
"Press ESC to quit. Press R to play again."
```

Schedule is cleared, a `PostMortem` actor is scheduled, FOV is recomputed.

**Difficulty modes** (`Program.cs:10-51`, defaults in `Game.cs:21-28`). Selected by
command-line argument, lowercase-compared. `--gj` takes precedence over `--easy`.

| Constant | Normal (no arg) | `--easy` | `--gj` |
|---|---|---|---|
| `MapWidth` | 48 | 48 | **64** |
| `MapHeight` | 48 | 48 | 48 |
| `DefaultSpawnRate` (turns between waves) | 50 | **75** | 50 |
| `EvolutionRate` (waves per difficulty step) | 6 | **7** | **5** |
| `MaxBudget` (max wave budget) | 5 | 5 | **6** |
| `CityArmor` (mass needed to break a gate) | 100 | 100 | **160** |
| `NumCities` | 12 | **10** | **16** |
| `GraceCities` (may be left alive) | 4 | 4 | **0** |
| Cities you must destroy | **8** | **6** | **16** |

Console prints `"GJ mode enabled."` / `"Easy mode enabled."` at startup.
`MapWidth` must not exceed 64 and `MapHeight` must not exceed 48 (the map console size).
Nothing else differs between modes — enemy stats, organelle stats, loot counts and
map-generator parameters are all mode-independent.

After the window closes, `Program.Main` loops: if `Program.PlayAgain` (set by `R` on the
post-mortem screen) a brand-new `Game` is constructed with the same options.

---

## 2. Map & generation

Files: `Systems/MapGenerator.cs`, `Core/DungeonMap.cs`, `Game.cs:203`.

`Game.StartNewGame` constructs
`new MapGenerator(this, MapWidth, MapHeight, maxBoulders: 20, boulderMaxSize: 13, boulderMinSize: 7, numCities: NumCities)`.

### 2.1 Generator constants

| Field | Value | Meaning |
|---|---|---|
| `_mapBoulders` | 20 | boulder placement attempts |
| `_boulderMaxSize` | 13 | inclusive max of `Rand.Next(min,max)` for boulder w/h |
| `_boulderMinSize` | 7 | inclusive min |
| `INITIAL_SLIME` | 4 | starting Cytoplasm count |
| `FOOD_AMT` | 32 | Nutrient items scattered |
| `DNA_AMT` | 5 | DNA items scattered |
| `WIRE_AMT` | 8 | Barbed Wire items scattered |
| `PLANT_AMT` | 8 | Plant items scattered |
| `_numCities` | 10 / 12 / 16 | per difficulty |

### 2.2 `CreateMap()` — exact step order

1. `Map.Initialize(width, height)` (RogueSharp: all cells not transparent, not walkable,
   not explored) then `InitalizeContent()` allocates the `Content[x][y] -> List<Entity>`
   spatial index.
2. **`Arena()`** — set *every* cell to `(transparent = true, walkable = true, explored = false)`;
   then set every cell in rows `0` and `height-1` to `(false, false, false)`; then every
   cell in columns `0` and `width-1` to `(false, false, false)`. Result: an open box with
   a 1-cell solid border.
3. **`PlaceBoulders()`** — 20 attempts. Each attempt:
   `w = Rand.Next(7, 13)`, `h = Rand.Next(7, 13)`,
   `x = Rand.Next(0, width - w - 1)`, `y = Rand.Next(0, height - h - 1)`.
   Build `Rectangle(x, y, w, h)`. If it does **not** `Intersects` any already-accepted
   boulder, keep it. Then for every kept boulder, `AddBoulder` sets every cell in
   `Left..Right` × `Top..Bottom` **inclusive** to `(false, false, false)`.
   *(RogueSharp `Rectangle.Right == X + Width`, `Bottom == Y + Height`, and the loop is
   `<=`, so a boulder of nominal size w×h actually blanks a (w+1)×(h+1) block.)*
   Rectangles are kept in `DungeonMap.Boulders` and never used again.
4. **`ConnectPockets()`** — see 2.3.
5. **`InitalizeNewPlayermassOnMap()`** — see 2.4.
6. **`PlaceFeatures()`** — in this exact order:
   32 × `PlaceLoot(new Nutrient())`, `NumCities` × `PlaceCity()`,
   5 × `PlaceLoot(new DNA())`, 8 × `PlaceLoot(new BarbedWire())`, 8 × `PlaceLoot(new Plant())`.
7. `UpdatePlayerFieldOfView()`.

### 2.3 `ConnectPockets()`

* `CalculatePockets()` — a column-major scan (`for x`, inner `for y`) that unions
  4-connected walkable cells into `List<List<ICell>>` pockets, merging when the cell above
  and the cell to the left belong to different pockets (with index fix-up of the
  `lastColIdx[]` row-tracking array).
* `GenerateBestPocketBridges(pockets)` — for every unordered pair `(i < j)`, brute-force
  the pair of cells `(f ∈ pocket[i], t ∈ pocket[j])` with minimum **taxicab** distance.
  Stored as a jagged triangular table `bestBridges[i][j-i-1] = (from, to, dist)`.
  This is O(|A|·|B|) per pair and is the single most expensive step of generation.
* While `pockets.Count > 1`: pick the globally shortest bridge, `RandomElbowTunnel(from,to)`,
  merge pocket `j` into pocket `i`, and repair the bridge table (parent steals the shorter
  of its own and the consumed pocket's bridge to every other pocket).
* `RandomElbowTunnel(from, to)` flips a coin (`Rand.Next(0,1) == 0`) but **both branches
  carve identical geometry** — a horizontal run along row `from.Y` spanning
  `min(from.X,to.X) .. max(from.X,to.X)`, and a vertical run along column `from.X`
  spanning `min(from.Y,to.Y) .. max(from.Y,to.Y)`. Carved cells become
  `(transparent = true, walkable = true, explored = false)`.
  Note the elbow is anchored at `from`, so the corridor reaches `(to.X, from.Y)` and
  `(from.X, to.Y)` but **not** `(to.X, to.Y)`. See §11.

### 2.4 Initial player mass

```
initialPlayer = Context.ActivePlayer ?? new Nucleus()     // always null on a fresh game
PlayerMass = [ initialPlayer, Cytoplasm, Cytoplasm, Cytoplasm, Cytoplasm, Nucleus ]  // 6 actors
do {
    initialPlayer.X = Rand.Next(0, width - 1);
    initialPlayer.Y = Rand.Next(0, height - 1);
} while (!cell.IsWalkable || !TryFluidSelect(out initialSlime, cell, 6));
for i in 0..5: playerMass[i].position = initialSlime[i]
AddActor(each); initialPlayer.SetAsActiveNucleus();
```

`FluidSelect(from, count)` is a randomized flood pick: `selection = [from]`,
`candidates = AdjacentWalkable(from)`; while `selection.Count < count`, throw
`InvalidOperationException` if `candidates` is empty, else pick
`candidates[Rand.Next(0, candidates.Count - 1)]`, remove it from candidates, append to
selection, and push its walkable neighbours that are in neither list.
So the start is a random 6-cell blob: nucleus #1 at the seed cell, 4 cytoplasm, and
nucleus #2 in the last slot.

### 2.5 Loot and city placement

`PlaceLoot(Item l)`: retry `l.pos = (Rand.Next(0,w-1), Rand.Next(0,h-1))` until the cell
`IsWalkable` **and** `IsEmpty` (no actor and no item), up to **2048** attempts; on failure
the item is silently dropped from the game.

`PlaceCity()`: loop until the chosen cell has **exactly one** walkable orthogonal
neighbour: repeatedly pick a random cell that is **not walkable** and `IsEmpty`.
Then `AddCity`. Cities therefore always sit inside rock with a single doorway.
(Note: this loop has no attempt cap and can in principle hang on a pathological map.)

### 2.6 Map queries (`Core/DungeonMap.cs`)

* `Content[x][y] : List<Entity>` — every `Entity` (actor, item, VFX, cursor) is indexed here
  by `Entity.Positions`; the setter of `Entity.Position` calls `DungeonMap.Move`.
* Parallel lists: `All`, `Actors`, `PlayerMass`, `Items`, `Cities`, `Effects` (VFX).
  Lookups `GetActorAt` / `GetItemAt` / `GetVFX` are **linear scans** of those lists.
* `AddActor` → `AddEntity` + `Actors.Add` + `SetIsWalkable(false)` +
  `SchedulingSystem.Add`; **and if the actor is an `Organelle`, every adjacent `NPC` gets
  `Engulf()` called on it** (`DungeonMap.cs:159-168`).
* `RemoveActor` → removes from `All`, `Content`, `Actors`, `PlayerMass`;
  `SetIsWalkable(true)`; `SchedulingSystem.Remove`. `Entity.Map` is deliberately **not**
  cleared so that `BecomeItem` / `BecomeActor` still work after removal.
* `RemoveCity` additionally does `SetCellProperties(x, y, true, true, true)` — the gate's
  tile becomes transparent, walkable and explored.
* `IsEmpty(x,y)` = no actor and no item. `IsWall(cell)` = `!IsWalkable && IsEmpty` —
  i.e. an actor-occupied cell is *not* a wall (this is what lets Hunter bullets pass
  through bodies and lets loot-drop BFS route around them).
* `SetIsWalkable` never touches transparency ⇒ **actors never block line of sight**.
* `Swap(a, b)` exchanges the two actors' coordinates directly; walkability is untouched
  because both cells remain occupied.
* `SetActorPosition(actor, x, y)` only succeeds if the destination `IsWalkable`; it flips
  the old cell walkable and the new cell unwalkable.
* `TaxiDistance` = Manhattan distance. `Adjacent`/`AdjacentWalkable` are 4-directional and
  bounds-clamped, always produced in order **left, right, up, down**.
* `NearestLootDrops(x, y, legalDrop, legalPath, seen, seenPerimeter)` — ring-by-ring BFS
  from `(x,y)`; a cell is a valid drop if `!IsWall(c) && !Cities.Contains(GetActorAt(c)) && legalDrop(c)`.
  Returns the whole first ring that contains any valid cell. Default predicates:
  `NoItemAndNotUnderPlayer` (no item there, and any actor there is not in `PlayerMass`)
  and `NotThroughWalls`. `NearestNoActor` uses `NotUnderActor` (cell has no actor at all).
* `NearestActors(x, y, filter)` — BFS outward from `(x,y)`, returns every actor satisfying
  `filter` on the first ring where any match is found. `filter` is called with `null` for
  empty cells, so predicates must be null-tolerant (`a is Organelle` is).
* `QuickShortestPath(map, from, to)` — temporarily forces `from` and `to` walkable, runs
  RogueSharp `PathFinder.ShortestPath` (4-way Dijkstra), restores walkability, rethrows
  `PathNotFoundException`.

---

## 3. Turn system

Files: `Systems/SchedulingSystem.cs`, `Systems/CommandSystem.cs`, `Game.cs:237-266`,
`Interfaces/ISchedulable.cs`, `Interfaces/IProactive.cs`.

### 3.1 SchedulingSystem semantics

* State: `int _time` (starts 0) and `SortedDictionary<int, List<ISchedulable>> _scheduleables`.
* `Add(s)` → bucket key `_time + s.Time`, where `ISchedulable.Time` returns `Actor.Delay`.
  Appended to the end of that key's list.
* `Get()` → take the **lowest key**, take the **first element** of its list (FIFO within a
  tie), remove it, set `_time = key`, return it. Ties are broken by insertion order.
* `Remove(s)` → linear scan of buckets in key order, removes the **first** bucket
  containing `s`, deletes the bucket if it becomes empty.
* `ScheduledFor(s)` → the key of the first bucket containing `s`, or `null`.
* `Clear()` resets `_time = 0` and empties the schedule (used on win and on loss).
* **One turn = 16 time units.** The UI prints `Turn: time / 16` as a float.

### 3.2 `CommandSystem.AdvanceTurn()`

```
do {
    nextUp = SchedulingSystem.Get();          // also advances _time
    if (nextUp is Nucleus n) {
        DMap.UpdatePlayerFieldOfView();
        IsPlayerTurn = true;
        n.SetAsActiveNucleus();               // reschedules ALL nuclei; recolors the drag path
    } else if (nextUp is PostMortem) {
        DMap.UpdatePlayerFieldOfView();
        IsPlayerTurn = true;
        Context.ActivePlayer = null;          // switches input handling to "meta" mode
        SchedulingSystem.Add(nextUp);
    } else if (nextUp is IProactive behavior) {
        behavior.Act();
        if (DMap.Actors.Contains(nextUp)) SchedulingSystem.Add(nextUp);   // don't reschedule self-destructed things
    } else {
        SchedulingSystem.Add(nextUp);         // inert schedulables (Cytoplasm, plain Membrane, Butcher...)
    }
    if (nextUp is IPostSchedule post) post.DoPostSchedule();   // only TerrorCore
} while (!IsPlayerTurn);
```

FOV is recomputed **only** when a Nucleus or the PostMortem pops. All calls to
`UpdatePlayerFieldOfView` inside `AddActor`, `RemoveActor`, `Swap`, `Upgrade` etc. are
commented out for performance (commit `53bf881`).

### 3.3 Input / turn coupling

`ASCIIGraphics.OnRootConsoleUpdate` polls the keyboard every frame (RLNET update tick) and
calls `Game.HandleUserInput(press)`, where `press` may be `null`:

```
if (CommandSystem.IsPlayerTurn) { AcceptUserInput(press); return true; }   // renderRequired
else { CommandSystem.AdvanceTurn(); return false; }
```

So the whole NPC phase runs inside one update tick, and while it is the player's turn the
screen redraws every frame. `AcceptUserInput` routes to examine mode → organelle mode →
live mode → meta mode (see §10) and, if the handler returns `true`, calls
`CommandSystem.EndPlayerTurn()` (`IsPlayerTurn = false`).

**What a player turn costs.** Nothing in the action handlers touches the scheduler for a
normal move — the active nucleus was already re-inserted at `time + Delay` by
`SetAsActiveNucleus()` when it popped. So *any* successful player action (move, swap,
eat, wait, destroy gate) costs exactly the active nucleus's `Delay`. Exceptions:

* `QuantumCore` swapping into a slimed tile: `Delay /= 2` (8 → **4**) and
  `SetAsActiveNucleus()` is re-run, so the swap only costs 4 time units.
  `IPostAttackMove.DoPostAttackMove` then restores `Delay = BaseSpeed = 8` for next time.
* Pressing `A`/`D` (cycle nucleus) or entering examine/organelle mode does **not** end the
  turn, but `NextNucleus` calls `SetAsActiveNucleus()`, which reschedules **every** nucleus
  to `time + newActive.Delay`.
* Failed actions (walking into a wall, into an armoured NPC without a Laser Core /
  Reinforced Maw, into a gate with insufficient mass) return `false` and do **not** end the
  turn.

**Nucleus rescheduling (`Nucleus.SetAsActiveNucleus`, `Nucleus.cs:36-54`).**

```
Context.ActivePlayer = this;
SchedulingSystem.Remove(this); SchedulingSystem.Add(this);       // -> _time + this.Delay
foreach (other nucleus n in PlayerMass) {
    SchedulingSystem.Remove(n);
    buffer = n.Delay; n.Delay = this.Delay;                       // temporarily adopt the active speed
    SchedulingSystem.Add(n);                                      // -> _time + this.Delay
    n.Delay = buffer;
}
ColorMovingSlime();
```

All nuclei therefore always pop at the same instant, and the FIFO tie-break inside the
bucket picks whichever was inserted first — but because the popped nucleus immediately
calls `SetAsActiveNucleus` again, control does not drift. This is the "only one nucleus can
move per turn" rule from the Nucleus description text.

---

## 4. Player amoeba mechanics

Files: `Systems/CommandSystem.cs`, `Core/Organelles/Nucleus.cs`, `Core/Actor.cs`.

### 4.1 The model

The player is a **set** (`DungeonMap.PlayerMass : List<Actor>`) of organelles. There is no
single "player entity". Score and gate-breaking threshold are both `PlayerMass.Count`.
`Actor.Slime > 0` means "player-aligned" (`IsPlayerAligned()`); it is also the background
colour index: `0` = none, `1` = body (dark green), `2` = on the drag path (bright green).
A `Nucleus` is the only organelle the user directly commands. `Game.ActivePlayer` is the
current one.

### 4.2 `AttackMoveOrganelle(player, x, y)` — the master action

`CommandSystem.cs:312-411`. Used for player moves *and* for every AI-driven organelle
move (Maw, Tentacle, Gravity Core pulls, Extractor pulls).

```
if (player is IPreMove pre) pre.DoPreMove();                 // QuantumCore: Delay = 8
targetActor = GetActorAt(x, y);
if (targetActor != null) {
    if (targetActor.Slime > 0) {                             // SWAP with own body
        Swap(player, targetActor);
        if (targetActor is CraftingMaterial c) c.TryUpgrade(player);
        if (player is QuantumCore q) { player.Delay /= 2; q.SetAsActiveNucleus(); }
        success = true;
    }
    else if (targetActor is NPC n && n.Armor > 0) {          // Tank / Mech / Caravan
        if (player is ReinforcedMaw) { log "The {t} is crushed by the jaws of the {p}!"; EatActor(); success = true; }
        else if (player is LaserCore) { log "The {t} is obliterated by the {p}'s laser beam!"; EatActor(); success = true; }
        else { log "The {t}'s armor is too strong for the {p}!"; success = false; }
    }
    else if (targetActor is City c) {
        if (PlayerMass.Count >= c.Armor) { c.Destroy(); success = true; }
        else log "There is not enough mass to destroy the {t}! (Have {PlayerMass.Count}, need {c.Armor})";
    }
    else if (targetActor is IEatable) {                      // unarmoured NPC
        log "The {p} consumes the {t}."; EatActor(); success = true;
    }
} else {
    targetItem = GetItemAt(x, y);
    if (targetItem != null) {
        if (targetItem is IEatable) {                        // all items are Catalysts
            success = Ingest(player, targetItem) ? true : MoveOrganelle(player, x, y);
        }
    } else success = MoveOrganelle(player, x, y);
}
if (success) {
    foreach adjacent cell of player's NEW position: if actor is NPC n -> n.Engulf();
    if (player is IPostAttackMove p) p.DoPostAttackMove();
}
```

`AttackMovePlayer(player, Direction)` just offsets by one cell
(`Up` = `y-1`, `Down` = `y+1`, `Left` = `x-1`, `Right` = `x+1`) and forwards here.

### 4.3 The drag / pull-path rule — `MoveOrganelle` (exact algorithm)

`CommandSystem.cs:438-499`. Only reached when the destination cell holds **no actor and no
item** (or ingestion failed). This is what the README calls "you drag a path of organelles
behind you".

**Step 1 — BFS over the slime graph.**

```
counter = 1; max = 0
root = Node(player, parent = null, dist = 0)
last = [root];  accountedFor = [root]
loop {
    frontier = []
    foreach l in last {
        pullIn = DMap.Actors.Where(a =>
                     a.Slime > 0
                  && (a is not Organelle o || !o.Anchor)          // anchored organelles are immovable
                  && a.AdjacentTo(l.current)                      // 4-neighbourhood, taxi distance == 1
                  && accountedFor contains no node whose .current == a)
        foreach a in pullIn (in DMap.Actors order) {
            max = counter
            node = Node(a, parent = l, dist = counter)
            accountedFor.Add(node); frontier.Add(node)
        }
    }
    counter++; last = frontier
    if (frontier is empty) break
}
```

This is a breadth-first spanning tree of the connected slime mass rooted at the mover.
`max` ends up as the eccentricity (greatest BFS depth reached). Membership in the graph is
`Slime > 0`, which includes cytoplasm, every organelle, crafting materials **and dissolving
NPCs** — but excludes any `Organelle` whose `Anchor` flag is set (only the Gravity Core
sets it, and only during its own post-move pulls).

**Step 2 — choose one deepest tail and reconstruct the path.**

```
best = accountedFor.Where(p => p.dist == max)
selected = best[Rand.Next(0, best.Count - 1)]
path = []
while (selected != null) { path.Add(selected.current); selected = selected.parent; }
path.Reverse()                       // path[0] == player, path[last] == the chosen tail
```

**Step 3 — move, then cascade.**

```
lastPoint = player.position
if (WithinBounds(x,y) && SetActorPosition(player, x, y)) {
    for (i = 1; i < path.Count; i++) {
        buffer = path[i].position
        SetActorPosition(path[i], lastPoint)
        lastPoint = buffer
    }
    return true
}
return false      // destination not walkable -> nothing moved, turn not consumed
```

Every organelle on the chosen chain shuffles forward one cell into the cell vacated by its
predecessor. The tail's original cell becomes empty. Net effect: the mass "flows" one cell
in the direction of travel while remaining connected.

**Highlighting (`Nucleus.ColorMovingSlime`, `Nucleus.cs:56-106`).** Called from
`SetAsActiveNucleus`, i.e. at the start of every player turn and on every `A`/`D` press.
It runs the *identical* BFS and then:

```
foreach a in PlayerMass: a.Slime = 1
foreach node in accountedFor.Where(p => p.dist == max):
    walk node -> parent -> ... -> root, setting .Slime = 2 on each
```

So the bright-green highlight is the **union of every maximum-depth path**, whereas the
actual drag uses **one uniformly random** deepest tail out of that set. When multiple tails
are tied, the highlight over-reports which tiles will move.

### 4.4 Swapping

Moving into any actor with `Slime > 0` swaps positions and does **not** drag the tail. Two
side effects fire on swap:

* If the target is a `Calcium` or `Electronics` (`CraftingMaterial`),
  `CraftingMaterial.TryUpgrade(player)` runs — this is the *only* way a Nucleus can be
  upgraded, because `CraftingMaterial.Act()` explicitly skips `Nucleus` when it
  auto-crafts with neighbours.
* If the mover is a `QuantumCore`, the action costs 4 time units instead of 8.

### 4.5 Multiple nuclei, A/D cycling

`CommandSystem.NextNucleus(shift)`:

```
nuclei = PlayerMass.Where(a => a is Nucleus).ToList()     // list order == acquisition order
curIdx = nuclei.IndexOf(ActivePlayer)                     // -1 if the active one just died
newIdx = (curIdx + shift) % nuclei.Count; if (newIdx < 0) newIdx += nuclei.Count
nuclei[newIdx].SetAsActiveNucleus()
```

`A` = `shift -1`, `D` = `shift +1`. Free (does not end the turn) but reschedules all nuclei
to `time + newActive.Delay`. Crashes with a divide-by-zero if there are no nuclei — which
cannot happen because the game is over and `ActivePlayer` is `null` by then, routing input
to meta mode.

### 4.6 What happens when a nucleus dies

* **Melee** (`CommandSystem.Attack`, victim is a Nucleus): first `CheckAndSave` (§6.5).
  If not saved, `Nucleus.Retreat()`:
  `sacrifices = PlayerMass.Where(taxi distance == 1 && !(a is Nucleus) && a is Organelle)`;
  prefer those **not** standing under a `Reticle`; pick uniformly at random; `Swap` the
  nucleus with it and return it. The attacker then recursively `Attack`s the organelle that
  just took the nucleus's place ("The {nucleus} retreated into the nearby {x}, thereby
  avoiding death"). If no sacrifice exists: "{name} could not retreat and was destroyed"
  and `Destroy()`.
* **Hunter fire** (`Hunter.Fire`): same `Retreat()`, but the victim is `Unslime()`d
  (full component drop) rather than `Destroy()`ed.
* Either way `Nucleus.OnDestroy` / `OnUnslime` run the base drop logic and then
  `HandleGameOver()`, which ends the run only when the *last* nucleus is gone.
* `Game.ActivePlayer` can be left pointing at a removed nucleus until the next nucleus pops
  off the scheduler; `NextNucleus` tolerates this via the `IndexOf == -1` path.

### 4.7 Eating: `IEatable` / `IEngulfable` / `IDigestable`

| Interface | Contract | Implementors |
|---|---|---|
| `IEatable` | `void OnEaten()` — become part of the consuming group | `NPC` (→ its `BecomesOnEaten` dissolving form, added to `PlayerMass`), `Catalyst` (→ its `NewOrganelle()`, added to `PlayerMass`) |
| `IEngulfable` | `bool Engulf()`, `bool CanEngulf(set)`, `void ProcessEngulf()` | `NPC` only |
| `IDigestable` | `int HP`, `int MaxHP`, `int Overfill` | `DissolvingNPC` only |
| `ISlayable` | `void Die()` — drop `BecomesOnDie` items | `NPC` |

**`EatActor(eating, eaten)`** (`CommandSystem.cs:213-231`):

```
under = GetItemAt(eaten.pos)              // remember the item beneath the victim
if (eaten is IEatable e) { Swap(eating, eaten); e.OnEaten(); }
else RemoveActor(eaten);
if (under != null && under is IEatable && !(under is Nutrient)) Ingest(eating, under);
```

Because of the `Swap`, the eater advances onto the victim's cell and the victim's dissolving
corpse materialises **behind** the eater, on the eater's former cell. A non-Nutrient item
that was under the victim is then ingested too (Nutrients are skipped deliberately, "we can
do two moves as long as we don't accidentally eat a nutrient").

**`Ingest(eating, item)`** (`CommandSystem.cs:233-291`):

* `Nutrient` → `OnEaten()` immediately: item removed, a fresh `Cytoplasm` spawns **on the
  item's cell** and joins `PlayerMass`. Net mass **+1**.
* Any other `Catalyst` (DNA, Barbed Wire, Plant, Calcium Dust, Silicon Dust) → find a host
  `Cytoplasm`:
  * if `eating` is itself a `Cytoplasm`, it is the host (`moveAndTransform = true`);
  * else BFS outward through `PlayerMass` adjacency from `eating`, one ring at a time,
    collecting every `Cytoplasm` on the first ring that has any; if a ring is empty,
    **return `false`** ("no room to eat") and the caller falls back to `MoveOrganelle`.
  * pick uniformly among the found cytoplasm, `RemoveActor` it, move the item entity to that
    cell, and `OnEaten()` — the new organelle appears there and joins `PlayerMass`.
    Net mass **0** (a cytoplasm was spent).
* Then `AttackMoveOrganelle(eating, item's original cell)` — the eater steps onto the tile
  the item occupied.

**Engulfing** (`Core/Enemies/NPC.cs:87-148`). An NPC is engulfed when it is *sealed in*:

```
CanEngulf(engulfing):
    engulfing.Add(this)
    adj = Map.Adjacent(x, y)                       // 4 orthogonal, in-bounds
    if any adj cell IsWalkable -> return false     // escape route
    if any adjacent actor is a City -> return false // "Cities will not help to engulf their friends!"
    hasEscape = false
    foreach adjacent actor a that is IEngulfable and not already in `engulfing`:
        if (!a.CanEngulf(engulfing)) hasEscape = true
    return !hasEscape
```

`Engulf()` builds the set, and if `CanEngulf` holds, calls `ProcessEngulf()` on **every**
member: logs `"The {Name} is engulfed!"` and runs `OnEaten()`. So a contiguous clump of
humans is captured all at once, but a City adjacent to any member breaks the seal for the
whole group. Walls count as sealing; `DissolvingNPC`s are Organelles (not `IEngulfable`) so
they seal too.

Engulf checks fire in three places: at the top of `Militia.Act` / `Hunter.Act` /
`Caravan.Act`; in `DungeonMap.AddActor` whenever an `Organelle` is added next to an NPC;
and in `AttackMoveOrganelle` after any successful organelle move.

**Digestion timing** (`NPC.cs:163-291`). Every `DissolvingNPC` is an `Organelle` with
`Slime = 1`, `Delay = 16`, `Awareness = 0`, so it ticks once per turn:

```
Act():
    HP--
    if (HP <= 0) {
        numButchers = PlayerMass.Count(k => k is Butcher)
        repeat numButchers times { Overfill = MaxHP * 2; ProduceIfOverfull(); }
        RemoveActor(this)
        spawn DigestsTo at own cell; AddActor; PlayerMass.Add
    }

ProduceIfOverfull():
    if (Overfill >= MaxHP) {
        drops = NearestNoActor(x, y)
        if (drops.Count > 0) { spawn DigestsTo at drops[Rand.Next(drops.Count-1)]; PlayerMass.Add; Overfill = 0 }
        return true
    }
    return false
```

Attacking a dissolving corpse **rescues** it: both `OnUnslime` and `OnDestroy` are overridden
to `BecomeActor(RescuesTo)`, restoring the live enemy.

---

## 5. Organelles

Files: `Core/Organelles/*.cs`, `Core/Actor.cs`, `UI/TextTilePalette.cs`.

### 5.1 Base semantics

`Organelle : Actor` (`Organelle.cs`) adds:

* `bool Anchor` (default `false`) — excluded from the drag BFS while set.
* `virtual List<Item> Components()` — everything used to build it.
* `Unslime()` = `RemoveActor` + `OnUnslime()`, default `BecomeItems(Components())` —
  **drops everything**. Used by Hunter fire.
* `Destroy()` = `RemoveActor` + `OnDestroy()`, default `BecomeItem(Components()[0])` —
  **drops only the first component**. Used by melee.
* `BecomeItem` places the item on `NearestLootDrop`; if there is nowhere,
  `"The {name} had nowhere to drop and was crushed!"`.
  `BecomeItems` walks outward rings reusing a `seen` buffer, same message on failure.

`Upgradable : Organelle` (`Upgradable.cs`) adds the crafting system:

```
class UpgradePath { int AmountRequired; Resource TypeRequired; Func<Organelle> Result; }
List<UpgradePath> PossiblePaths;      // set in each constructor
UpgradePath CurrentPath = null;       // locked in on first matching material
int Progress = 0;

Components() = OrganelleComponents() + (CurrentPath's material) x AmountRequired

Upgrade(material):
    if (CurrentPath == null) CurrentPath = first p in PossiblePaths with p.TypeRequired == material
    if (CurrentPath != null && CurrentPath.TypeRequired == material) {
        Progress++
        if (Progress >= CurrentPath.AmountRequired) {
            RemoveActor(this); result = BecomeActor(CurrentPath.Result()); PlayerMass.Add(result)
            if (result is Nucleus n) n.SetAsActiveNucleus()       // avoids control drifting to another nucleus
            log "The {oldName} absorbs the {material} and transforms into a {result.Name}!"
        } else log "The {Name} absorbs the {material}"
        return true
    }
    return false
```

Note `Components()` charges the *full* `AmountRequired` for the in-progress path even at
`Progress == 1`, so an interrupted upgrade refunds more than was invested when unslimed.

`CraftingMaterial : Organelle, IProactive` — the "catalyst delivery" actors
(`Calcium`, `Electronics`), `Delay = 1` so they tick **16 times per turn**:

```
Act():
    adjUpg = adjacent actors that are IUpgradable and NOT Nucleus
    while (adjUpg not empty) {
        picked = adjUpg[Rand.Next(0, adjUpg.Count-1)]; adjUpg.Remove(picked)
        if (picked.Upgrade(Provides)) {
            byproduct = new Cytoplasm at own cell; PlayerMass.Add(byproduct)
            RemoveActor(this); BecomeActor(byproduct)
            break
        }
    }

TryUpgrade(recipient):     // called when something swaps INTO this material
    same, but on a single explicit recipient (this is the nucleus upgrade route)
```

Consuming a crafting material leaves a `Cytoplasm` in its place, so player mass is conserved.

### 5.2 Nucleus line

Glyph `@` for all. `NucleusTextTile` shows `ActiveColor` when
`Game.ActivePlayer == this`, otherwise `InactiveColor`.

| Class | Name | Glyph | Active colour | Inactive colour | Awareness | Delay | Slime | Upgrade paths |
|---|---|---|---|---|---|---|---|---|
| `Nucleus` | Nucleus | `@` | `RootOrganelle` (208,70,72) | `PlayerInactive` (88,56,72) | 3 | 16 | 1 | 1 Calcium → Eye Core; 2 Electronics → Smart Core |
| `EyeCore` | Eye Core | `@` | `Calcium` (107,113,247) | `RestingTank` (48,52,109) | **6** | 16 | 1 | 2 Calcium → Laser Core; 2 Electronics → Terror Core |
| `SmartCore` | Smart Core | `@` | `Electronics` (218,212,94) | `RestingTank` (48,52,109) | 3 | **8** | 1 | 2 Calcium → Gravity Core; 3 Electronics → Quantum Core |
| `LaserCore` | Laser Core | `@` | `SuperBright` (222,238,214) | `RestingTank` (48,52,109) | 6 | 16 | 1 | terminal |
| `TerrorCore` | Terror Core | `@` | `OrganelleInactive` (133,149,161) | `PlayerInactive` (88,56,72) | 6 | 16 | 1 | terminal |
| `GravityCore` | Gravity Core | `@` | `DarkSlime` (52,101,36) | `PlayerInactive` (88,56,72) | 3 | 8 | 1 | terminal |
| `QuantumCore` | Quantum Core | `@` | `Cursor` = LightMagenta (255,127,255) | `PlayerInactive` (88,56,72) | 3 | 8 (4 on swap) | 1 | terminal |

Components (drop lists), from `OrganelleComponents()`:

| Class | Components |
|---|---|
| `Nucleus` | DNA, Nutrient |
| `EyeCore` | DNA, Nutrient, Calcium Dust |
| `SmartCore` | DNA, Nutrient, Silicon Dust |
| `LaserCore` | DNA, Nutrient, Calcium Dust ×3 |
| `TerrorCore` | DNA, Nutrient, Calcium Dust, Silicon Dust ×2 |
| `GravityCore` | DNA, Nutrient, Silicon Dust, Calcium Dust ×2 |
| `QuantumCore` | DNA, Nutrient, Silicon Dust ×4 |

Melee `Destroy()` therefore drops only a **DNA** for any nucleus.

Special behaviours:

* **Eye Core** — vision only (Awareness 6).
* **Smart Core** — `Delay 8` ⇒ two actions per game turn.
* **Laser Core** — can `EatActor` armoured NPCs (Tank/Mech/Caravan) by walking into them.
* **Terror Core** (`IPostAttackMove`, `IPostSchedule`):
  ```
  DoPostAttackMove():                       // after every successful move
      Terrified.Clear()
      foreach adjacent actor that is Militia (incl. subclasses):
          t = SchedulingSystem.ScheduledFor(a)
          if (t.HasValue) {
              untilTurn = t - SchedulingSystem.GetTime()
              SchedulingSystem.Remove(a)
              Terrified.Add((a, a.Delay))       // remember original delay
              a.Delay += untilTurn + 16
          } else log "{a.Name} is already terrified."     // the code comments this as a bug
  DoPostSchedule():                         // when the Terror Core pops off the scheduler
      foreach (a, originalDelay) in Terrified {
          SchedulingSystem.Add(a); a.Delay = originalDelay; SetAsActiveNucleus();
      }
  ```
  Net effect: an adjacent human loses roughly two turns; the effect "stacks" only in the
  sense that it can be reapplied. `Terrified` is never cleared by `DoPostSchedule`, so if
  the player waits instead of moving, the same actors get re-added a second time (§11).
* **Gravity Core** (`IPostAttackMove`), `GravityAttempts = 2`, `MaxRange = 2`:
  ```
  Anchor = true
  repeat 2 times {
      adj = AdjacentWalkable(self)                      // empty neighbouring cells
      if (adj empty) continue
      gravityTo = adj[Rand.Next(adj.Count - 1)]
      nearest = NearestActors(self.x, self.y, a => a is Organelle
                        && TaxiDistance(a, gravityTo) <= 2
                        && a != this
                        && a.PathExists(x => x is Militia && !(x is Tank), gravityTo))
      while (nearest not empty) {
          sel = nearest[Rand.Next(nearest.Count - 1)]; nearest.Remove(sel)
          p = sel.PathIgnoring<Actor>(x => x is Militia && !(x is Tank), gravityTo)
          if (p != null) { AttackMoveOrganelle(sel, p.StepForward()); nearest.Clear(); }
      }
  }
  Anchor = false
  ```
  i.e. after each Gravity Core move, up to two nearby organelles are yanked one step toward
  an empty cell next to it, treating unarmoured humans as walkable.
* **Quantum Core** (`IPreMove`, `IPostAttackMove`), `BaseSpeed = 8`: `DoPreMove` and
  `DoPostAttackMove` both set `Delay = 8`; the swap branch halves it to 4 and immediately
  reschedules. So: 8 units to move/eat, 4 units to swap.

### 5.3 Cytoplasm and crafting materials

| Class | Name | Glyph | Colour | Awareness | Delay | Slime | Components | Notes |
|---|---|---|---|---|---|---|---|---|
| `Cytoplasm` | Cytoplasm | `' '` (space) | `Slime` (109,170,44) | 0 | 16 | 1 | Nutrient | `OnDestroy` overridden to drop **nothing** |
| `Calcium` | Calcium | `$` | `Calcium` (107,113,247) | 0 | **1** | 1 | Calcium Dust, Nutrient | `Provides = CALCIUM` |
| `Electronics` | Electronics | `$` | `Electronics` (218,212,94) | 0 | **1** | 1 | Silicon Dust, Nutrient | `Provides = ELECTRONICS` |

Cytoplasm is inert (not `IProactive`), so its scheduler entry just re-queues itself every
16 units. Crafting materials are hidden from the Organelle log (`OrganelleLog.GetLoggable`
filters out `Cytoplasm` and `CraftingMaterial`).

### 5.4 Membrane line (from Barbed Wire)

| Class | Name | Glyph | Colour | Awareness | Delay | Upgrade paths | Components |
|---|---|---|---|---|---|---|---|
| `Membrane` | Membrane | `B` | `RootOrganelle` (208,70,72) | 0 | 16 | 1 Calcium → Tough Membrane; 1 Electronics → Maw | Barbed Wire, Nutrient |
| `ReinforcedMembrane` | Tough Membrane | `B` | `Calcium` | 0 | 16 | 3 Calcium → Force Field; 1 Electronics → Phase Membrane | Barbed Wire, Nutrient, Calcium Dust |
| `Maw` | Maw | `W` | `Electronics` | **1** | 16 | 3 Calcium → Reinforced Maw; 3 Electronics → Tentacle | Barbed Wire, Nutrient, Silicon Dust |
| `ForceField` | Force Field | `F` | `Calcium` | 0 | 16 | terminal | Barbed Wire, Nutrient, Calcium Dust ×4 |
| `NonNewtonianMembrane` | Phase Membrane | `P` | `Electronics` | 0 | 16 | terminal | Barbed Wire, Nutrient, Calcium Dust, Silicon Dust |
| `ReinforcedMaw` | Reinforced Maw | `W` | `Calcium` | 1 | 16 | terminal | Barbed Wire, Nutrient, Silicon Dust, Calcium Dust ×3 |
| `Tentacle` | Tentacle | `T` | `Electronics` | **3** | **4** | terminal | Barbed Wire, Nutrient, Silicon Dust ×4 |

Combat properties are implemented in `CommandSystem.Attack` (§6.5), not on the classes:

* Any `Membrane` (including Maw/Tentacle) **kills** an attacker with `Armor == 0`.
* `ReinforcedMembrane`, `ForceField`, `Phase Membrane`, `ReinforcedMaw` also kill attackers
  with `Armor > 0` (Tank/Mech/Caravan). Plain `Membrane`, `Maw` and `Tentacle` are destroyed
  by armoured attackers instead ("The {monster} shrugs off the {victim}'s proteins").
* `ForceField` additionally protects **allies** (§6.5).
* `NonNewtonianMembrane` intercepts attacks on adjacent allies by swapping with the victim
  and killing the attacker.

**Maw AI** (`IProactive`, `HungerRange = 2`):

```
Act():
    smellArea = GetCellsInDiamond(x, y, 2)
    desire = actors-or-items in smellArea satisfying IsHungryFor
    if (desire non-empty) {
        nearest = those at minimum taxi distance
        ActToTargets(nearest, through: e => e is Organelle)
    }

IsHungryFor(e) = (e is Militia && !(e is Tank)) || e is Item        // ReinforcedMaw also: || e is Tank

ActToTargets(targets, through):
    paths = PathsThroughToNearest(targets, through)    // paths that traverse ONLY matching actors
    pick a random shortest path
    next = path.StepForward()
    if (IsHungryFor(entity at next) || entity at next is Organelle) AttackMoveOrganelle(this, next)
```

`PathThrough` temporarily marks *every* walkable cell unwalkable, then marks only cells
occupied by matching actors walkable, then runs Dijkstra — so a Maw crawls through its own
body toward food.

**Tentacle AI** — `Delay 4` ⇒ four actions per turn, `Awareness 3`, `TerrorRadius = 1`:

```
Act():
    seen = Seen(s => s is Militia && !(s is Caravan))       // taxi distance <= 3, ignores walls
    brave = true
    if (any target is a Tank that is not a Caravan) {
        tanks = those targets
        if (any tank within taxi distance 1) {
            if (MinimizeTerrorMove(tanks)) brave = false     // stepped away
        }
        if (brave) seen = seen without tanks
    }
    if (seen non-empty && brave) ActToTargets(seen, through: a => a is Actor act && act.Slime > 0)

MinimizeTerrorMove(sources):
    best = ImmediateUphillStep(sources, canPassThroughOthers: true)
    if (best == own cell) return false
    AttackMoveOrganelle(this, best); return true
```

### 5.5 Chloroplast line (from Plant)

| Class | Name | Glyph | Colour | Awareness | Delay | `NextFood` init | Upgrade paths | Components |
|---|---|---|---|---|---|---|---|---|
| `Chloroplast` | Chloroplast | `H` | `RootOrganelle` | 0 | 16 | **20** | 1 Calcium → Bioreactor; 1 Electronics → Cultivator | Plant, Nutrient |
| `Bioreactor` | Bioreactor | `R` | `Calcium` | 0 | 16 | 16 | 3 Calcium → Biometal Forge; 2 Electronics → Primordial Soup | Plant, Nutrient, Calcium Dust |
| `Cultivator` | Cultivator | `U` | `Electronics` | 0 | 16 | n/a | 2 Calcium → Extractor; 2 Electronics → Butcher | Plant, Nutrient, Silicon Dust |
| `BiometalForge` | Biometal Forge | `G` | `Calcium` | 0 | 16 | 16 | terminal | Plant, Nutrient, Calcium Dust ×4 |
| `PrimordialSoup` | Primordial Soup | `S` | `Electronics` | 0 | 16 | 16 | terminal | Plant, Nutrient, Calcium Dust, Silicon Dust ×2 |
| `Extractor` | Extractor | `V` | `Calcium` | 0 | 16 | n/a | terminal | Plant, Nutrient, Silicon Dust, Calcium Dust ×2 |
| `Butcher` | Butcher | `K` | `Electronics` | 0 | 16 | n/a | **not `Upgradable`** | Plant, Nutrient, Silicon Dust ×3 |

**Production loop** (`Chloroplast.Act`):

```
NextFood--
if (NextFood <= 0 && Produce()) NextFood = Delay      // Delay == 16 for every member of the line
```

`Produce()` picks a cell from `NearestNoActor(x, y)` uniformly and spawns there:

| Class | Produces |
|---|---|
| `Chloroplast`, `Bioreactor` | `Cytoplasm` |
| `BiometalForge` | `Calcium` (50%) or `Electronics` (50%) — `Rand.Next(1) == 0` |
| `PrimordialSoup` | see below |

`PrimordialSoup.Produce`:

```
choice = (PlayerMass.Count(x => x.Name == "Nucleus") > 4) ? Rand.Next(3) : Rand.Next(4)
choice < 2 -> new Membrane
choice < 4 -> new Chloroplast
else       -> new Nucleus
```

With the cap active (more than 4 **unupgraded** nuclei — the check is a literal name
comparison, so `Eye Core`, `Smart Core`, … do not count): 50% Membrane, 50% Chloroplast.
Otherwise: 40% Membrane, 40% Chloroplast, 20% Nucleus. (Commit `7df9f39`.)

**Cultivator** (`OverfillRate = 1`):

```
Act():
  foreach adjacent DissolvingNPC m:
      if (m.HP < m.MaxHP) m.HP += min(2, m.MaxHP - m.HP)     // out-heals its own 1/turn decay
      m.Overfill += 1
      m.ProduceIfOverfull()                                   // free product every MaxHP turns
```

**Extractor** (Cultivator + a puller):

```
Act():
  destinations = AdjacentWalkable(self) + cells of adjacent actors that are NOT IDigestable
  shuffle destinations (repeated Rand.Next(count-1) removal)
  foreach dest in destinations:
      wants = NeedsToReach(dest)                              // see §11 — the min-distance filter is a no-op
      if (wants empty) break
      do {
          getsToGo = wants[Rand.Next(wants.Count-1)]
          p = getsToGo.PathIgnoring<Organelle>(x => PlayerMass.Contains(x)
                                                 && !(x is Extractor)
                                                 && !(x is DissolvingNPC && x.AdjacentTo(self)), dest)
          if (p == null) wants.Remove(getsToGo)
      } while (p == null && wants non-empty)
      if (p != null) AttackMoveOrganelle(getsToGo, p.StepForward())
  base.Act()                                                  // then behave as a Cultivator
```

`NeedsToReach(dest)` = `PlayerMass` members that are `IDigestable`, are `Organelle`s and are
**not** already adjacent to the Extractor.

**Butcher** is inert; it is read by `DissolvingNPC.Act`, which grants **one extra product
per Butcher in the whole player mass** when a corpse finishes dissolving.

### 5.6 Organelle log UI behaviour

`Systems/OrganelleLog.cs`, rendered by `UI/PlayerConsole.cs`.

* `Tracking` is the *same list object* as `DungeonMap.PlayerMass`.
* `GetLoggable()` = `Tracking` minus `Cytoplasm` and minus `CraftingMaterial`, in
  `PlayerMass` order (acquisition order).
* `idx` = selection cursor, wrapped by `Scroll(by)`: `idx = (idx + by) % count`, `+= count`
  if negative. `Highlighted` = `GetLoggable()[idx]` cast to `Organelle` (null if out of
  range or not an organelle).
* `page` scrolls by whole screens: `Page(by)` adds and clamps at ≥ 0. `PlayerConsole`
  computes `effectivePage = page % ceil(count / 55)` (only when `page != 0`), and **overrides
  it** to the page containing the examine target when the examine cursor is over a logged
  organelle.
* `NiceTurnBuffer` is a monotone maximum of `time/16` so that `SchedulingSystem.Clear()` on
  win/loss does not reset the displayed turn counter.
* `InfoConsole.DrawOrganelle` clamps `idx` to 0 if it exceeds the list, and renders the
  selected entry's `Description`.

---

## 6. Enemies & NPCs

Files: `Core/Enemies/NPC.cs`, `Militia.cs`, `Tank.cs`, `Hunter.cs`, `City.cs`,
`Systems/CommandSystem.cs`.

### 6.1 Live enemy stat block

`Armor 0` = killable/eatable by walking into it. `Armor 1` = "armoured"; only a
`ReinforcedMaw` or `LaserCore` can eat it by contact, and it dies to reinforced membranes,
Hunter fire or engulfing.

| Class | Name | Glyph | Colour (active / resting) | Armor | Awareness | Delay | AI | `BecomesOnDie` | `BecomesOnEaten` |
|---|---|---|---|---|---|---|---|---|---|
| `Militia` | Militia | `m` | `Militia` (210,125,44) | 0 | 3 | 16 | approach & attack | 1 Nutrient | Dissolving Militia |
| `Hunter` | Hunter | `h` | `Electronics` (218,212,94) | 0 | 3 | 16 | ranged, `Range 64` | 1 Silicon Dust | Dissolving Hunter |
| `Scout` | Scout | `s` | `Electronics` | 0 | **4** | 16 | ranged, `Range 3` | 1 Silicon Dust | Dissolving Scout |
| `Tank` | Tank | `t` | `Calcium` (107,113,247) / `RestingTank` (48,52,109) | **1** | 3 | **48** | approach & attack | 1 Calcium Dust | Dissolving Tank |
| `Mech` | Mech | `c` | `Calcium` / `RestingTank` | **1** | 3 | **32** | approach & attack | 1 Calcium Dust | Dissolving Mech |
| `Caravan` | Caravan | `v` | `Militia` (210,125,44) / `RestingMilitia` (133,76,48) | **1** | 3 | **32** | **flees** | 1 × `Cargo()` = Calcium Dust 50% / Silicon Dust 50% | Dissolving Caravan |
| `City` | City Gate | `C` | `City` (133,149,161) | — | 0 | 16 | spawns waves | — | — |

`Tank`, `Mech` and `Caravan` render with `SlowTextTile`: the "active" colour is shown when
`ScheduledFor(actor) - now <= 16`, i.e. it will act within one turn. `Tank.StaminaPoolSize`
= `Delay / 16` = 3 (Mech/Caravan: 2) and `TurnsToAct` = `(scheduledTime - now) / 16`; both
appear in the description text.

`Hunter` and `Scout` render with `RangedTextTile`: while `Firing < FiringTime`, on alternate
animation frames the glyph is replaced with a direction arrow — `(char)16` `►` for +x,
`(char)17` `◄` for −x, `(char)31` `▼` for +y, `(char)30` `▲` for −y.

### 6.2 Basic AI (`Militia.Act`, inherited by Tank/Mech/Hunter/Scout)

```
if (Engulf()) return;                                        // captured this turn instead of acting
seenTargets = Seen(a => a.IsPlayerAligned())                 // see the WARNING below
if (seenTargets.Count > 0) ActToTargets(seenTargets)
else Wander()

ActToTargets(targets):
    paths = PathsToNearest(targets)                          // Dijkstra, obstacles = unwalkable cells
    if (paths.Count > 0) {
        picked = paths[Rand.Next(0, paths.Count - 1)]
        try { AttackMove(this, picked.StepForward()) }
        catch (NoMoreStepsException) { log "The {Name} contemplates the irrationality of its existence." }
    }   // otherwise: wait a turn

Wander():
    adj = AdjacentWalkable(x, y)
    pick = Rand.Next(0, adj.Count)                           // inclusive -> 1/(count+1) chance to stand still
    if (pick != adj.Count) AttackMove(this, adj[pick])

AttackMove(monster, cell):
    if (!SetActorPosition(monster, cell)) {                   // blocked
        target = GetActorAt(cell)
        if (PlayerMass.Contains(target)) Attack(monster, target)
    }
```

> **WARNING — "sight" is not line of sight.** `Actor.Seen(Func<Actor,bool>)`
> (`Core/Actor.cs:146-157`) iterates `Map.GetCellsInDiamond(X, Y, Awareness)` and filters —
> it never calls FOV. Humans therefore detect the amoeba through solid rock at taxicab
> distance ≤ `Awareness`. The `Seen(IEnumerable<Actor>)` overload *does* use FOV but is only
> reachable from commented-out code. This was a deliberate perf change (commit `81bfdca`).

`PathsToNearest` returns every shortest path tied for minimum `Path.Length` (RogueSharp
`Path.Length` counts cells including the start, so the step count is `Length - 1`).

### 6.3 Caravan AI

```
if (Engulf()) return
seen = Seen(a => a.IsPlayerAligned())
if (seen.Count > 0) AttackMove(this, ImmediateUphillStep(seen, canPassThroughOthers: false))
else Wander()
```

`ImmediateUphillStep(sources, canPassThroughOthers)` (`Core/Actor.cs:187-251`) is a greedy
one-step flee: it maximises the **sum** of taxicab distances to all sources over
{stay put, adjacent walkable cells, (optionally) adjacent non-source non-City actors}, with
a coin flip when the best sacrifice ties the best free space.

### 6.4 Hunter / Scout ranged AI

State: `FiringTime = 2`, `Firing = 2` (starts equal), `Range` (Hunter 64, Scout 3),
`FiringDirection : Point`, `Targeted : List<Reticle>`.

```
Act():
    if (Engulf()) return
    if (Firing <= 0)          Fire()
    else if (Firing < FiringTime) Firing--          // charging, does nothing else
    else                      base.Act()            // Militia behaviour, which may call ActToTargets

ActToTargets(targets):                              // override
    paths = PathsToNearest(targets)
    do {
        picked = paths[Rand.Next(0, paths.Count - 1)]
        target = picked.Steps.Last()
        paths.Remove(picked)
        HasFiringPath = picked.Length <= Range + 1 && (target.X == X || target.Y == Y)
    } while (!HasFiringPath && paths.Count > 0)

    if (HasFiringPath) {
        sights = (X, Y)
        if      (target.X > X) sights.X++
        else if (target.X < X) sights.X--
        else if (target.Y > Y) sights.Y++
        else                   sights.Y--
        FiringDirection = sights - (X, Y)
        bullet = sights; travelled = 0
        while (WithinBounds(bullet) && !IsWall(bullet) && travelled < Range) {
            travelled++
            add Reticle VFX at bullet; Targeted.Add(it)
            bullet += FiringDirection
        }
        Firing--                                    // 2 -> 1
    } else base.ActToTargets(targets)               // fall back to melee approach

Fire():
    Firing = FiringTime                             // reset to 2
    foreach Reticle r in Targeted:
        hit = GetActorAt(r)
        if (hit == null) continue
        hitCount++
        if (hit is Nucleus n)   { v = n.Retreat(); (v ?? n).Unslime(); }
        else if (hit is Organelle o) o.Unslime()    // drops ALL components
        else if (hit is Militia m)   m.Die()        // friendly fire kills Tanks/Mechs/Caravans too
    if (hitCount > 0) log "The {Name} hit {hitCount} mass."
    ClearReticles(); Targeted.Clear()
```

Timeline: turn *n* aims (paints reticles, `Firing 2→1`); turn *n+1* charges (`Firing 1→0`);
turn *n+2* fires. `IsWall` is false for actor-occupied cells, so the beam **penetrates
bodies** and only stops at rock or after `Range` cells. `Die()` and `OnEaten()` are
overridden to clear reticles first.

### 6.5 How enemies damage the amoeba — `CommandSystem.Attack(monster, victim)`

```
if (victim is Nucleus n) {
    if (!CheckAndSave(monster, victim)) {
        newVictim = n.Retreat()
        if (newVictim == null) { log "{victim.Name} could not retreat and was destroyed"; n.Destroy(); }
        else { log "The {victim.Name} retreated into the nearby {newVictim.Name}, thereby avoiding death";
               Attack(monster, newVictim); }                         // recursive
    }
}
else if (victim is Membrane m && monster is ISlayable i) {           // NOTE: no CheckAndSave here
    if (monster.Armor > 0) {
        if (victim is ReinforcedMembrane || victim is ReinforcedMaw) { log "The {monster} is impaled by sharp {victim} proteins!"; i.Die(); }
        else { log "The {monster} shrugs off the {victim}'s proteins"; m.Destroy(); }
    } else { log "The {monster} is impaled by sharp {victim} proteins!"; i.Die(); }
}
else if (victim is Organelle o) {
    if (!CheckAndSave(monster, victim)) { log "The {monster} destroys the {victim}"; o.Destroy(); }
}
else { log "A {victim} is destroyed by a {monster}"; RemoveActor(victim); }
```

`CheckAndSave(monster, victim)` (`CommandSystem.cs:170-211`), `LONG_RANGE_FF_DIST = 3`:

1. If `monster` is **not** a `Tank` (so not Tank/Mech/Caravan): if any actor within
   `GetCellsInDiamond(victim, 3)` is a `ForceField` → saved.
   `"An energy mantle force protects the {victim} from the {monster}"`.
2. If still unsaved: if any *adjacent* actor is a `ForceField` → saved (same message).
   This branch applies to Tanks too.
3. If still unsaved: gather adjacent `NonNewtonianMembrane`s; if any, pick one at random,
   `Swap(savior, victim)`,
   `"The {savior} rematerializes and protects the {victim} from the {monster}!"`, and if the
   monster is an `NPC`, `n.Die()` +
   `"{monster} is killed by the rematerializing phase membrane!!"`.

### 6.6 Dissolving forms (`DissolvingNPC`)

All have `Awareness 0`, `Slime 1`, `Delay 16`, are `Organelle`s in `PlayerMass`, and lose
1 HP per turn.

| Class | Name | Glyph | Colour | MaxHP | `DigestsTo` | `RescuesTo` |
|---|---|---|---|---|---|---|
| `Militia.CapturedMilitia` | Dissolving Militia | `m` | `Militia` | **8** | Cytoplasm | Militia |
| `Hunter.CapturedHunter` | Dissolving Hunter | `h` | `Electronics` | **16** | Electronics | Hunter |
| `Scout.CapturedScout` | Dissolving Scout | `s` | `Electronics` | **16** | Electronics | Scout |
| `Tank.CapturedTank` | Dissolving Tank | `t` | `Calcium` | **24** | Calcium | Tank |
| `Mech.CapturedMech` | Dissolving Mech | `c` | `Calcium` | **24** | Calcium | Mech |
| `Caravan.CapturedCaravan` | Dissolving Caravan | `v` | `Militia` | **24** | Calcium 50% / Electronics 50% | Caravan |

`CapturedCaravan.NameOfResult` is hard-coded to `"Calcium (50%) or Electronics (50%)"`.
`NameOfResult` is otherwise lazily cached from `DigestsTo.Name` on first read.

### 6.7 City behaviour

`City : Actor, IProactive, IDescribable`. `Awareness 0`, `Delay 16`, glyph `C`,
`VisibilityCondition.EXPLORED_ONLY` (a remembered gate is drawn in `DbStone` (117,113,97)).

Per-instance fields: `WaveRate = null` (lazily = `Game.DefaultSpawnRate`),
`TurnsToNextWave = 50` (**hard-coded initial value, not `DefaultSpawnRate`**),
`WaveNumber = 0`, `CityLevel = 1`, `SpawnQueue : Queue<Actor>`,
`Armor` lazily = `Game.CityArmor`.
Unit costs: `ScoutCost 2`, `HunterCost 3`, `TankCost 2`, `MechCost 3`.

```
Act():                                    // once per 16 time units
    TurnsToNextWave--
    if (TurnsToNextWave <= 0) {
        SpawnNextWave(min(Game.MaxBudget, CityLevel))
        CityLevel = (WaveNumber / Game.EvolutionRate) + 2        // integer division; WaveNumber already incremented
    }
    if (SpawnQueue.Count > 0) {
        spawnAreas = AdjacentWalkable(x, y)                      // order: left, right, up, down
        if (spawnAreas.Count > 0) { baby = SpawnQueue.Dequeue(); place at spawnAreas[0]; AddActor(baby); }
    }

SpawnNextWave(budget):
    stock = budget
    while (stock > 0) stock = AddNewMilitia(stock)
    // caravan roll, using WaveNumber BEFORE increment:
    if      (WaveNumber == 0) hasCaravan = Rand.Next(3) == 0      //  1/4
    else if (WaveNumber < 4)  hasCaravan = Rand.Next(19) <= 2     //  3/20
    else                      hasCaravan = Rand.Next(19) == 0     //  1/20
    if (hasCaravan) SpawnQueue.Enqueue(new Caravan())
    WaveNumber++
    if (WaveRate == null) WaveRate = Game.DefaultSpawnRate
    TurnsToNextWave += WaveRate
```

Because only **one** unit is released per city turn, the queue drains at one human per turn
and stalls entirely if the gate's single doorway is blocked.

**Spawn table** — `AddNewMilitia(budget)` builds the allowed set and picks uniformly:

```
allowed = { 0 }                                     // 0 = militia group
if (budget >= MechCost   /*3*/) allowed += 1        // Mech
else if (budget >= TankCost /*2*/) allowed += 2     // Tank
if (budget >= HunterCost /*3*/) allowed += 3        // Hunter
else if (budget >= ScoutCost /*2*/) allowed += 4    // Scout
pick = allowed[Rand.Next(allowed.Count - 1)]

0 -> enqueue min(budget, 3) Militia;  return budget - min(budget, 3)
1 -> enqueue Mech;                    return budget - 3
2 -> enqueue Tank;                    return budget - 2
3 -> enqueue Hunter;                  return budget - 3
4 -> enqueue Scout;                   return budget - 2
```

| Budget | Options (uniform) |
|---|---|
| 1 | 1 Militia |
| 2 | 2 Militia · Tank · Scout (⅓ each) |
| 3 | 3 Militia · Mech · Hunter (⅓ each) |
| 4 | 3 Militia · Mech · Hunter (⅓ each), then recurse with 1 → +1 Militia |
| 5 | 3 Militia · Mech · Hunter (⅓ each), then recurse with 2 |
| 6 (GJ only) | 3 Militia · Mech · Hunter (⅓ each), then recurse with 3 |

Note Tanks and Scouts can **only** appear at exactly budget 2, and Mechs/Hunters only at
budget ≥ 3.

**Budget over time.** Wave *k* (0-indexed) uses `min(MaxBudget, CityLevel)` where
`CityLevel` before wave *k* is `1` for *k* = 0 and `floor(k / EvolutionRate) + 2` thereafter.

| Wave index | Normal (`ER 6`, `MB 5`) | Easy (`ER 7`, `MB 5`) | GJ (`ER 5`, `MB 6`) |
|---|---|---|---|
| 0 | 1 | 1 | 1 |
| 1 … | 2 (waves 1–5) | 2 (waves 1–6) | 2 (waves 1–4) |
| | 3 (waves 6–11) | 3 (waves 7–13) | 3 (waves 5–9) |
| | 4 (waves 12–17) | 4 (waves 14–20) | 4 (waves 10–14) |
| | 5 (waves 18+) | 5 (waves 21+) | 5 (waves 15–19) |
| | — | — | 6 (waves 20+) |

**Wave timing.** First wave at turn 50 for every difficulty (the hard-coded
`TurnsToNextWave = 50`), then every `DefaultSpawnRate` turns: **50** (normal/GJ) or **75**
(easy). Every city runs its own independent timer, and all cities start with the same
timer, so waves come out of all gates simultaneously.

**Gate HP.** Cities have no HP. They are destroyed in one action if
`PlayerMass.Count >= Armor` (100 normal/easy, 160 GJ) and are otherwise invulnerable.

**City description text** is generated dynamically and reports `Armor`, `SpawnQueue.Count`,
`CityLevel` and `TurnsToNextWave`.

---

## 7. Items & catalysts

`Core/Item.cs`, `Core/Catalyst.cs`, plus per-line files. Every `Item` in the game is a
`Catalyst : Item, IEatable, IDescribable`:

```
OnEaten():
    Map.RemoveItem(this)
    Actor t = NewOrganelle(); t.position = this.position
    Map.AddActor(t); Map.PlayerMass.Add(t)
```

Only one item may occupy a cell (`PlaceLoot` and `NoItemAndNotUnderPlayer` enforce it);
items do not block movement and an actor can stand on one.

| Class | Name | Glyph | Colour | `NewOrganelle()` | Description |
|---|---|---|---|---|---|
| `Nutrient` | Nutrient | `%` | `Slime` (109,170,44) | `Cytoplasm` | "This precious meal is the foundation of growth." |
| `CalciumDust` | Calcium Dust | `%` | `Calcium` (107,113,247) | `Calcium` | "This precious powder could be used to build strong bones." |
| `SiliconDust` | Silicon Dust | `%` | `Electronics` (218,212,94) | `Electronics` | "These rocks contain the magic of humanity, and could be used to accelerate evolution." |
| `BarbedWire` | Barbed Wire | `*` | `OrganelleInactive` (133,149,161) | `Membrane` | "Humans set these up to protect their cities. You can probably put it to better use." |
| `Plant` | Plant | `?` | `OrganelleInactive` (133,149,161) | `Chloroplast` | "A cute green plant. Its ability to use the sun to produce food is fascinating and could be exploited." |
| `DNA` | DNA | `&` | `OrganelleInactive` (133,149,161) | `Nucleus` | "Short for Deoxyribonucleic Acid. It would be possible to fasion a new nucleus out of this." |

All items use `VisibilityCondition.LOS_ONLY` and background `FloorBackgroundFov` (20,12,28).

Ingestion rules recap (§4.7): a **Nutrient** converts in place (mass +1). Every other
catalyst consumes the nearest `Cytoplasm` in the mass as its host cell (mass unchanged),
and if there is no cytoplasm anywhere in the connected mass, ingestion fails and the eater
just walks onto the tile instead. `Reticle` (Hunter targeting marker) is an `Entity` in the
VFX layer, not an item.

---

## 8. Field of view & visibility

`Core/DungeonMap.cs:71-94`, `Core/Actor.cs:131-177`, `Interfaces/VisibilityCondition.cs`,
`UI/TextTile.cs`.

**Player FOV** — recomputed only in `CommandSystem.AdvanceTurn` when a `Nucleus` or the
`PostMortem` pops:

```
grantsVision = PlayerMass.Where(a => a.Awareness >= 0)          // in practice: everything
first  -> ComputeFov(a.X, a.Y, a.Awareness, lightWalls: true)
rest   -> AppendFov(a.X, a.Y, a.Awareness, lightWalls: true)
if none -> ComputeFov(0, 0, 0, false)                           // "player is blind"
foreach cell in FOV: mark IsExplored = true
```

So visibility is the **union** of a diamond around every single organelle in the mass —
a cytoplasm with `Awareness 0` still lights its own tile, which is why the amoeba's own
body is always visible.

**FOV algorithm (RogueSharp 4.2 `FieldOfView.ComputeFov`).** Cast Bresenham lines from the
origin to every border cell of the bounding square of side `2·radius+1`; walk each line and
stop when `|dx| + |dy| > radius` (**taxicab radius ⇒ diamond-shaped FOV, not a circle**) or
when a non-transparent cell is reached (that wall cell is included because `lightWalls` is
true); then a quadrant post-process removes walls that should be hidden behind other walls.
Transparency comes only from `Cell.IsTransparent`, which the game sets **only in
`MapGenerator`** — `SetIsWalkable` preserves transparency, so **actors never block sight**.

`Actor.FOV(lightWalls)` builds a per-actor `FieldOfView` and is used only by the
`Seen(IEnumerable<Actor>)` overload, which is unreachable in the shipped code.
The overload used by every AI, `Seen(Func<Actor,bool>)`, is a pure taxicab-diamond scan
with **no occlusion** (see §6.2 warning).

**Memory.** `Cell.IsExplored` is sticky once set. The base map layer draws explored cells
even out of FOV (dark colours). Item and actor tiles use `VisibilityCondition`:

| Value | Behaviour in `TextTile.Draw` |
|---|---|
| `INVISIBLE` | never drawn as itself; falls through to `Backup`, else to a floor glyph |
| `LOS_ONLY` | drawn only when the cell is currently in FOV; if explored but not in FOV, falls through to `Backup` then to a dark `.` floor glyph. Everything except City and Cursor. |
| `EXPLORED_ONLY` | drawn in full colour when in FOV; drawn in `DbStone` (117,113,97) with its own background when explored but out of FOV. Used by `City` only. |
| `ALWAYS_VISIBLE` | drawn unconditionally, even on unexplored cells. Used by `Cursor` only. |

Exact `TextTile.Draw` order:

```
if (Visibility not in {ALWAYS_VISIBLE, EXPLORED_ONLY} && !IsExplored()) return;
if (Visibility == ALWAYS_VISIBLE || (IsInFov() && Visibility != INVISIBLE))
    console.Set(x, y, Color, BackgroundColor, Symbol);
else if (Visibility == EXPLORED_ONLY && IsExplored() && Visibility != INVISIBLE)
    console.Set(x, y, Palette.DbStone, BackgroundColor, Symbol);
else if (Backup != null) Backup.Draw(console);
else if (IsInFov())   console.Set(x, y, Palette.FloorFov,  Palette.FloorBackgroundFov, '.');
else if (IsExplored())console.Set(x, y, Palette.Floor,     Palette.FloorBackground,    '.');
```

`Backup` is the tile of whatever the entity is standing on: `ActorTextTile` sets it to the
`Item` beneath; `ReticleTextTile` sets it to the actor-or-item beneath. This is how a
blinking reticle reveals what it covers on its "off" frame.

There is **no item memory** — an item you have seen disappears from the display once it
leaves FOV (noted as a TODO in `Core/Item.cs` and in `Wishlist.md`).

`Cursor.Under()` returns the actor-or-item at the cursor **only if** it is `IDescribable`
and (it is a `City` on an explored cell **or** the cell is currently in FOV).

---

## 9. UI

Files: `UI/ASCIIGraphics.cs`, `MapConsole.cs`, `InfoConsole.cs`, `PlayerConsole.cs`,
`Palette.cs`, `TextTile.cs`, `ActorTextTile.cs`, `ReticleTextTile.cs`, `TextTilePalette.cs`,
`Systems/MessageLog.cs`, `Systems/OrganelleLog.cs`.

### 9.1 Console layout

| Console | Size (cells) | Blit position on root |
|---|---|---|
| `MapConsole` | `MAP_WIDTH = 64` × `MAP_HEIGHT = 48` | (0, 0) |
| `InfoConsole` | `INFO_WIDTH = 64` (= map width) × `INFO_HEIGHT = 11` | (0, 48) |
| `PlayerConsole` | `PLAYER_WIDTH = 22` × `PLAYER_HEIGHT = 59` (= 48 + 11) | (64, 0) |
| `RLRootConsole` | 86 × 59 | window title `"Amoeba RL"` |

Font `terminal12x12_gs_ro.png`, 12×12 px, scale 1.0 ⇒ window 1032 × 708 px.
The map is drawn 1:1 with no scrolling or camera; map coordinates are console coordinates.
This is why `MapWidth ≤ 64` and `MapHeight ≤ 48`.

Render loop: `ASCIIGraphics.NeedsAnimationUpdate()` advances `AnimationFrame` and returns
true every `ANIMATION_RATE = 250 ms`; a frame is rendered when that fires or when
`_renderRequired` (true whenever it is the player's turn). Order per frame:
`GenerateRepresentation()` (rebuild the whole `Tiles` list by scanning every map cell,
row-major, preferring VFX over actor-or-item, then appending the examine cursor last) →
`RenderMapBase` (clear, draw every explored cell's floor/wall, then `Animate` every tile) →
`InfoCanvas.DrawContent` → `PlayerCanvas.DrawContent` → blit all three → `RootConsole.Draw()`.

Base map glyphs (`ASCIIGraphics.SetConsoleSymbolBackground`):

| Cell | Glyph | Foreground | Background |
|---|---|---|---|
| unexplored | *(nothing drawn)* | — | — |
| walkable, in FOV | `.` | `FloorFov` (129,121,107) | `FloorBackgroundFov` (20,12,28) |
| wall, in FOV | `#` | `WallFov` (93,97,105) | `WallBackgroundFov` (51,56,64) |
| walkable, explored | `.` | `Floor` (71,62,45) | `FloorBackground` = black (0,0,0) |
| wall, explored | `#` | `Wall` (72,77,85) | `WallBackground` (31,38,47) |

Actor backgrounds (`ActorTextTile.BackgroundColor`) are driven by `Actor.Slime`:
`0` → `FloorBackgroundFov` (20,12,28); `1` → `BodySlime` (99,143,42); `2` → `PathSlime`
(132,190,56); anything else → `FloorBackground` (0,0,0).

### 9.2 Palette (exact RGB)

Named ramps (`Palette.cs:19-60`), all constructed with the int constructor (value/255):

| Name | RGB | | Name | RGB |
|---|---|---|---|---|
| `PrimaryLightest` | 110,121,119 | | `DbDark` | 20,12,28 |
| `PrimaryLighter` | 88,100,98 | | `DbOldBlood` | 68,36,52 |
| `Primary` | 68,82,79 | | `DbDeepWater` | 48,52,109 |
| `PrimaryDarker` | 48,61,59 | | `DbOldStone` | 78,74,78 |
| `PrimaryDarkest` | 29,45,42 | | `DbWood` | 133,76,48 |
| `SecondaryLightest` | 116,120,126 | | `DbVegetation` | 52,101,36 |
| `SecondaryLighter` | 93,97,105 | | `DbBlood` | 208,70,72 |
| `Secondary` | 72,77,85 | | `DbStone` | 117,113,97 |
| `SecondaryDarker` | 51,56,64 | | `DbWater` | 89,125,206 |
| `SecondaryDarkest` | 31,38,47 | | `DbBrightWood` | 210,125,44 |
| `AlternateLightest` | 190,184,174 | | `DbMetal` | 133,149,161 |
| `AlternateLighter` | 158,151,138 | | `DbGrass` | 109,170,44 |
| `Alternate` | 129,121,107 | | `DbSkin` | 210,170,153 |
| `AlternateDarker` | 97,89,75 | | `DbSky` | 109,194,202 |
| `AlternateDarkest` | 71,62,45 | | `DbSun` | 218,212,94 |
| `ComplimentLightest` | 190,180,174 | | `DbLight` | 222,238,214 |
| `ComplimentLighter` | 158,147,138 | | | |
| `Compliment` | 129,116,107 | | | |
| `ComplimentDarker` | 97,84,75 | | | |
| `ComplimentDarkest` | 71,56,45 | | | |

Game-use aliases (`Palette.cs:63-112`):

| Alias | Definition | Resulting RGB |
|---|---|---|
| `FloorBackground` | `RLColor.Black` | 0,0,0 |
| `Floor` | `AlternateDarkest` | 71,62,45 |
| `FloorBackgroundFov` | `DbDark` | 20,12,28 |
| `FloorFov` | `Alternate` | 129,121,107 |
| `WallBackground` | `SecondaryDarkest` | 31,38,47 |
| `Wall` | `Secondary` | 72,77,85 |
| `WallBackgroundFov` | `SecondaryDarker` | 51,56,64 |
| `WallFov` | `SecondaryLighter` | 93,97,105 |
| `TextHeading` | `DbLight` | 222,238,214 |
| `TextBody` | `DbBrightWood` | 210,125,44 |
| `SuperBright` | `DbLight` | 222,238,214 |
| `PlayerInactive` | `DbOldBlood + RLColor(20,20,20)` | **88,56,72** |
| `Slime` | `DbGrass` | 109,170,44 |
| `DarkSlime` | `DbVegetation` | 52,101,36 |
| `PathSlime` | `RLColor(132,190,56)` | 132,190,56 |
| `BodySlime` | `PathSlime × 0.75` | **99,143,42** (99, 142.5, 42) |
| `City` | `DbMetal` | 133,149,161 |
| `Militia` | `DbBrightWood` | 210,125,44 |
| `RestingMilitia` | `DbWood` | 133,76,48 |
| `Calcium` | `RLColor(DbWater.r×1.2, DbWater.g×0.9, DbWater.b×1.2)` | **≈107,113,247** |
| `RestingTank` | `DbDeepWater` | 48,52,109 |
| `Electronics` | `DbSun` | 218,212,94 |
| `ReticleForeground` | `DbBlood` | 208,70,72 |
| `ReticleBackground` | `DbOldBlood` | 68,36,52 |
| `RootOrganelle` | `DbBlood` | 208,70,72 |
| `OrganelleInactive` | `DbMetal` | 133,149,161 |
| `InactiveGravityCore` | `DbOldBlood` | 68,36,52 *(unused)* |
| `InactiveQuantumCore` | `Magenta × (0.6, 1, 0.6)` | ≈153,0,153 *(unused)* |
| `TerrorCoreActive` | `LightGray` with `b × 1.2` | ≈191,191,229 *(unused)* |
| `Cursor` | `RLColor.LightMagenta` | 255,127,255 |
| `DarkCursor` | `RLColor.Magenta` | 255,0,255 |
| `SmartCoreInactive` | `DbOldStone` | 78,74,78 *(unused)* |
| `Overfill` | `DbSky` | 109,194,202 |
| `OrganelleConsoleBG` | `RLColor(6,26,0)` | 6,26,0 |

The four "unused" entries are never referenced by `TextTilePalette`. The derived colours
are computed in RLNET's **normalized float** space (`RLColor(int)` divides by 255,
`RLColor(float)` does not), so a port must normalize before multiplying/adding, then clamp.
RLNET's standard colors used here are `Black (0,0,0)`, `Magenta (255,0,255)`,
`LightMagenta (255,127,255)`, `LightGray (191,191,191)` — verify against the RLNET5 package
if exact fidelity matters.

### 9.3 Complete glyph table

`UI/TextTilePalette.cs` dispatches on the runtime type name; unknown types render as
`?` in `Cursor`/`DarkCursor`.

| Glyph | Entity | Foreground | Background | Tile class / visibility |
|---|---|---|---|---|
| `%` | Nutrient | `Slime` | `FloorBackgroundFov` | `TextTile` / LOS |
| `%` | Calcium Dust | `Calcium` | `FloorBackgroundFov` | `TextTile` / LOS |
| `%` | Silicon Dust | `Electronics` | `FloorBackgroundFov` | `TextTile` / LOS |
| `*` | Barbed Wire | `OrganelleInactive` | `FloorBackgroundFov` | `TextTile` / LOS |
| `?` | Plant | `OrganelleInactive` | `FloorBackgroundFov` | `TextTile` / LOS |
| `&` | DNA | `OrganelleInactive` | `FloorBackgroundFov` | `TextTile` / LOS |
| `' '` | Cytoplasm | `Slime` | by `Slime` | `ActorTextTile` / LOS |
| `$` | Electronics | `Electronics` | by `Slime` | `ActorTextTile` / LOS |
| `$` | Calcium | `Calcium` | by `Slime` | `ActorTextTile` / LOS |
| `@` | Nucleus | `RootOrganelle` / `PlayerInactive` | by `Slime` | `NucleusTextTile` / LOS |
| `@` | Eye Core | `Calcium` / `RestingTank` | by `Slime` | `NucleusTextTile` / LOS |
| `@` | Smart Core | `Electronics` / `RestingTank` | by `Slime` | `NucleusTextTile` / LOS |
| `@` | Laser Core | `SuperBright` / `RestingTank` | by `Slime` | `NucleusTextTile` / LOS |
| `@` | Terror Core | `OrganelleInactive` / `PlayerInactive` | by `Slime` | `NucleusTextTile` / LOS |
| `@` | Gravity Core | `DarkSlime` / `PlayerInactive` | by `Slime` | `NucleusTextTile` / LOS |
| `@` | Quantum Core | `Cursor` / `PlayerInactive` | by `Slime` | `NucleusTextTile` / LOS |
| `B` | Membrane | `RootOrganelle` | by `Slime` | `ActorTextTile` / LOS |
| `B` | Tough Membrane | `Calcium` | by `Slime` | `ActorTextTile` / LOS |
| `W` | Maw | `Electronics` | by `Slime` | `ActorTextTile` / LOS |
| `F` | Force Field | `Calcium` | by `Slime` | `ActorTextTile` / LOS |
| `P` | Phase Membrane | `Electronics` | by `Slime` | `ActorTextTile` / LOS |
| `W` | Reinforced Maw | `Calcium` | by `Slime` | `ActorTextTile` / LOS |
| `T` | Tentacle | `Electronics` | by `Slime` | `ActorTextTile` / LOS |
| `H` | Chloroplast | `RootOrganelle` | by `Slime` | `ActorTextTile` / LOS |
| `R` | Bioreactor | `Calcium` | by `Slime` | `ActorTextTile` / LOS |
| `U` | Cultivator | `Electronics` | by `Slime` | `ActorTextTile` / LOS |
| `G` | Biometal Forge | `Calcium` | by `Slime` | `ActorTextTile` / LOS |
| `S` | Primordial Soup | `Electronics` | by `Slime` | `ActorTextTile` / LOS |
| `V` | Extractor | `Calcium` | by `Slime` | `ActorTextTile` / LOS |
| `K` | Butcher | `Electronics` | by `Slime` | `ActorTextTile` / LOS |
| `m` | Militia | `Militia` | by `Slime` (0) | `ActorTextTile` / LOS |
| `v` | Caravan | `Militia` / `RestingMilitia` | by `Slime` | `SlowTextTile` / LOS |
| `t` | Tank | `Calcium` / `RestingTank` | by `Slime` | `SlowTextTile` / LOS |
| `s` | Scout | `Electronics` | by `Slime` | `RangedTextTile` / LOS |
| `c` | Mech | `Calcium` / `RestingTank` | by `Slime` | `SlowTextTile` / LOS |
| `h` | Hunter | `Electronics` | by `Slime` | `RangedTextTile` / LOS |
| `m` | Dissolving Militia | `Militia` | by `Slime` (1) | `ActorTextTile` / LOS |
| `v` | Dissolving Caravan | `Militia` | by `Slime` | `ActorTextTile` / LOS |
| `t` | Dissolving Tank | `Calcium` | by `Slime` | `ActorTextTile` / LOS |
| `s` | Dissolving Scout | `Electronics` | by `Slime` | `ActorTextTile` / LOS |
| `c` | Dissolving Mech | `Calcium` | by `Slime` | `ActorTextTile` / LOS |
| `h` | Dissolving Hunter | `Electronics` | by `Slime` | `ActorTextTile` / LOS |
| `C` | City Gate | `City`; `DbStone` when remembered | by `Slime` (0) | `CityTextTile` / **EXPLORED_ONLY** |
| `X` | Reticle | `ReticleForeground` | `ReticleBackground` | `ReticleTextTile` / LOS, blinks |
| `X` | Cursor | `Cursor` | `DarkCursor` | `ReticleTextTile` / **ALWAYS_VISIBLE**, blinks |
| `?` | anything unregistered | `Cursor` | `DarkCursor` | `TextTile` / LOS |
| `►◄▼▲` | Hunter/Scout while charging | as Hunter | by `Slime` | chars 16/17/31/30 |
| `0`–`9`, `*` | City timer overlay | `Electronics` fg, `ReticleForeground`/`ReticleBackground` bg | | see below |

Animated tiles (`Speed` = animation-frames per step, `Frames` = number of steps; the
frame index is `(AnimationFrame / Speed) % Frames`, and `AnimationFrame` ticks every 250 ms):

* `RangedTextTile` — `Frames 2`, `Speed 3`. `FiringBlink = (idx == 0)`; the arrow glyph is
  shown when `Firing < FiringTime` **and** `FiringBlink`.
* `ReticleTextTile` — `Frames 2`, `Speed 3`. `ForceInvisible = (idx != 0)`; when invisible
  the `Backup` (whatever is underneath) is drawn instead.
* `CityTextTile` — `Frames 2`, `Speed 3`.
  `ShowCounter = idx != 0 && (SpawnQueue.Count > 0 || TurnsToNextWave < 10)`.
  When showing: glyph is `'*'` if the queue exceeds 9, else the queue count digit; if the
  queue is empty but `TurnsToNextWave < 10`, the countdown digit. Colours while counting:
  fg `TimerPrimary` = `Electronics`; bg `TimerSecondary` = `ReticleForeground` when the
  queue is non-empty, `TimerTetriary` = `ReticleBackground` when counting down.

### 9.4 Message log

`Systems/MessageLog.cs`. `_maxLines = INFO_HEIGHT - 2 = 9`, wrap width
`maxLen = INFO_WIDTH - 2 = 62`. `Add(msg)`: if `msg.Length <= 62`, enqueue and dequeue the
oldest while `Count > 9`; otherwise word-wrap and `Add` each resulting line.
`WrapText(text, width)` is a greedy splitter on `" "`; note each emitted line carries a
trailing space, and a single word longer than the width produces an empty first line.
`InfoConsole.DrawMessage` clears and prints line *i* at `(1, i+1)` in `RLColor.White`.

### 9.5 Info console (bottom bar, 64 × 11)

Constructed with background `PrimaryDarker` and `"Log"` at `(1,1)`, but `DrawContent`
clears every frame, so that header is only ever visible pre-first-render. `DrawContent`
switches on `Game.Showing`: `MESSAGE` → message log; `ORGANELLE` → describe
`OrganelleLog.GetLoggable()[idx]`; `EXAMINE` → describe `ExamineCursor.Under()`.

`Describe(d)`:
1. `Clear()`.
2. Name at `(1,1)`. Colour: `Organelle` → `Slime`; `Militia` or `City` → `Militia`;
   `Item` → `RootOrganelle`; otherwise `TextHeading`.
3. `Description` word-wrapped to 62 columns, printed from row 3 in `TextHeading`.
4. If `Upgradable`: with no `CurrentPath`, one line per possible path —
   `"It can be upgraded with {N} {Material}."` with the material word recoloured at
   column 27 (`Calcium` → `Palette.Calcium`, `Electronics` → `Palette.Electronics`).
   With a `CurrentPath`: `"It needs {AmountRequired - Progress} more {Material}."`,
   recoloured at column 17.
5. If it is an `Actor` standing on an `Item`: `"It is standing on a {Name}."`, item name
   recoloured at column 21 with the item's glyph colour.

### 9.6 Player console (right sidebar, 22 × 59)

`_maxLines = PLAYER_HEIGHT - 4 = 55`, `_nameWidth = PLAYER_WIDTH - 4 = 18`.
Constructed with `DbWood` background; each draw resets it to `OrganelleConsoleBG` (6,26,0).

```
(1,1) "Organelles"                    TextHeading
(1,2) "Mass: {PlayerMass.Count}"      TextBody
(1,3) "Turn: {niceturn}"              TextBody      // max(time/16.0, previous value), float
rows 5.. : one entry per GetLoggable() item, row = i + 5 - effectivePage*55
```

Per row:
* `>` at column 1 in `Palette.Cursor` if this entry is what the examine cursor is over.
* `>` at column 1 in `TextHeading` plus the name at column 3 if `i == log.idx` **and**
  the game is in `ORGANELLE` mode.
* Name at column 3 in the entity's glyph colour, except `DarkSlime` is remapped to `Slime`
  (so the Gravity Core is readable).
* `Chloroplast` progress bar: `cutoff = floor(18 × (1 - NextFood / Delay))`; that span gets
  background `Overfill` (109,194,202) and foreground `RootOrganelle`.
* `IDigestable` bar: `cutoff = floor(18 × (1 - HP/MaxHP))`; `[3, 3+cutoff)` background
  `Slime`, `[3+cutoff, 21)` background `RootOrganelle`, both foreground = glyph colour.
  If `Overfill > 0`, `[3, 3+floor(18 × Overfill/MaxHP))` background `Overfill`.
* Otherwise, `Upgradable` with a `CurrentPath`: `cutoff = floor(18 × Progress/AmountRequired)`;
  for `CALCIUM` bar `Calcium` on `RestingTank`, for `ELECTRONICS` bar `Electronics` on
  `ReticleBackground`, text `TextHeading`. Skipped for `Chloroplast` so the production bar
  is not overwritten.
* Nucleus navigation hints at column 21 (`Width-1`), only when ≥ 2 nuclei exist, printed in
  `SuperBright` on `Slime`: `@` for the active one, `A` for the previous in the list
  (wrapping), `D` for the next.
* If `GetLoggable().Count > 55`, `Q▲` at `(0, 58)` and `E▼` at `(20, 58)` in `TextHeading`
  on `Slime`.

### 9.7 Examine mode, organelle mode, post-mortem, F1

* **Examine mode** — `Game.ExamineCursor` is a `Cursor` entity added to the map (so it
  participates in the `Content` index). It starts at `ActivePlayer`'s cell (from live mode)
  or at `OrganelleLog.Highlighted`'s cell (from organelle mode). Movement is clamped to
  `[0, Width) × [0, Height)`. It renders last (drawn on top of everything) as a blinking
  magenta `X`, `ALWAYS_VISIBLE`. Description shown in the info console comes from
  `Cursor.Under()`. Moving the cursor never costs a turn.
* **Organelle mode** — info console shows the selected organelle's description; the player
  console shows the `>` selection marker. Arrow keys scroll the selection.
* **Post-mortem** — `PostMortem : Actor` with `Delay 0`. On win, `CommandSystem.Win()`
  clears the schedule and `AddActor(new PostMortem())`. On loss, `Nucleus.HandleGameOver()`
  does the same after logging the score. Because `Delay == 0` it is always the next thing
  the scheduler returns, and `AdvanceTurn` sets `ActivePlayer = null`, which routes all
  input to `UserInputMeta`. There is no dedicated screen — the map stays on-screen and the
  final messages sit in the log. `NiceTurnBuffer` prevents the reset `_time = 0` from
  blanking the turn counter.
* **F1 help** (`Game.WriteF1Instructions`, also called once at game start) pushes these
  messages into the log, filling all 9 lines:
  ```
  Arrow keys: Move / Select
  Space: Wait
  X: Toggle examine mode
  Z: Toggle organelle browsing mode
  ESC: Back to player mode
  A, D: Cycle active nucleus
  Destroy {NumCities - GraceCities} cities to win
  Consult the "README" file to review these instructions and more. F1 to show these
    messages again.                            (wraps to 2 lines at 62 columns)
  ```

---

## 10. Controls

`Game.HandleUserInput` → `AcceptUserInput`, which dispatches in this priority order:
**examine** (`ExamineCursor != null`) → **organelle** (`Showing == ORGANELLE`) →
**live** (`ActivePlayer != null`) → **meta**. A handler returning `true` ends the player's
turn. `keyPress` may be `null` (no key this frame); every handler no-ops on null.

### Live mode

| Key | Action | Ends turn? |
|---|---|---|
| `↑` `↓` `←` `→` | `AttackMovePlayer(ActivePlayer, dir)` | only if the action succeeded |
| `Space`, `.`, keypad `.`, keypad `5` | `Wait()` | **yes, always** |
| `A` | `NextNucleus(-1)` | no |
| `D` | `NextNucleus(+1)` | no |
| `Z` | enter organelle mode | no |
| `X` | enter examine mode, cursor at active nucleus | no |
| `Q` | `OrganelleLog.Page(-1)` | no |
| `E` | `OrganelleLog.Page(+1)` | no |
| `Esc` | no-op (only fires if `Showing == ORGANELLE`, unreachable here) | no |
| `F1` | print the help block to the message log | no |

### Organelle mode

| Key | Action |
|---|---|
| `↑` or `←` | `OrganelleLog.Scroll(-1)` |
| `↓` or `→` | `OrganelleLog.Scroll(+1)` |
| `Q` / `E` | `Page(-1)` / `Page(+1)` |
| `Z` | back to message mode |
| `Esc` | back to message mode |
| `X` | enter examine mode with the cursor on the highlighted organelle |
| `F1` | help |

Nothing in organelle mode ends the turn.

### Examine mode

| Key | Action |
|---|---|
| `↑` `↓` `←` `→` | move the cursor one cell (clamped to the map) |
| `X` | remove cursor, back to message mode |
| `Z` | remove cursor, go to organelle mode |
| `Esc` | remove cursor, back to message mode |
| `F1` | help |

Nothing in examine mode ends the turn.

### Meta mode (after win or loss, `ActivePlayer == null`)

| Key | Action |
|---|---|
| `Esc` | `Graphics.End()` — close the window and exit |
| `R` | `Program.PlayAgain = true`, then `Graphics.End()` — restart with the same difficulty |
| any other key | consumed (returns `true`), no effect |

---

## 11. Quirks & bugs worth knowing

Reproduce these deliberately if you want behaviour parity; fix them if you want a better
game. File/line references are to the C# as read.

1. **RogueSharp RNG is inclusive at both ends.** `Rand.Next(3)` yields `0..3`. Getting
   this wrong silently changes every probability in the game.
   (`Systems/MapGenerator.cs`, `Core/Enemies/City.cs:88-92`, `Chloroplast.cs:228-231`.)
2. **AI "sight" ignores walls.** `Actor.Seen(Func<Actor,bool>)`
   (`Core/Actor.cs:146-157`) scans `GetCellsInDiamond` without any FOV test, so Militia,
   Hunters, Tanks, Maws and Tentacles all detect targets through solid rock. This was a
   deliberate perf trade (commit `81bfdca`) and the `lightWalls` parameter is dead.
3. **`RandomElbowTunnel` is not random and may not connect.** Both coin-flip branches carve
   the same two segments (`MapGenerator.cs:232-244`), and both segments are anchored at
   `from`, so if the shortest inter-pocket bridge is diagonal the tunnel reaches
   `(to.X, from.Y)` and `(from.X, to.Y)` but never `(to.X, to.Y)`. The algorithm then
   *assumes* the pockets merged, so an unreachable region can survive generation.
4. **Boulders are one cell bigger than requested** in each dimension, because RogueSharp's
   `Rectangle.Right == X + Width` and `AddBoulder` iterates `Left..Right` inclusive
   (`MapGenerator.cs:339-348`).
5. **`Bioreactor` is not "twice as fast".** `Bioreactor()` sets `Delay = 10;` and then
   immediately `Delay = 16;` (`Chloroplast.cs:94-95`). Same for `BiometalForge`
   (`45` then `16`, lines 170-171) and `PrimordialSoup` (`60` then `16`, lines 213-214).
   Every member of the line produces on a 16-turn cycle; the only mechanical difference
   between Chloroplast and Bioreactor is the initial `NextFood` (20 vs 16). The
   descriptions still claim the old numbers.
6. **`Extractor.NeedsToReach` filter is a no-op.** `MapGenerator`-style typo at
   `Chloroplast.cs:333-337`: the inner `Min` lambda uses `w` (the outer variable) instead of
   its own parameter `x`, so the predicate compares each candidate's distance to itself and
   is always true. Every non-adjacent dissolving corpse is a candidate, not just the nearest.
7. **`ImmediateUphillStep` accumulates non-optimal candidates.** `Core/Actor.cs:206-228`:
   both `safestSacrifices` and `safestFreeSpaces` use `>=` and never clear the list when a
   strictly better value is found, so worse options remain selectable. Affects Caravan
   fleeing and Tentacle tank-avoidance.
8. **Terror Core `Terrified` list is never cleared by `DoPostSchedule`.**
   (`Nucleus.cs:276-302`.) It is cleared only at the start of `DoPostAttackMove`. If the
   player waits (or the move fails) after terrifying somebody, the same actors are
   `SchedulingSystem.Add`ed a **second** time on the next pop, giving them duplicate
   schedule entries (they act twice; `Remove` only deletes one). Re-terrifying an actor
   that is currently off-schedule logs `"{a.Name} is already terrified."` — the code itself
   comments `// This is a bug`.
9. **`ForceField` description contradicts the code.** The text says it protects range-1
   allies "except [from] tanks and mechs"; `CheckAndSave` (`CommandSystem.cs:184-190`)
   actually applies the adjacency check to **all** attackers including Tanks, and the
   range-3 check to all **non-Tank** attackers, not just Militia.
10. **Membranes bypass `CheckAndSave` entirely** (`CommandSystem.cs:132`), so a membrane
    standing next to a Phase Membrane is never rescued by it — it resolves its own
    kill-or-die rule first.
11. **Crafting materials tick 16× per turn** (`Delay = 1` on `Calcium`/`Electronics`), which
    means 16 scheduler round-trips and 16 adjacency scans per material per turn. With a
    large mass this dominates the frame budget.
12. **`Cytoplasm` (and plain `Membrane`, `Butcher`, `ForceField`, …) are scheduled but
    inert** — they fall into `AdvanceTurn`'s final `else` and simply re-queue themselves.
    A 200-cell mass means 200 pointless scheduler operations per turn.
13. **`Nucleus.ColorMovingSlime` runs a full-mass BFS on every `SetAsActiveNucleus`**, i.e.
    on every player turn *and* on every `A`/`D` keypress. `MoveOrganelle` runs the identical
    BFS again on every move. Both are O(mass × |Actors|) because of the
    `DMap.Actors.Where(...)` linear scan inside the inner loop.
14. **Highlighted path ≠ dragged path.** The highlight is the union of all maximum-depth
    BFS paths; the drag picks one at random. With ties the visual over-promises.
15. **Unupgraded-nucleus cap is a string comparison.** `PrimordialSoup.Produce` counts
    `PlayerMass.Count(x => x.Name == "Nucleus") > 4` (`Chloroplast.cs:228`), so upgraded
    cores do not count toward the cap. Commit `7df9f39`.
16. **`SchedulingSystem.Clear()` resets `_time` to 0** on win/loss, which is why the UI
    keeps `OrganelleLog.NiceTurnBuffer` as a monotone maximum.
17. **`RemoveActor` does not null out `Entity.Map`** (deliberately commented out at
    `DungeonMap.cs:203`), because `BecomeItem`/`BecomeActor`/`OnDestroy` are invoked after
    removal and still need the map handle.
18. **`Upgradable.Components()` over-refunds.** It charges the full `AmountRequired` for the
    in-progress path regardless of `Progress`, so unsliming a half-upgraded organelle drops
    more dust than went in (`Upgradable.cs:38-51`).
19. **`Actor.BecomeItems` passes a `seenPerimeter` buffer that is never populated**, so
    `NearestLootDrops` copies the whole `seen` list on every ring
    (`DungeonMap.cs:344-372`, `Actor.cs:88-111`). Correct but quadratic. Commit `4a891c0`
    fixed an earlier, worse version of this ("preventing some major lag spikes").
20. **`MapGenerator.PlaceCity` has no attempt cap** (unlike `PlaceLoot`'s 2048), so a map
    with no single-doorway wall cells would hang.
21. **`PlaceLoot` silently discards items** after 2048 failed attempts, so the actual item
    counts can be lower than the nominal 32/5/8/8 on cramped maps.
22. **Four palette entries are dead** (`InactiveGravityCore`, `InactiveQuantumCore`,
    `TerrorCoreActive`, `SmartCoreInactive`) — leftovers from an earlier recolour
    (commit `93f8e67`).
23. **`Game.UserInputMeta` returns `true` for any key**, including keys with no binding, so
    the post-mortem screen "consumes turns" harmlessly.
24. **Wishlist notes still-open issues** (`Wishlist.md`): page-up/page-down are broken, the
    Quantum Core's in/out speed bonuses stack, terror is "weird", the Smart Core should only
    work in your own cytoplasm, and cultivators need a working/not-working indicator.

---

## 12. Port guidance (C# → Rust)

**What RogueSharp actually provides, and what you must replace.**

| RogueSharp feature used | Where | Rust suggestion |
|---|---|---|
| `Map` cell grid with `IsWalkable` / `IsTransparent` / `IsExplored` | everywhere | A flat `Vec<Cell>` indexed `y * width + x`. Do **not** subclass; make `DungeonMap` own the grid. |
| `Map.ComputeFov` / `AppendFov` (Bresenham line casting, **taxicab** radius, `lightWalls = true`) | `DungeonMap.UpdatePlayerFieldOfView` | Reimplement literally, or use a shadowcasting crate and accept a slightly different (circular) shape. If you want bit-exact parity, port the line-cast loop — the diamond radius is load-bearing for gameplay. |
| `Map.GetCellsInDiamond(x, y, d)` | `Actor.Seen`, `CheckAndSave`, `Maw.Act` | trivial `|dx| + |dy| <= d` iterator |
| `Map.GetCellsInRows` / `GetCellsInColumns` / `GetAllCells` | `MapGenerator.Arena` | trivial |
| `PathFinder.ShortestPath` (4-way Dijkstra, throws `PathNotFoundException`) | all NPC/organelle AI | `pathfinding` crate's `bfs`/`dijkstra`, returning `Option<Vec<Coord>>` instead of exceptions |
| `Path` (`Length` counts cells **including** the start; `StepForward()` throws `NoMoreStepsException` at the end) | Militia, Hunter, Maw, Extractor, GravityCore | Return `Vec<Coord>`; remember `steps == Length - 1` when porting `picked.Length <= Range + 1` |
| `Rectangle.Intersects`, `Left/Right/Top/Bottom` with `Right = X + Width` | `MapGenerator.PlaceBoulders` | Write it out explicitly; do not assume exclusive bounds |
| `IRandom` / `DotNetRandom` with **inclusive** `Next` | everywhere | Wrap `rand::Rng` in an `inclusive(min, max)` helper so the ported call sites read identically |
| `Point` | Hunter firing, `CommandSystem` | Use the game's own `Coord` (already a plain 2-int struct with `TaxiDistance`, `+`, `-`, `*`) |

**Subsystems that map poorly.**

1. **Class-hierarchy dispatch.** The whole game is `is`-checks against a deep inheritance
   tree (`Militia → Tank → Mech`, `Membrane → Maw → ReinforcedMaw`, `Nucleus → EyeCore →
   LaserCore`), and semantics depend on **subtype** relationships: `monster is Tank` is true
   for Mech *and* Caravan; `victim is ReinforcedMembrane` is true for ForceField *and*
   Phase Membrane. A flat Rust enum will get this wrong. Recommended: give each kind an id
   plus explicit predicate tables/bitflags — e.g. `is_tank_family`, `is_militia_family`,
   `is_reinforced_membrane`, `armor`, `kills_unarmored_attackers` — and rewrite each `is`
   check against those flags. Keep a comment mapping each flag back to the C# type it stood
   for.
2. **Constructor-time `Init()` virtual dispatch.** `NPC`/`DissolvingNPC` call the virtual
   `Init()` from their base constructor, so subclass overrides run before subclass field
   initializers. In Rust, use a data table of stat blocks keyed by kind, which is clearer
   and sidesteps the ordering subtleties (e.g. `CapturedMech` sets its fields in a
   *constructor* rather than an `Init` override, `Tank.cs:94-102`).
3. **Aliased mutable graph.** `DungeonMap.PlayerMass` is the *same list object* as
   `OrganelleLog.Tracking`; entities hold a back-pointer `Entity.Map`; `Attack` recurses;
   `CraftingMaterial.Act` removes itself mid-iteration; `Upgrade` replaces an actor while
   the scheduler holds a reference to it. Borrow-checker hostile. Recommended: an
   `Arena<Actor>` / slotmap with `ActorId` handles, all systems taking `&mut World`, and
   *no* references stored inside actors. Deferred-removal queues will simplify
   `AdvanceTurn`'s `if (DMap.Actors.Contains(nextUp))` guard.
4. **`ISchedulable.Time` reads mutable `Delay`.** Several places temporarily mutate `Delay`
   around an `Add` call (`SetAsActiveNucleus`, `TerrorCore`, `QuantumCore`). Keep `Delay`
   as plain data on the actor and pass the effective cost explicitly to `schedule.add(id,
   cost)` so the temporary-mutation dance disappears.
5. **Exceptions as control flow.** `PathNotFoundException`, `NoMoreStepsException`,
   `InvalidOperationException` (in `TryFluidSelect`) are all caught and used as ordinary
   branches. Use `Option`/`Result` and keep the log messages
   ("The {Name} contemplates the irrationality of its existence.", "{Name} wimpers sadly",
   "A dissolving human was pulled into the space it was already in!") on the `None` arms.
6. **RLNET rendering.** Replace with `crossterm`/`ratatui` (drop the bitmap font) or
   `bracket-lib`/`bevy_ascii_terminal` (keeps a 12×12 CP437 font and the code-page glyphs
   16/17/30/31 used by the Hunter arrows and the `Q▲`/`E▼` markers). Note the render model
   is "rebuild every tile every frame", which is cheap enough to keep verbatim. Colours must
   be converted to normalized f32 before applying the `×1.2`/`×0.75` multipliers, then
   clamped, to match `Palette`.
7. **Input polling.** The C# polls the keyboard every RLNET update tick and calls
   `HandleUserInput(null)` when nothing is pressed; the whole NPC phase runs inside one such
   tick. In Rust, prefer an explicit state machine: `loop { if player_turn { block on input }
   else { advance_turn() } }`, redrawing at ≥ 4 Hz for the animations.
8. **Hot spots to fix while porting** (from §11 items 11-13, 19): give `DungeonMap` a real
   `HashMap<Coord, SmallVec<ActorId>>` or grid-of-slots index so `GetActorAt` is O(1);
   skip scheduling inert organelles entirely (or give them a huge `Delay`); cache the slime
   BFS between `ColorMovingSlime` and `MoveOrganelle` within a turn; and batch the crafting
   material tick to once per turn instead of 16 (a visible balance change — gate it).
