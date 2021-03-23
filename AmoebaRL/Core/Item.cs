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
    /// <summary>
    /// Something which exists on the floor, but is not autonomous and does not block primary features (e.g. <see cref="Actor"/>).
    /// Only one can exist in a location at a time.
    /// </summary>
    /// <remarks>
    /// Does not use a proper memory system yet, but this is needed so that the user cannot tell if items out of FOV were destroyed.
    /// Maybe add a "memory layer" to the map.
    /// </remarks>
    public class Item : IItem, IDrawable
    {
        // IItem
        public String Name { get; set; }

        // IDrawable
        public RLColor Color { get; set; }

        public char Symbol { get; set; }

        public int X { get; set; }

        public int Y { get; set; }

        public VisibilityCondition Visibility { get; set; } = VisibilityCondition.EXPLORED_ONLY;

        /// <inheritdoc/>
        public void Draw(RLConsole console, IMap map)
        {
            if (Visibility == VisibilityCondition.ALWAYS_VISIBLE ||
                ((Visibility == VisibilityCondition.LOS_ONLY || Visibility == VisibilityCondition.EXPLORED_ONLY) && map.IsInFov(X, Y)))
            {
                DrawSelfGraphic(console, map);
                return;
            }

            if (map.IsExplored(X, Y))
            {
                if (Visibility == VisibilityCondition.EXPLORED_ONLY)
                    DrawSelfGraphicMemory(console, map);
                else
                    console.Set(X, Y, Palette.Floor, Palette.FloorBackground, '.');
            }
        }

        /// <summary>
        /// Draws the graphical representation of this as if it was observed directly.
        /// </summary>
        /// <param name="console">Drawing canvas.</param>
        /// <param name="map">The game area the drawing is done in the context of.</param>
        protected virtual void DrawSelfGraphic(RLConsole console, IMap map)
        {
            console.Set(X, Y, Color, Palette.FloorBackgroundFov, Symbol);
        }

        /// <summary>
        /// Draws the graphical representation of this as if it was remembered, but is not directly visible.
        /// </summary>
        /// <param name="console">Drawing canvas.</param>
        /// <param name="map">The game area the drawing is done in the context of.</param>
        protected virtual void DrawSelfGraphicMemory(RLConsole console, IMap map)
        {
            console.Set(X, Y, Palette.DbStone, Palette.FloorBackground, Symbol);
        }
    }
}
