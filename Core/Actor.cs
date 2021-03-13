using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using RLNET;
using RogueSharp;
using AmoebaRL.Interfaces;
using AmoebaRL.UI;

namespace AmoebaRL.Core
{
    public class Actor : IActor, IDrawable, ISchedulable
    {
        // IActor
        public string Name { get; set; }

        public int Awareness { get; set; }

        public int Speed { 
            get; 
            set; } 
            = 1;

        public int Slime { get; set; }

        public bool Unforgettable { get; set; } = false;

        // IDrawable
        public RLColor Color { get; set; }

        public char Symbol { get; set; }

        public int X { get; set; }

        public int Y { get; set; }

        public void Draw(RLConsole console, IMap map)
        {
            // Don't draw actors in cells that haven't been explored
            if (!map.GetCell(X, Y).IsExplored)
            {
                return;
            }
            
            // Only draw the actor with the color and symbol when they are in field-of-view
            if (map.IsInFov(X, Y) || Unforgettable)
            {
                // TODO invert these slime colors???
                if(Slime == 1)
                    console.Set(X, Y, Color, Palette.BodySlime, Symbol);
                else if(Slime == 2)
                    console.Set(X, Y, Color, Palette.PathSlime, Symbol);
                else
                    console.Set(X, Y, Color, Palette.FloorBackgroundFov, Symbol);
            }
            else
            {
                // When not in field-of-view just draw a normal floor
                console.Set(X, Y, Palette.Floor, Palette.FloorBackground, '.');
            }
        }

        // ISchedulable
        public int Time
        {
            get
            {
                return Speed;
            }
        }

        #region Helpers
        public FieldOfView FOV()
        {
            // When replacing static handles, Instances of Game.DMap
            // should instead be replaced by object references to this class.
            FieldOfView selfFOV = new FieldOfView(Game.DMap);
            selfFOV.ComputeFov(X, Y, Awareness, true);
            return selfFOV;
        }

        // public List<Actor> Seen() => Seen(Game.PlayerMass);

        public List<Actor> Seen(List<Actor> testSightTo)
        {
            List<Actor> seenTargets = new List<Actor>();
            FieldOfView selfFOV = FOV();
            foreach (Actor a in testSightTo)
            {
                if (selfFOV.IsInFov(a.X, a.Y))
                    seenTargets.Add(a);
            }
            return seenTargets;
        }

        /// <summary>
        /// Take a single step to be as far away as possible from sources of terror.
        /// </summary>
        /// <param name="terrorizers">Things to be afraid of.</param>
        /// <returns>Whether the source of terror was escaped.</returns>
        public ICell MinimizeTerrorStep(IEnumerable<Actor> terrorizers, bool canTakeSacrifices)
        {
            int mySafety = 0;
            foreach (Actor t in terrorizers)
                mySafety += DungeonMap.TaxiDistance(t, this);


            // Find the safest sacrifice.
            int safestSacrificeVal = -1;
            List<Actor> safestSacrifices = new List<Actor>();
            List<Actor> sacrifices = null;
            if (canTakeSacrifices)
            {
                sacrifices = Game.DMap.AdjacentActors(X, Y).Where(a => !terrorizers.Contains(a) && !(a is City)).ToList();
                foreach (Actor s in sacrifices)
                {
                    int safety = 0;
                    foreach (Actor t in terrorizers)
                        safety += DungeonMap.TaxiDistance(t, s);
                    if (safety >= safestSacrificeVal)
                    {
                        safestSacrificeVal = safety;
                        safestSacrifices.Add(s);
                    }
                }
            }

            // Find the safest place to walk to.
            List<ICell> freeSpaces = Game.DMap.AdjacentWalkable(X, Y);
            List<ICell> safestFreeSpaces = new List<ICell>();
            int safestFreeSpaceVal = 0;
            foreach (ICell s in freeSpaces)
            {
                int safety = 0;
                foreach (Actor t in terrorizers)
                    safety += DungeonMap.TaxiDistance(Game.DMap.GetCell(t.X, t.Y), s);
                if (safety >= safestFreeSpaceVal)
                {
                    safestFreeSpaceVal = safety;
                    safestFreeSpaces.Add(s);
                }
            }

            // TODO Make this method a part of the "Actor" class.


            // If waiting is the safest option, return false.
            if (mySafety >= safestSacrificeVal && mySafety >= safestFreeSpaceVal)
                return Game.DMap.GetCell(X, Y);

            // Otherwise, move to the safest spot and return true.
            bool takeSacrifice = safestSacrificeVal > safestFreeSpaceVal;
            if (safestFreeSpaceVal == safestSacrificeVal)
            {
                takeSacrifice = Game.Rand.Next(1) == 0;
            }
            if (takeSacrifice)
            {
                Actor picked = safestSacrifices[Game.Rand.Next(safestSacrifices.Count - 1)];
                ICell targ = Game.DMap.GetCell(picked.X, picked.Y);
                return targ; // Game.CommandSystem.AttackMoveOrganelle(this, targ.X, targ.Y);
            }
            else
            {
                ICell targ = safestFreeSpaces[Game.Rand.Next(safestFreeSpaces.Count - 1)];
                return targ; // Game.CommandSystem.AttackMoveOrganelle(this, targ.X, targ.Y);
            }
        }

