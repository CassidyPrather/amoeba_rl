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

namespace AmoebaRL.Core
{
    public class DungeonMap : Map
    {
        public List<Actor> Actors;
        public List<Item> Items;
        public List<Rectangle> Boulders;
        public List<IDrawable> Effects;

        
        private static readonly TimeSpan ANIMATION_RATE = TimeSpan.FromMilliseconds(250);

        public TimeSpan TimeSinceLastAnimation = TimeSpan.Zero;

        public int AnimationFrame = 0;

        public DungeonMap()
        {
            Boulders = new List<Rectangle>();
            Actors = new List<Actor>();
            Items = new List<Item>();
            Effects = new List<IDrawable>();
        }

        // The Draw method will be called each time the map is updated
        // It will render all of the symbols/colors for each cell to the map sub console
        public void Draw(RLConsole mapConsole)
        {
            mapConsole.Clear();
            foreach (Cell cell in GetAllCells())
            {
                SetConsoleSymbolForCell(mapConsole, cell);
            }
            foreach (Item i in Items)
            {
                i.Draw(mapConsole, this);
            }
            foreach (Actor a in Actors)
            {
                a.Draw(mapConsole, this);
            }
            foreach(VFX e in Effects)
            {
                e.Draw(mapConsole, this);
            }
        }

        public bool Animate(RLConsole mapConsole, TimeSpan delta)
        {
            TimeSinceLastAnimation += delta;
            if(TimeSinceLastAnimation >= ANIMATION_RATE)
            {
                AnimationFrame++;
                TimeSinceLastAnimation = TimeSpan.Zero;
                foreach (VFX e in Effects)
                    e.Draw(mapConsole, this);
                return true;
            }
            return false;
        }

        private void SetConsoleSymbolForCell(RLConsole console, Cell cell)
        {
            // When we haven't explored a cell yet, we don't want to draw anything
            if (!cell.IsExplored)
            {
                return;
            }

            // When a cell is currently in the field-of-view it should be drawn with ligher colors
            if (IsInFov(cell.X, cell.Y))
            {
                // Choose the symbol to draw based on if the cell is walkable or not
                // '.' for floor and '#' for walls
                if (cell.IsWalkable)
                {
                    console.Set(cell.X, cell.Y, Palette.FloorFov, Palette.FloorBackgroundFov, '.');
                }
                else
                {
                    console.Set(cell.X, cell.Y, Palette.WallFov, Palette.WallBackgroundFov, '#');
                }
            }
            // When a cell is outside of the field of view draw it with darker colors
            else
            {
                if (cell.IsWalkable)
                {
                    console.Set(cell.X, cell.Y, Palette.Floor, Palette.FloorBackground, '.');
                }
                else
                {
                    console.Set(cell.X, cell.Y, Palette.Wall, Palette.WallBackground, '#');
                }
            }
        }


        // This method will be called any time we move the player to update field-of-view
        public void UpdatePlayerFieldOfView()
        {
            IEnumerator<Actor> grantsVision = Game.PlayerMass.Where(a => a.Awareness >= 0).GetEnumerator();
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



        public IDrawable GetActorOrItem(int x, int y)
        {
            IDrawable firstChoice = GetActorAt(x, y);
            if (firstChoice != null)
                return firstChoice;
            IDrawable secondChoice = GetItemAt(x, y);
            return secondChoice; // may be null
        }

        public Actor GetActorAt(int x, int y) => Actors.FirstOrDefault(a => a.X == x && a.Y == y);

        public Item GetItemAt(int x, int y) => Items.FirstOrDefault(a => a.X == x && a.Y == y);

        public IDrawable GetVFX(int x, int y) => Effects.FirstOrDefault(a => a.X == x && a.Y == y);

        /// <summary>
        /// Determines whether there is an <see cref="Actor"/> or <see cref="Item"/> at the point.
        /// </summary>
        /// <param name="x"></param>
        /// <param name="y"></param>
        /// <returns></returns>
        public bool IsEmpty(int x, int y) => GetActorAt(x, y) == null && GetItemAt(x, y) == null;

        public bool IsWall(ICell w) => !w.IsWalkable && IsEmpty(w.X, w.Y);

        public bool IsWall(int x, int y) => IsWall(GetCell(x,y));

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
                if (Game.PlayerMass.Contains(actor))
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
            if ((a.Slime == true && a.Awareness != 0) || (b.Slime == true && b.Awareness != 0))
                UpdatePlayerFieldOfView();
        }

