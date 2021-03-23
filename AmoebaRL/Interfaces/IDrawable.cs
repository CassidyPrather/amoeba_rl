using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using RLNET;
using RogueSharp;

namespace AmoebaRL.Interfaces
{
    /// <summary>
    /// Something which can be drawn. Should extend IOnMap (has x,y) or something, and be made much more sophisticated.
    /// </summary>
    /// <remarks>
    /// TODO needs a default implementation for draw because it is repeated too often.
    /// </remarks>
    public interface IDrawable
    {

        RLColor Color { get; set; }

        char Symbol { get; set; }

        int X { get; set; }

        int Y { get; set; }

        VisibilityCondition Visibility { get; set; }

        /// <summary>
        /// Draws the graphical representation of this <see cref="IDrawable"/>.
        /// </summary>
        /// <param name="console">Drawing canvas.</param>
        /// <param name="map">The game area the drawing is done in the context of.</param>
        void Draw(RLConsole console, IMap map);
    }

    public enum VisibilityCondition
    {
        INVISIBLE,
        LOS_ONLY,
        EXPLORED_ONLY,
        ALWAYS_VISIBLE
    }
}