        public bool PathExists(Func<Actor, bool> ignoreIf, int x, int y)
        {
            return PathIgnoring(ignoreIf, x, y) != null;
        }

        public Path PathIgnoring(Func<Actor,bool> ignoreIf, int x, int y)
        {
            IEnumerable<Actor> ignore = Game.DMap.Actors.Where(ignoreIf);
            List<bool> wasAlreadyIgnored = new List<bool>();
            foreach (Actor toIgnore in ignore)
            {
                wasAlreadyIgnored.Add(Game.DMap.IsWalkable(toIgnore.X, toIgnore.Y));
                Game.DMap.SetIsWalkable(toIgnore.X, toIgnore.Y, true);
            }

            Path found = null;
            try
            {
                //found = f.ShortestPath(
                //    Game.DMap.GetCell(X, Y),
                //    Game.DMap.GetCell(x, y)
                //);
                found = DungeonMap.QuickShortestPath(Game.DMap, Game.DMap.GetCell(X, Y), Game.DMap.GetCell(x, y));
            }
            catch (PathNotFoundException)
            {
                
            }

            IEnumerator<bool> alreadyIgnored = wasAlreadyIgnored.GetEnumerator();
            foreach (Actor toIgnore in ignore)
            {
                alreadyIgnored.MoveNext();
                Game.DMap.SetIsWalkable(toIgnore.X, toIgnore.Y, alreadyIgnored.Current);
            }


            return found;

        }

        public List<Path> PathsTo(List<Actor> potentialTargets)
        {
            List<Path> results = new List<Path>();
            Path attempt;
            foreach (Actor candidate in potentialTargets)
            {
                attempt = null;
                try
                {
                    attempt = DungeonMap.QuickShortestPath(Game.DMap,
                    Game.DMap.GetCell(X, Y),
                    Game.DMap.GetCell(candidate.X, candidate.Y));
                }
                catch (PathNotFoundException) { }
                if (attempt != null)
                    results.Add(attempt);
            }
            return results;
        }

        protected static bool IgnoreNone(Actor a) => true;

        public List<Path> PathsToNearest(List<Actor> potentialTargets) => PathsToNearest(potentialTargets, IgnoreNone);

        public List<Path> PathsToNearest(List<Actor> potentialTargets, Func<Actor, bool> ignoring)
        {
            List<Path> nearestPaths = new List<Path>();
            Path attempt;
            int nearestTargetDistance = int.MaxValue;
            foreach (Actor candidate in potentialTargets)
            {
                attempt = null;
                try
                {
                    attempt = PathIgnoring(ignoring, candidate.X, candidate.Y);
                    /*attempt = DungeonMap.QuickShortestPath(Game.DMap,
                    Game.DMap.GetCell(X, Y),
                    Game.DMap.GetCell(candidate.X, candidate.Y));*/
                }
                catch (PathNotFoundException) { }
                if (attempt != null)
                {
                    if (attempt.Length <= nearestTargetDistance)
                    {
                        if (attempt.Length < nearestTargetDistance)
                        {
                            nearestPaths.Clear();
                            nearestTargetDistance = attempt.Length;
                        }
                        nearestPaths.Add(attempt);
                    }
                }
            }
            return nearestPaths;
        }

        public bool AdjacentTo(int tx, int ty)
        {
            if (Math.Abs(X - tx) + Math.Abs(Y - ty) == 1)
                return true;
            return false;
        }
        #endregion
    }
}