        // A helper method for setting the IsWalkable property on a Cell
        public void SetIsWalkable(int x, int y, bool isWalkable)
        {
            ICell cell = GetCell(x, y); // For some reason, this didn't work with the specific type "Cell"
            SetCellProperties(cell.X, cell.Y, cell.IsTransparent, isWalkable, cell.IsExplored);
        }

        // Called by MapGenerator after we generate a new map to add the player to the map
        public void AddPlayer(Nucleus player)
        {
            // Game.Player = player; // would like to move away from this handle altogether.
            Actors.Add(player);
            SetIsWalkable(player.X, player.Y, false);
            UpdatePlayerFieldOfView();
            Game.SchedulingSystem.Add(player);
        }

        public void AddActor(Actor toAdd)
        {
            if (!IsWalkable(toAdd.X, toAdd.Y) && !(toAdd is City) && !(toAdd is PostMortem))
                Game.MessageLog.Add($"Placed actor in an impossible location");
            Actors.Add(toAdd);
            SetIsWalkable(toAdd.X, toAdd.Y, false);
            Game.SchedulingSystem.Add(toAdd);
        }

        public void AddItem(Item toAdd) => Items.Add(toAdd);

        public void AddVFX(VFX toAdd) => Effects.Add(toAdd);


        public void RemoveActor(Actor a)
        {
            Actors.Remove(a);
            if(Game.PlayerMass.Contains(a))
            {
                Game.PlayerMass.Remove(a);
                UpdatePlayerFieldOfView();
                // It is okay for the player mass to be disjoint.
            }
            SetIsWalkable(a.X, a.Y, true);
            Game.SchedulingSystem.Remove(a);
        }
        
        public void RemoveItem(Item targetItem)
        {
            Items.Remove(targetItem);
            // Items don't make cells unwalkable so this is fine to not mess with.
        }

        public void RemoveVFX(VFX toRemove) => Effects.Remove(toRemove);

        // Helpers
        public ICell NearestLootDrop(int x, int y)
        {
            List<ICell> candidates = NearestLootDrops(x, y);
            if (candidates.Count == 0)
                return null;
            return candidates[Game.Rand.Next(0, candidates.Count - 1)];
        }

        public List<ICell> NearestLootDrops(int x, int y)
        {
            List<ICell> seen = new List<ICell>();
            List<ICell> frontier = new List<ICell>();
            List<ICell> candidates = new List<ICell>() { GetCell(x, y) };
            List<ICell> found = new List<ICell>();
            while(found.Count == 0 && candidates.Count > 0)
            {
                foreach(ICell c in candidates)
                {
                    seen.Add(c);
                    if(GetItemAt(c.X, c.Y) == null && !IsWall(c))
                        found.Add(c);
                    else
                    {
                        foreach(ICell adj in Adjacent(c.X, c.Y).Where(a => !seen.Contains(a) && !IsWall(a)))
                            frontier.Add(adj);
                    } 
                }
                candidates.Clear();
                candidates.AddRange(frontier);
                frontier.Clear();
            }
            return found;
        }


        public List<Actor> NearestActors(int x, int y, Func<Actor, bool> filterBy)
        {
            IEnumerable<Actor> Candidates = Actors.Where(filterBy);
            int shortestDistance = Candidates.Min(c => TaxiDistance(GetCell(c.X, c.Y), GetCell(x, y)));
            return Candidates.Where(c => TaxiDistance(GetCell(c.X, c.Y), GetCell(x, y)) == shortestDistance).ToList();
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

        public static int TaxiDistance(IDrawable from, IDrawable to) => Math.Abs(from.X - to.X) + Math.Abs(from.Y - to.Y);


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
