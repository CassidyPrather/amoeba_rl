using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using RLNET;
using RogueSharp;
using AmoebaRL.Interfaces;
using AmoebaRL.UI;
using AmoebaRL.Core.Enemies;

namespace AmoebaRL.Core
{
    /// <summary>
    /// Autonomous <see cref="Entity"/> which has a proactive impact on the map.
    /// </summary>
    public class Actor : Entity, IActor, ISchedulable
    {
        // IActor
        /// <summary>Flavor only, displayed in text log contexts.</summary>
        public string Name { get; set; } = "Actor";

        /// <summary>
        /// Maximum range field of view can be calculated at for this actor.
        /// <list type="bullet">
        ///     <item> 0 : Can see self </item>
        ///     <item> Below 0 : Cannot see anything </item>
        /// </list>
        /// </summary>
        public int Awareness { get; set; } = 0;

        /// <summary>
        /// The time between turns this actor will take a turn when it appears in <see cref="Systems.SchedulingSystem"/>
        /// </summary>
        public int Delay { get; set; } = 16;

        /// <summary>
        /// Visual indicator for the background.
        /// <list type="bullet">
        ///     <item> 0 : Default floor </item>
        ///     <item> 1 : Dark green </item>
        ///     <item> 2 : Dark green </item>
        /// </list>
        /// </summary>
        /// <remarks>May later be used as a shorthand for faction.</remarks>
        public int Slime { get; set; } = 0;

        // ISchedulable
        /// </inheritdoc>
        public int Time
        {
            get
            {
                return Delay;
            }
        }

        #region Helpers
        /// <summary>
        /// Transforms this actor into a single <see cref="Item"/> and places it in the nearest available spot on the map.
        /// <seealso cref="DungeonMap.NearestLootDrop(int, int)"/>
        /// </summary>
        /// <param name="i">The <see cref="Item"/> to transform into.</param>
        public void BecomeItem(Item i)
        {
            ICell lands = Map.NearestLootDrop(X, Y);
            i.X = lands.X;
            i.Y = lands.Y;
            Map.AddItem(i);
        }

        /// <summary>
        /// Transforms this actor into a set of <see cref="Item"/>s and places it in the nearest available spots on the map.
        /// <seealso cref="DungeonMap.NearestLootDrop(int, int)"/>
        /// </summary>
        /// <param name="items">The <see cref="Item"/>s to transform into.</param>
        public void BecomeItems(IEnumerable<Item> items)
        {
            List<ICell> alreadyTriedDrop = new List<ICell>();
            List<ICell> alreadyTriedDropPerimeter = new List<ICell>();
            List<ICell> nextAvailable = new List<ICell>();
            foreach (Item i in items)
            {
                if (nextAvailable.Count == 0)
                    nextAvailable = Map.NearestLootDrops(X, Y, alreadyTriedDrop, alreadyTriedDropPerimeter);
                if (nextAvailable.Count > 0)
                {
                    int picker = Map.Context.Rand.Next(nextAvailable.Count - 1);
                    ICell lands = nextAvailable[picker];
                    nextAvailable.RemoveAt(picker);
                    i.X = lands.X;
                    i.Y = lands.Y;
                    Map.AddItem(i);
                }
                else
                {
                    Map.Context.MessageLog.Add($"The {i.Name} had nowhere to drop, and is crushed!");
                }
            }
        }

        /// <summary>
        /// Replace this <see cref="Actor"/> with another.
        /// </summary>
        /// <param name="a">The replacement</param>
        /// <returns><paramref name="a"/></returns>
        public virtual Actor BecomeActor(Actor a)
        {
            a.X = X;
            a.Y = Y;
            Map.AddActor(a);
            return a;
        }

