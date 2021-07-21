using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using RLNET;
using RogueSharp;
using AmoebaRL.UI;
using AmoebaRL.Interfaces;
using AmoebaRL.Core.Organelles;
using AmoebaRL.Core.Enemies;

namespace AmoebaRL.Core
{
    /// <summary>
    /// 
    /// </summary>
    /// <remarks>
    /// The RogueSharp paradigm for maps is ultimately too limiting for this game and eventually needs to be moved away from.
    /// But for now, it is sufficient.
    /// Requires optimization for things like GetXAt() and Add/Remove.
    /// </remarks>
    public class DungeonMap : Map
    {
        public Game Context { get; protected set; }

        // TODO: Replace List<Entity> with a special struct.
        protected List<List<List<Entity>>> Content { get; set; } // indexed with x,y


        #region Indices
        public List<Entity> All;
        public List<Actor> Actors;
        public List<Actor> PlayerMass;
        public List<Item> Items;
        public List<City> Cities;
        public List<Entity> Effects;
        #endregion

        // Would love to do away with this:
        public List<Rectangle> Boulders;

        public DungeonMap(Game context)
        {
            Context = context;
            All = new List<Entity>();
            Boulders = new List<Rectangle>();
            Actors = new List<Actor>();
            PlayerMass = new List<Actor>();
            Items = new List<Item>();
            Effects = new List<Entity>();
            Cities = new List<City>();
        }

        public void InitalizeContent()
        {
            Content = new List<List<List<Entity>>>();
            for (int col = 0; col < Width; col++)
            {
                Content.Add(new List<List<Entity>>());
                for (int row = 0; row < Height; row++)
                {
                    Content[col].Add(new List<Entity>());
                }
            }
        }

        // This method will be called any time we move the player to update field-of-view
        public void UpdatePlayerFieldOfView()
        {
            IEnumerator<Actor> grantsVision = PlayerMass.Where(a => a.Awareness >= 0).GetEnumerator();
            bool hasNext = grantsVision.MoveNext();
            if (hasNext)
            {
                ComputeFov(grantsVision.Current.X, grantsVision.Current.Y, grantsVision.Current.Awareness, true);
                // Compute the field-of-view based on the player's location and awareness
                while(grantsVision.MoveNext())
                {
                    AppendFov(grantsVision.Current.X, grantsVision.Current.Y, grantsVision.Current.Awareness, true);
                }
            }
            else
                ComputeFov(0, 0, 0, false);// Player is blind (shouldn't happen?)
            // Mark all cells in field-of-view as having been explored
            foreach (Cell cell in GetAllCells())
            {
                if (IsInFov(cell.X, cell.Y))
                {
                    SetCellProperties(cell.X, cell.Y, cell.IsTransparent, cell.IsWalkable, true);
                }
            }
        }

        #region Accessors
        /// <summary>
        /// Move an entity on the map, updating <see cref="GetEntity(int, int)"/>.
        /// </summary>
        /// <param name="toMove">The <see cref="Entity.Positions"/> value to change the position of.</param>
        /// <param name="dest">The new <see cref="Entity.Positions"/> value.</param>
        /// <remarks>
        /// If <see cref="Entity.Positions"/> is modified without calling <see cref="Move(Entity, Coord)"/> for each modification,
        /// both <see cref="GetEntity(int, int)"/> and <see cref="RemoveEntity(Entity)"/> will no longer work.
        /// </remarks>
        public void Move(Entity toMove, Coord dest) => Move(toMove, new List<Coord>() { dest });

        /// <summary>
        /// Move an entity on the map, updating <see cref="GetEntity(int, int)"/>.
        /// </summary>
        /// <param name="toMove">The <see cref="Entity.Positions"/> value to change the position of.</param>
        /// <param name="dest">The new <see cref="Entity.Positions"/> value.</param>
        /// /// <remarks>
        /// If <see cref="Entity.Positions"/> is modified without calling <see cref="Move(Entity, Coord)"/> for each modification,
        /// both <see cref="GetEntity(int, int)"/> and <see cref="RemoveEntity(Entity)"/> will no longer work.
        /// </remarks>
        public void Move(Entity toMove, IEnumerable<Coord> dests)
        {
            foreach (Coord old in toMove.Positions)
                Content[old.X][old.Y].Remove(toMove);
            foreach (Coord c in dests)
                Content[c.X][c.Y].Add(toMove);
        }

