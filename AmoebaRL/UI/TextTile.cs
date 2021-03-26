using AmoebaRL.Core;
using AmoebaRL.Interfaces;
using RLNET;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.UI
{
    /// <summary>
    /// An ASCII glyph representation of a single cell <see cref="Entity"/>.
    /// </summary>
    public class TextTile : IGraphic
    {
        /// <summary>
        /// The <see cref="Entity"/> this is an <see cref="IGraphic"/> for.
        /// </summary>
        public Entity Represents { get; set; }

        /// <summary>
        /// The conditions under which this should be drawn.
        /// </summary>
        /// <remarks>
        /// May eventually use a predicate for this instead of an enum to allow finer-grain control over the visibility condition.
        /// </remarks>
        public VisibilityCondition Visibility { get; set; } = VisibilityCondition.LOS_ONLY;

        /// <summary>
        /// The color of the glyph.
        /// </summary>
        public virtual RLColor Color { get; set; } = Palette.Wall;

        /// <summary>
        /// The color of the background behind the glyph.
        /// </summary>
        public virtual RLColor BackgroundColor { get; set; } = Palette.Floor;

        /// <summary>
        /// The glyph of the representation.
        /// </summary>
        public virtual char Symbol { get; set; } = '*';

        /// <summary>
        /// Show this glyph on the console in the position corresponding to this actor's position on the map.
        /// The responsibility for offsetting this- for example, through map scrolling, is the responsibility of <paramref name="console"/>.
        /// </summary>
        /// <param name="console">The canvas to draw on.</param>
        public virtual void Draw(RLConsole console)
        {
            // Don't draw actors in cells that haven't been explored
            if (!(Visibility == VisibilityCondition.ALWAYS_VISIBLE || Visibility == VisibilityCondition.EXPLORED_ONLY) && !Represents.IsExplored())
            {
                return;
            }

            // Only draw the actor with the color and symbol when they are in field-of-view
            if (Visibility == VisibilityCondition.ALWAYS_VISIBLE || Represents.IsInFov())
            {
                console.Set(Represents.X, Represents.Y, Color, BackgroundColor, Symbol);
            }
            else if (Visibility == VisibilityCondition.EXPLORED_ONLY && Represents.IsExplored())
            {
                console.Set(Represents.X, Represents.Y, Palette.DbStone, BackgroundColor, Symbol);
            }
            
            // Don't draw invisible things.
            /*
            if (Represents.IsInFov())
            {
                IDrawable under = Represents.Map.GetActorOrItem(Represents.X, Represents.Y);
                if (under != null)
                    under.Draw(console);
                else // When not in field-of-view just draw whatever else is ordinarily in that space.
                    console.Set(Represents.X, Represents.Y, Palette.FloorFov, Palette.FloorBackgroundFov, '.');
            }
            else
            {
                console.Set(Represents.X, Represents.Y, Palette.Floor, Palette.FloorBackground, '.');
            }
            */
        }
    }
}