        /// <summary>
        /// Performs a field of view calculation originating from <see cref="X"/>, <see cref="Y"/> with a range <see cref="Awareness"/>.
        /// </summary>
        /// <param name="lightWalls">Whether to include the first non-transparent cell hit in each FOV trace in the output.</param>
        /// <returns>Result of calculation.</returns>
        public FieldOfView FOV(bool lightWalls = true)
        {
            // When replacing static handles, Instances of Map.Context.DMap
            // should instead be replaced by object references to this class.
            FieldOfView selfFOV = new FieldOfView(Map.Context.DMap);
            selfFOV.ComputeFov(X, Y, Awareness, lightWalls);
            return selfFOV;
        }

        /// <summary>
        /// Determines which of <paramref name="testSightTo"/> are in <see cref="FOV(bool)"/>.
        /// </summary>
        /// <param name="testSightTo">The possible set of actors which might be in <see cref="FOV(bool)"/> to include in the output.</param>
        /// <param name="lightWalls">Whether to include the first non-transparent actor hit in each FOV trace in the output.</param>
        /// <returns>All of the <see cref="Actor"/>s which are in both <see cref="FOV(bool)"/> and <paramref name="testSightTo"/>.</returns>
        public List<Actor> Seen(List<Actor> testSightTo, bool lightWalls = true)
        {
            List<Actor> seenTargets = new List<Actor>();
            FieldOfView selfFOV = FOV(lightWalls);
            foreach (Actor a in testSightTo)
            {
                if (selfFOV.IsInFov(a.X, a.Y))
                    seenTargets.Add(a);
            }
            return seenTargets;
        }

        /// <summary>
        /// Take a single step to be as far away as possible from <paramref name="sources"/>.
        /// This does not use a safety map implementation; this is deliberately a flawed algorithm in this sense;
        /// compute a separate djikstra map for this.
        /// </summary>
        /// <param name="sources">Things to move away from.</param>
        /// <param name="canPassThroughOthers">Whether this step can include spaces occupied by <see cref="NPC"/>s or <see cref="Organelles.Organelle"/>s.</param>
        /// <returns>The location moved to; can be own cell if no further location exists.</returns>
        public ICell ImmediateUphillStep(IEnumerable<Actor> sources, bool canPassThroughOthers)
        {
            int mySafety = 0;
            foreach (Actor t in sources)
                mySafety += Position.TaxiDistance(t.Position);


            // Find the safest sacrifice.
            int safestSacrificeVal = -1;
            List<Actor> safestSacrifices = new List<Actor>();
            List<Actor> sacrifices = null;
            if (canPassThroughOthers)
            {
                sacrifices = Map.AdjacentActors(X, Y).Where(a => !sources.Contains(a) && !(a is City)).ToList();
                foreach (Actor s in sacrifices)
                {
                    int safety = 0;
                    foreach (Actor t in sources)
                        safety += t.Position.TaxiDistance(s.Position);
                    if (safety >= safestSacrificeVal)
                    {
                        safestSacrificeVal = safety;
                        safestSacrifices.Add(s);
                    }
                }
            }

            // Find the safest place to walk to.
            List<ICell> freeSpaces = Map.AdjacentWalkable(X, Y);
            List<ICell> safestFreeSpaces = new List<ICell>();
            int safestFreeSpaceVal = 0;
            foreach (ICell s in freeSpaces)
            {
                int safety = 0;
                foreach (Actor t in sources)
                    safety += DungeonMap.TaxiDistance(Map.GetCell(t.X, t.Y), s);
                if (safety >= safestFreeSpaceVal)
                {
                    safestFreeSpaceVal = safety;
                    safestFreeSpaces.Add(s);
                }
            }

            // If waiting is the safest option, return false.
            if (mySafety >= safestSacrificeVal && mySafety >= safestFreeSpaceVal)
                return Map.GetCell(X, Y);

            // Otherwise, move to the safest spot and return true.
            bool takeSacrifice = safestSacrificeVal > safestFreeSpaceVal;
            if (safestFreeSpaceVal == safestSacrificeVal)
            {
                takeSacrifice = Map.Context.Rand.Next(1) == 0;
            }
            if (takeSacrifice)
            {
                Actor picked = safestSacrifices[Map.Context.Rand.Next(safestSacrifices.Count - 1)];
                ICell targ = Map.GetCell(picked.X, picked.Y);
                return targ; // Map.Context.CommandSystem.AttackMoveOrganelle(this, targ.X, targ.Y);
            }
            else
            {
                ICell targ = safestFreeSpaces[Map.Context.Rand.Next(safestFreeSpaces.Count - 1)];
                return targ; // Map.Context.CommandSystem.AttackMoveOrganelle(this, targ.X, targ.Y);
            }
        }