        public Entity GetEntity(int x, int y) => GetEntities(x, y).FirstOrDefault();

        public List<Entity> GetEntities(int x, int y) => Content[x][y];

        public Actor GetActorAt(int x, int y) => Actors.FirstOrDefault(a => a.X == x && a.Y == y);

        public Item GetItemAt(int x, int y) => Items.FirstOrDefault(a => a.X == x && a.Y == y);

        public Entity GetVFX(int x, int y) => Effects.FirstOrDefault(a => a.X == x && a.Y == y);

        public Entity GetActorOrItem(int x, int y)
        {
            Entity firstChoice = GetActorAt(x, y);
            if (firstChoice != null)
                return firstChoice;
            Entity secondChoice = GetItemAt(x, y);
            return secondChoice; // may be null
        }

        public void AddEntity(Entity toAdd)
        {
            All.Add(toAdd);
            Content[toAdd.X][toAdd.Y].Add(toAdd);
            toAdd.Map = this;
        }

        public void AddActor(Actor toAdd)
        {
            AddEntity(toAdd);
            Actors.Add(toAdd);
            SetIsWalkable(toAdd.X, toAdd.Y, false);
            Context.SchedulingSystem.Add(toAdd);
            if (toAdd is Organelle)
            {
                UpdatePlayerFieldOfView();
                foreach (ICell adj in Adjacent(toAdd.X, toAdd.Y))
                {
                    Actor mightEngulf = GetActorAt(adj.X, adj.Y);
                    if (mightEngulf is NPC n)
                        n.Engulf();
                }
            }
        }

        public void AddItem(Item toAdd)
        {
            AddEntity(toAdd);
            Items.Add(toAdd);
        }

        public void AddVFX(Entity toAdd)
        {
            AddEntity(toAdd);
            Effects.Add(toAdd);
        }

        public void AddCity(City toAdd)
        {
            Cities.Add(toAdd);
            AddActor(toAdd);
        }

        // Called by MapGenerator after we generate a new map to add the player to the map
        public void AddPlayer(Nucleus player)
        {
            // Game.Player = player; // would like to move away from this handle altogether.
            Actors.Add(player);
            SetIsWalkable(player.X, player.Y, false);
            UpdatePlayerFieldOfView();
            Context.SchedulingSystem.Add(player);
        }

        public void RemoveEntity(Entity toRemove)
        {
            All.Remove(toRemove);
            Content[toRemove.X][toRemove.Y].Remove(toRemove);
            //toRemove.Map = null;
        }

        public void RemoveActor(Actor a)
        {
            RemoveEntity(a);
            Actors.Remove(a);
            if(PlayerMass.Contains(a))
            {
                PlayerMass.Remove(a);
                UpdatePlayerFieldOfView();
                // It is okay for the player mass to be disjoint.
            }
            SetIsWalkable(a.X, a.Y, true);
            Context.SchedulingSystem.Remove(a);
        }
        
        public void RemoveItem(Item targetItem)
        {
            RemoveEntity(targetItem);
            Items.Remove(targetItem);
            // Items don't make cells unwalkable so this is fine to not mess with.
        }

        public void RemoveVFX(Entity toRemove)
        {
            RemoveEntity(toRemove);
            Effects.Remove(toRemove);
        }

        public void RemoveCity(City c)
        {
            RemoveEntity(c);
            RemoveActor(c);
            SetCellProperties(c.X, c.Y, true, true, true);
            Cities.Remove(c);
            UpdatePlayerFieldOfView();
        }

        #endregion

        /// <summary>
        /// Determines whether there is an <see cref="Actor"/> or <see cref="Item"/> at the point.
        /// </summary>
        /// <param name="x"></param>
        /// <param name="y"></param>
        /// <returns></returns>
        public bool IsEmpty(int x, int y) => GetActorAt(x, y) == null && GetItemAt(x, y) == null;

