using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using RLNET;
using RogueSharp;
using AmoebaRL.UI;
using AmoebaRL.Interfaces;

namespace AmoebaRL.Core
{
    public class DungeonMap : Map
    {
        public List<Actor> Actors;
        public List<Item> Items;
        public List<Rectangle> Boulders;

        public DungeonMap()
        {
            Boulders = new List<Rectangle>();
            Actors = new List<Actor>();
            Items = new List<Item>();
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
            Nucleus player = Game.Player;
            ComputeFov(player.X, player.Y, player.Awareness, true);
            // Compute the field-of-view based on the player's location and awareness
            foreach (Actor a in Game.PlayerMass.Where(a => a != player))
            {
                AppendFov(a.X, a.Y, a.Awareness, true);
            }

            // Mark all cells in field-of-view as having been explored
            foreach (Cell cell in GetAllCells())
            {
                if (IsInFov(cell.X, cell.Y))
                {
                    SetCellProperties(cell.X, cell.Y, cell.IsTransparent, cell.IsWalkable, true);
                }
            }
        }

        public Actor GetActorAt(int x, int y)
        {
            return Actors.FirstOrDefault(a => a.X == x && a.Y == y);
        }
        public Item GetItemAt(int x, int y)
        {
            return Items.FirstOrDefault(a => a.X == x && a.Y == y);
        }

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
            Game.Player = player;
            SetIsWalkable(player.X, player.Y, false);
            UpdatePlayerFieldOfView();
        }

        public void AddActor(Actor toAdd)
        {
            Actors.Add(toAdd);
            SetIsWalkable(toAdd.X, toAdd.Y, false);
        }

        public void AddItem(Item toAdd)
        {
            Items.Add(toAdd);
        }


        public void RemoveActor(Actor a)
        {
            Actors.Remove(a);
            SetIsWalkable(a.X, a.Y, true);
        }
        public void RemoveItem(Item targetItem)
        {
            Items.Remove(targetItem);
            // Items don't make cells unwalkable so this is fine anyway.
        }


        // Helpers
        public static int TaxiDistance(ICell from, ICell to) => Math.Abs(from.X - to.X) + Math.Abs(from.Y - to.Y);

    }
}
