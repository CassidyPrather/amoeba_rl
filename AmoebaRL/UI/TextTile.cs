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
        /// The number of differnet frames the animation has.
        /// </summary>
        public int Frames { get; protected set; }

        /// <summary>
        /// The number of <see cref="ASCIIGraphics.ANIMATION_RATE"/>s to pass before updating the animation.
        /// Higher numbers = slower transitions.
        /// </summary>
        public int Speed { get; protected set; }

        /// <summary>
        /// The <see cref="Entity"/> this is an <see cref="IGraphic"/> for.
        /// </summary>
        public virtual Entity Represents { get; set; }

        /// <summary>
        /// Attempts <see cref="TextTile.Draw(RLConsole)"/> if 
        /// <see cref="TextTile.Visibility"/> evaluates such that this is not visible.
        /// </summary>
        public TextTile Backup { get; protected set; }

        /// <summary>
        /// The conditions under which this should be drawn.
        /// </summary>
        /// <remarks>
        /// May eventually use a predicate for this instead of an enum to allow finer-grain control over the visibility condition.
        /// </remarks>
        public virtual VisibilityCondition Visibility { get; set; } = VisibilityCondition.LOS_ONLY;

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
        /// Generate a <see cref="TextTile"/> to show <see cref="Represents"/>.
        /// </summary>
        /// <param name="represents">The <see cref="Entity"/> this <see cref="TextTile"/> <see cref="Represents"/>.</param>
        public TextTile(Entity represents)
        {
            Represents = represents;
        }

        /// <summary>
        /// Generate a <see cref="TextTile"/> to show <see cref="Represents"/>.
        /// </summary>
        /// <param name="represents">The <see cref="Entity"/> this <see cref="TextTile"/> <see cref="Represents"/>.</param>
        /// <param name="symbol">The glyph which shows <paramref name="represents"/>.</param>
        /// <param name="color">The foreground color which shows <paramref name="represents"/>.</param>
        /// <param name="background">The background color which shows <paramref name="represents"/>.</param>
        /// <param name="visibility">The condition under which <paramref name="represents"/> should be drawn. When this is not met, no drawing action occurs.</param>
        public TextTile(Entity represents, char symbol, RLColor color, RLColor background, VisibilityCondition visibility)
        {
            Represents = represents;
            Symbol = symbol;
            Color = color;
            BackgroundColor = background;
            Visibility = visibility;
        }

        /// <summary>
        /// Show this glyph on the console in the position corresponding to this actor's position on the map.
        /// The responsibility for offsetting this- for example, through map scrolling, is the responsibility of <paramref name="console"/>.
        /// </summary>
        /// <param name="console">The canvas to draw on.</param>
        /// <param name="AnimationFrame">The number of frames which have passed since any <see cref="TextTile"/> was drawn.</param>
        public virtual void Animate(RLConsole console, int AnimationFrame)
        {
            if (Speed != 0)
                SetFrame((AnimationFrame / Speed) % Frames);
            Draw(console);
        }

        /// <summary>
        /// Show this glyph on the console in the position corresponding to this actor's position on the map.
        /// The responsibility for offsetting this- for example, through map scrolling, is the responsibility of <paramref name="console"/>.
        /// Consider calling <see cref="Animate(RLConsole, int)"/> instead if animations are desired.
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
            if (Visibility == VisibilityCondition.ALWAYS_VISIBLE || (Represents.IsInFov() && Visibility != VisibilityCondition.INVISIBLE))
                console.Set(Represents.X, Represents.Y, Color, BackgroundColor, Symbol);
            else if (Visibility == VisibilityCondition.EXPLORED_ONLY && (Represents.IsExplored() && Visibility != VisibilityCondition.INVISIBLE))
                console.Set(Represents.X, Represents.Y, Palette.DbStone, BackgroundColor, Symbol);
            else if (Backup != null) // Don't draw invisible things. Instead draw their backups.
                Backup.Draw(console);
            else if(Represents.IsInFov()) // If there's absolutely nothing to draw, pretend the space is empty:
                console.Set(Represents.X, Represents.Y, Palette.FloorFov, Palette.FloorBackgroundFov, '.');
            else if (Represents.IsExplored())
                console.Set(Represents.X, Represents.Y, Palette.Floor, Palette.FloorBackground, '.');
        }

        public virtual void SetFrame(int idx) { }
    }
}