        public bool IsWall(ICell w) => !w.IsWalkable && IsEmpty(w.X, w.Y);

        public bool IsWall(int x, int y) => IsWall(GetCell(x, y));

        public bool WithinBounds(int x, int y) => x >= 0 && y >= 0 && x < Width && y < Height;

        // Returns true when able to place the Actor on the cell or false otherwise
        public bool SetActorPosition(Actor actor, int x, int y)
        {
            // Only allow actor placement if the cell is walkable
            if (GetCell(x, y).IsWalkable)
            {
                // The cell the actor was previously on is now walkable
                SetIsWalkable(actor.X, actor.Y, true);
                // Update the actor's position
                actor.X = x;
                actor.Y = y;
                // The new cell the actor is on is now not walkable
                SetIsWalkable(actor.X, actor.Y, false);
                // Don't forget to update the field of view if we just repositioned the player
                if (PlayerMass.Contains(actor))
                {
                    UpdatePlayerFieldOfView();
                }
                return true;
            }
            return false;
        }

        public void Swap(Actor a, Actor b)
        {
            Point buffer = new Point(a.X, a.Y);
            a.X = b.X;
            a.Y = b.Y;
            b.X = buffer.X;
            b.Y = buffer.Y;
            if ((a.Slime > 0 && a.Awareness != 0) || (b.Slime > 0 && b.Awareness != 0))
                UpdatePlayerFieldOfView();
        }

        // A helper method for setting the IsWalkable property on a Cell
        public void SetIsWalkable(int x, int y, bool isWalkable)
        {
            ICell cell = GetCell(x, y); // For some reason, this didn't work with the specific type "Cell"
            SetCellProperties(cell.X, cell.Y, cell.IsTransparent, isWalkable, cell.IsExplored);
        }

        private bool NotUnderPlayer(ICell lootSpot)
        {
            Actor actAt = GetActorAt(lootSpot.X, lootSpot.Y);
            if (actAt == null)
                return true;
            return !PlayerMass.Contains(actAt);
        }

        private bool NotThroughWalls(ICell candidate) => !IsWall(candidate);

        public ICell NearestLootDrop(int x, int y) => NearestLootDrop(x, y, NotUnderPlayer, NotThroughWalls);

        public List<ICell> NearestLootDrops(int x, int y) => NearestLootDrops(x, y, NotUnderPlayer, NotThroughWalls);

        // Helpers
        public ICell NearestLootDrop(int x, int y, Func<ICell, bool> legalLootDrop, Func<ICell, bool> legalLootPath)
        {
            List<ICell> candidates = NearestLootDrops(x, y, legalLootDrop, legalLootPath);
            if (candidates.Count == 0)
                return null;
              return candidates[Context.Rand.Next(0, candidates.Count - 1)];
        }

        public List<ICell> NearestLootDrops(int x, int y, List<ICell> seen, List<ICell> seenPerimeter) => NearestLootDrops(x, y, NotUnderPlayer, NotThroughWalls, seen, seenPerimeter);

