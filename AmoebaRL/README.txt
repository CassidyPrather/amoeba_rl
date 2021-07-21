# Amoeba Roguelike

Play as a giant, constantly evolving amoeba and fight off intensifying waves of humans trying to protect their cities. Craft new organelles and cores to respond to escalating threats. Can you destroy all 16 city gates and escape to the surface?

Post 7DRL patch (v3.0.0). Fixes performance, UI, and balance issues.

# Installation

This game depends on [.NET Desktop Runtime 5.0.*](https://dotnet.microsoft.com/download/dotnet/5.0/runtime). Be sure to install it on your system before running it.

## Important Note for Linux Users

Make sure to install [libgdplus](https://www.mono-project.com/docs/gui/libgdiplus/) with your package manager of choice, in addition to the .NET framework, to run the game.

# How to Play

## Controls

Arrow keys: Move
Space: Wait
A, D: Go to previous/next nucleus
Z: Organelle mode toggle
    Arrow keys (Organelle mode): Select organelle
X: Examine mode toggle
    Arrow keys (Examine mode): Move examine cursor
Destroy all cities to win.

## Tips

Stuck? Try reviewing the following information:

* When you move (not swap), you drag a path of organelles behind you. The highlighted slime shows which tiles will be dragged! This can be used to position organelles strategically and quickly.
* To learn what something is, e[x]amine it.
* Pressing space to pass a turn or luring enemies can break an otherwise impenetrable formation.
* In the early game, it is easy to find new base organelles but hard to find crafting materials. This inverts as time goes on.
* Find the right balance between combat, exploration, and organelle management for your play-style; all of these cost time and come with different risks and rewards.

Want an easier gamemode? Launch the program with the command line argument `--easy` for a tuned-down difficulty level.

Want an extra challenge instead? Launch the program with the command line argument `--gj` to activate GJ mode.

For those unfamiliar, to launch a program with a command line argument, just add the argument to the end of the line where you would launch the program from the terminal. For example, `AmoebaRL.exe --easy` to launch easy mode. On windows, this can be automated with shortcuts: Right click the executable, select "create shortcut", then right click the shortcut and add the argument to the end of the "Target" field, separated by a space.

# Credits

Extensive playtesting, design, and support from JackNine
Extensive playtesting and bug reports from GJ
Further playtesting and reports from Qu, Kyzrati, and Decinym
Engine: https://github.com/FaronBracy/RogueSharp
Font: https://github.com/libtcod/libtcod/blob/develop/data/fonts/terminal12x12_gs_ro.png
.NET 5.0 by Microsoft