        /// <summary>
        /// Determines whether a path exists from this to a specified location that is not obstructed.
        /// </summary>
        /// <param name="ignoreIf">Criteria by which a cell may be considered to not be an obstruction. 
        /// <param name="x">Horizontal coordinate of target location.</param>
        /// <param name="y">Vertical coordinate of target location.</param>
        /// <returns>A path exists from this to a specified location that is not obstructed.</returns>
        public bool PathExists(Func<Actor, bool> ignoreIf, int x, int y)
        {
            return PathIgnoring(ignoreIf, x, y) != null;
        }

        /// <summary>
        /// Calculates a shortest contiguous group of adjacent cells between this space and a target location.
        /// Cannot contain obstructed locations, except that it always includes the location of this and the target location.
        /// </summary>
        /// <param name="ignoreIf">Criteria by which a cell may be considered to not be an obstruction.
        /// Empty locations or locations containing only <see cref="Item"/>s or <see cref="VFX"/>s are never obstructed by default.</param>
        /// <param name="x">Horizontal coordinate of target location.</param>
        /// <param name="y">Vertical coordinate of target location.</param>
        /// <returns>A shortest contiguous group of adjacent cells between this space and a target location that is not obstructed.
        /// Null if none exists.</returns>
        public Path PathIgnoring<T>(Func<T,bool> ignoreIf, int x, int y) where T : Entity
        {
            IEnumerable<Actor> ignore = Map.Actors.Where(x => x is T x_t && ignoreIf(x_t));
            List<bool> wasAlreadyIgnored = new List<bool>();
            foreach (Actor toIgnore in ignore)
            {
                wasAlreadyIgnored.Add(Map.IsWalkable(toIgnore.X, toIgnore.Y));
                Map.SetIsWalkable(toIgnore.X, toIgnore.Y, true);
            }

            Path found = null;
            try
            {
                //found = f.ShortestPath(
                //    Map.GetCell(X, Y),
                //    Map.GetCell(x, y)
                //);
                found = DungeonMap.QuickShortestPath(Map.Context.DMap, Map.GetCell(X, Y), Map.GetCell(x, y));
            }
            catch (PathNotFoundException)
            {
                
            }

            IEnumerator<bool> alreadyIgnored = wasAlreadyIgnored.GetEnumerator();
            foreach (Actor toIgnore in ignore)
            {
                alreadyIgnored.MoveNext();
                Map.SetIsWalkable(toIgnore.X, toIgnore.Y, alreadyIgnored.Current);
            }
            return found;
        }