        /// <summary>
        /// Finds the set of nearest locations which an <see cref="Item"/> can appear in to an origin.
        /// </summary>
        /// <param name="x">The x coordinate of the point to try to be nearest to.</param>
        /// <param name="y">The y coordinate of the point to try to be nearest to.</param>
        /// <param name="legalLootDrop">The rule a tile must meet for an item to be able to appear on it. Doesn't need to include "an item doesn't already exist".</param>
        /// <param name="legalLootPath">The rule a tile must meet to be included in pathfinding from the cell at <paramref name="x"/>, <paramref name="y"/>.</param>
        /// <param name="seen">A buffer of tiles to exclude from tile drops and to start searching from, generated and modified by calls to this function.</param>
        /// <param name="seenPerimeter">The tiles on the outermost ring of <paramref name="seen"/>, used to improve performance in repeated calls. Generated and modified by calls to this function.</param>
        /// <returns>All of the tiles on which an item can legally appear.</returns>
        /// <remarks>Should eventually replace this with the efficient floodfill library. Should ultimately be delegated to a separate class which can store this state internally and query a "next" method.</remarks>
        public List<ICell> NearestLootDrops(int x, int y, Func<ICell, bool> legalLootDrop, Func<ICell, bool> legalLootPath, List<ICell> seen = null, List<ICell> seenPerimeter = null)
        {
            // Initalize the temporary collections used in this algorithm.
            List<ICell> candidates = new(); // All unseen cells adjacent to seen perimeter.
            List<ICell> found = new(); // The next set of nearest loot drop locations.
            if(seen == null)
                seen = new List<ICell>();
            if(seen.Count == 0)
            {
                ICell startingCell = GetCell(x, y);
                candidates.Add(startingCell);
                seen.Add(startingCell);
            }
            else
            {
                // Determine all of the cells adjacent to the seen ones
                // and store it in candidates.
                List<ICell> perim;
                if (seenPerimeter == null || seenPerimeter.Count == 0)
                    perim = new(seen);
                else
                    perim = seenPerimeter;
                foreach (ICell onPerimeter in perim)
                    foreach (ICell adj in Adjacent(onPerimeter.X, onPerimeter.Y).Where(a => !seen.Contains(a) && legalLootPath(a)))
                    {
                        seen.Add(adj);
                        candidates.Add(adj);
                    }
            }
            // Evaluate the candidates and return the set of valid ones.
            // If none are valid, re-evaluate the list of candidates on the next-outer ring.
            List<ICell> nextCandidateRing = new();
            do
            {
                
                foreach (ICell c in candidates)
                {
                    if (GetItemAt(c.X, c.Y) == null && !IsWall(c) && !Cities.Contains(GetActorAt(c.X, c.Y)) && legalLootDrop(c))
                        found.Add(c);
                    else
                    {
                        foreach (ICell adj in Adjacent(c.X, c.Y).Where(a => !seen.Contains(a) && legalLootPath(a)))
                        {
                            seen.Add(adj);
                            nextCandidateRing.Add(adj);
                        }
                    }
                }
                candidates.Clear();
                candidates.AddRange(nextCandidateRing);
                nextCandidateRing.Clear();
            } while (found.Count == 0 && candidates.Count > 0);
            return found;
        }

        /// <summary>
        /// Finds all of the <see cref="Actor"/>s in <see cref="Actors"/>, meeting <paramref name="filterBy"/>,
        /// that are the minimum distance from (<paramref name="x"/>, <paramref name="y"/>).
        /// </summary>
        /// <param name="x">The horizontal coordinate to test for closeness to.</param>
        /// <param name="y">The vertical coordinate to test for closeness to.</param>
        /// <param name="filterBy">Determines whether to include an actor in the search.</param>
        /// <returns>The <see cref="Actor"/>s in <see cref="Actors"/>, meeting <paramref name="filterBy"/>,
        /// that are the minimum distance from (<paramref name="x"/>, <paramref name="y"/>).</returns>
        public List<Actor> NearestActors(int x, int y, Func<Actor, bool> filterBy)
        {
            // GJ complained about lag in this implementation:
            /* IEnumerable<Actor> Candidates = Actors.Where(filterBy);
            if (Candidates.Count() == 0)
                return Candidates.ToList();
            int shortestDistance = Candidates.Min(c => TaxiDistance(GetCell(c.X, c.Y), GetCell(x, y)));
            return Candidates.Where(c => TaxiDistance(GetCell(c.X, c.Y), GetCell(x, y)) == shortestDistance).ToList();*/
            List<ICell> seen = new() { GetCell(x, y) };
            List<ICell> search = new() { GetCell(x, y) };
            List<Actor> found = new();
            while(search.Count > 0)
            {
                List<ICell> nextSearch = new();
                
                foreach(ICell candidate in search)
                {
                    Actor here = GetActorAt(candidate.X, candidate.Y);
                    if (filterBy(here))
                        found.Add(here);
                    else if(found.Count == 0)
                    {
                        foreach(ICell adj in Adjacent(candidate.X, candidate.Y))
                        {
                            if(!seen.Contains(adj))
                            {
                                seen.Add(adj);
                                nextSearch.Add(adj);
                            }
                        }
                    }
                }
                if (found.Count > 0)
                    return found;
                search = nextSearch;
            }
            return found;
        }

