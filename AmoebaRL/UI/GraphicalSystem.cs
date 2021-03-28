using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.UI
{
    /// <summary>
    /// Using the logical state of a <see cref="Game"/>, presents information to the user.
    /// </summary>
    public abstract class GraphicalSystem
    {
        /// <summary>
        /// The <see cref="Game"/> represented by this instance.
        /// </summary>
        protected Game Showing { get; private set; }

        /// <summary>
        /// Create a graphical representation of <paramref name="toShow"/>.
        /// </summary>
        /// <param name="toShow">The game to present a representation of.</param>
        public GraphicalSystem(Game toShow)
        {
            Showing = toShow;
        }

        /// <summary>
        /// Begin the UI process, thereby starting <see cref="Showing"/>.
        /// </summary>
        public abstract void Run();

        /// <summary>
        /// Ends the graphical process.
        /// </summary>
        public abstract void End();
    }
}
