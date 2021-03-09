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

        public int Speed { get; set; } = 1;

        public bool Slime { get; set; }

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
            if (map.IsInFov(X, Y))
            {
                if(Slime)
                    console.Set(X, Y, Color, Palette.Slime, Symbol);
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

        /// <summary>
        /// Handy?
        /// </summary>
        public bool AdjacentTo(int tx, int ty)
        {
            if (Math.Abs(X - tx) + Math.Abs(Y - ty) == 1)
                return true;
            return false;
        }
    }
}