        public List<ICell> NearestNoActor(int x, int y)
        {
            // implementation wise this is very similar to nearestLootDrops so try to merge the functionalities maybe.
            List<ICell> seen = new List<ICell>();
            List<ICell> frontier = new List<ICell>();
            List<ICell> candidates = new List<ICell>() { GetCell(x, y) };
            List<ICell> found = new List<ICell>();
            while (found.Count == 0 && candidates.Count > 0)
            {
                foreach (ICell c in candidates)
                {
                    seen.Add(c);
                    if (GetActorAt(c.X, c.Y) == null && !IsWall(c))
                        found.Add(c);
                    else
                    {
                        foreach (ICell adj in Adjacent(c.X, c.Y).Where(a => !seen.Contains(a) && !IsWall(a)))
                            frontier.Add(adj);
                    }
                }
                candidates.Clear();
                candidates.AddRange(frontier);
                frontier.Clear();
            }
            return found;
        }

        public List<ICell> AdjacentWalkable(ICell from) => AdjacentWalkable(from.X, from.Y);

        public List<ICell> AdjacentWalkable(int X, int Y)
        {
            List<ICell> adj = new List<ICell>();

            if (X > 0)
                AddIfWalkable(adj, X - 1, Y);
            if (X < Width - 1)
                AddIfWalkable(adj, X + 1, Y);
            if (Y > 0)
                AddIfWalkable(adj, X, Y - 1);
            if (Y < Height - 1)
                AddIfWalkable(adj, X, Y + 1);

            return adj;
        }

        public List<ICell> Adjacent(int X, int Y)
        {
            List<ICell> adj = new List<ICell>();

            if (X > 0)
                adj.Add(GetCell(X - 1, Y));
            if (X < Width - 1)
                adj.Add(GetCell(X + 1, Y));
            if (Y > 0)
                adj.Add(GetCell(X, Y - 1));
            if (Y < Height - 1)
                adj.Add(GetCell(X, Y + 1));

            return adj;
        }


        private void AddIfWalkable(ICollection<ICell> addTo, int x, int y)
        {
            ICell candidate = GetCell(x, y);
            if (candidate.IsWalkable)
                addTo.Add(candidate);
        }

        public List<Actor> AdjacentActors(int x, int y)
        {
            List<Actor> adj = new List<Actor>();

            if (x > 0)
                AddActorIfNotNull(adj, x - 1, y);
            if (x < Width - 1)
                AddActorIfNotNull(adj, x + 1, y);
            if (y > 0)
                AddActorIfNotNull(adj, x, y - 1);
            if (y < Height - 1)
                AddActorIfNotNull(adj, x, y + 1);

            return adj;
        }

        private void AddActorIfNotNull(ICollection<Actor> addTo, int x, int y)
        {
            Actor candidate = GetActorAt(x, y);
            if (candidate != null)
                addTo.Add(candidate);
        }

        public static int TaxiDistance(ICell from, ICell to) => Math.Abs(from.X - to.X) + Math.Abs(from.Y - to.Y);

        public static int TaxiDistance(Entity from, Entity to) => Math.Abs(from.X - to.X) + Math.Abs(from.Y - to.Y);


        public static Path QuickShortestPath(DungeonMap m, ICell from, ICell to)
        {
            PathNotFoundException innerException = null;
            Tuple<bool, bool> oldWalkable = new Tuple<bool, bool>(m.IsWalkable(from.X, from.Y), m.IsWalkable(to.X, to.Y));
            m.SetIsWalkable(from.X, from.Y, true);
            m.SetIsWalkable(to.X, to.Y, true);
             
            PathFinder f = new PathFinder(m);
            Path found = null;
            try
            {
                found = f.ShortestPath(from, to);
            }
            catch (PathNotFoundException e)
            {
                innerException = e;
            }
            m.SetIsWalkable(from.X, from.Y, oldWalkable.Item1);
            m.SetIsWalkable(to.X, to.Y, oldWalkable.Item2);
            if (innerException != null)
                throw innerException;
            return found;
        }
    }
}