        public Path PathThrough<T>(Func<T,bool> throughIf, int x, int y) where T : Entity
        {
            IEnumerable<Actor> canOnlyPathThrough = Map.Actors.Where(x => x is T x_t && throughIf(x_t));
            IEnumerable<ICell> emptyCells = Map.GetAllCells().Where(c => c.IsWalkable).ToList();
            foreach (ICell tempBlock in emptyCells)
                Map.SetIsWalkable(tempBlock.X, tempBlock.Y, false);
            List<bool> wasAlreadyIgnored = new List<bool>();
            foreach (Actor toIgnore in canOnlyPathThrough)
            {
                wasAlreadyIgnored.Add(Map.IsWalkable(toIgnore.X, toIgnore.Y));
                Map.SetIsWalkable(toIgnore.X, toIgnore.Y, true);
            }

            Path found = null;
            try
            {
                found = DungeonMap.QuickShortestPath(Map.Context.DMap, Map.GetCell(X, Y), Map.GetCell(x, y));
            }
            catch (PathNotFoundException)
            {

            }

            IEnumerator<bool> alreadyIgnored = wasAlreadyIgnored.GetEnumerator();
            foreach (Actor toIgnore in canOnlyPathThrough)
            {
                alreadyIgnored.MoveNext();
                Map.SetIsWalkable(toIgnore.X, toIgnore.Y, alreadyIgnored.Current);
            }
            foreach (ICell tempBlock in emptyCells)
                Map.SetIsWalkable(tempBlock.X, tempBlock.Y, true);
            return found;
        }

        /// <summary>
        /// Always returns false, regardless of <paramref name="discard"/>.
        /// </summary>
        /// <param name="discard">Discarded.</param>
        /// <returns><c>false</c></returns>
        protected static bool IgnoreNone<T>(T discard) => false;

        /// <summary>
        /// Finds the list of shortest <see cref="Path"/>s from this to each of <paramref name="potentialTargets"/>.
        /// This only includes <see cref="Path"/>s that could be generated without obstructions.
        /// An obstruction includes a location which is not <see cref="Map.IsWalkable(int, int)"/>.
        /// </summary>
        /// <param name="potentialTargets">The <typeparamref name="T"/>s to calculate paths to.</param>
        /// <returns>The unobstructed <see cref="Path"/>s to <paramref name="potentialTargets"/> with the minimum length.</returns>
        public List<Path> PathsToNearest<T>(IEnumerable<T> potentialTargets) where T : Entity => PathsToNearest(potentialTargets, IgnoreNone);

        /// <summary>
        /// Finds the list of shortest <see cref="Path"/>s from this to each of <paramref name="potentialTargets"/>.
        /// This only includes <see cref="Path"/>s that could be generated without obstructions.
        /// An obstruction includes a location which is not <see cref="Map.IsWalkable(int, int)"/> and does not meet <paramref name="ignoring"/>.
        /// </summary>
        /// <param name="potentialTargets">The <typeparamref name="T"/>s to calculate paths to.</param>
        /// <returns>The unobstructed <see cref="Path"/>s to <paramref name="potentialTargets"/> with the minimum length.</returns>
        public List<Path> PathsToNearest<T>(IEnumerable<T> potentialTargets, Func<T, bool> ignoring) where T : Entity
        {
            List<Path> nearestPaths = new List<Path>();
            Path attempt;
            int nearestTargetDistance = int.MaxValue;
            foreach (T candidate in potentialTargets)
            {
                attempt = null;
                try
                {
                    attempt = PathIgnoring(ignoring, candidate.X, candidate.Y);
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

        public List<Path> PathsThroughToNearest<T>(IEnumerable<T> potentialTargets, Func<T, bool> ignoring) where T : Entity
        {
            List<Path> nearestPaths = new List<Path>();
            Path attempt;
            int nearestTargetDistance = int.MaxValue;
            foreach (T candidate in potentialTargets)
            {
                attempt = null;
                try
                {
                    attempt = PathThrough(ignoring, candidate.X, candidate.Y);
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

        /// <summary>
        /// Determines whether this is exactly one unit away from a provided location.
        /// </summary>
        /// <param name="tx">The horizontal coordinate of the location to be checked for adjacency.</param>
        /// <param name="ty">The vertical coordinate of the location to be checked for adjacency.</param>
        /// <returns>(<see cref="X"/>, <see cref="Y"/>) is adjacent to (<paramref name="tx"/>, <paramref name="ty"/>)</returns>
        public bool AdjacentTo(int tx, int ty)
        {
            if (Math.Abs(X - tx) + Math.Abs(Y - ty) == 1)
                return true;
            return false;
        }
        #endregion
    }
}
