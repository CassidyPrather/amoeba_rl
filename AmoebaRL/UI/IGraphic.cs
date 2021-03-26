using RLNET;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.UI
{
    /// <summary>
    /// Something which can be shown to the user.
    /// </summary>
    public interface IGraphic
    {
        /// <summary>
        /// Called when the visual representation needs to be rendered to the user.
        /// </summary>
        /// <param name="console">The canvas upon which to present the representation.</param>
        void Draw(RLConsole console);
    }
}
