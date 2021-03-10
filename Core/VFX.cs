using AmoebaRL.Interfaces;
using AmoebaRL.UI;
using RLNET;
using RogueSharp;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Core
{
    public abstract class VFX : IDrawable
    {
        public bool Transparent { get; set; } = false;

        public bool AlwaysVisible { get; set; } = false;

        public virtual RLColor Color { get; set; } = Palette.Wall;

        public virtual RLColor BackgroundColor { get; set; } = Palette.Floor;

        public virtual char Symbol { get; set; } = '*';
        public int X { get; set; }
        public int Y { get; set; }

        public virtual void Draw(RLConsole console, IMap map)
        {
            // Don't draw actors in cells that haven't been explored
            if (!AlwaysVisible && !map.GetCell(X, Y).IsExplored)
            {
                return;
            }

            // Only draw the actor with the color and symbol when they are in field-of-view
            if (!Transparent && (AlwaysVisible || map.IsInFov(X, Y)))
            {
                console.Set(X, Y, Color, BackgroundColor, Symbol);
            }
            else
            {
                if(map.IsInFov(X, Y))
                { 
                    IDrawable under = Game.DMap.GetActorOrItem(X, Y);
                    if (under != null)
                        under.Draw(console, map);
                    else // When not in field-of-view just draw whatever else is ordinarily in that space.
                        console.Set(X, Y, Palette.FloorFov, Palette.FloorBackgroundFov, '.');
                }
                else
                {
                    console.Set(X, Y, Palette.Floor, Palette.FloorBackground, '.');
                }
            }
        }
    }
}
