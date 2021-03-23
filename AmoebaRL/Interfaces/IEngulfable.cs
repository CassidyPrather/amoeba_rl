using AmoebaRL.Core;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Interfaces
{
    /// <summary>
    /// Has a behavior to enact upon being a part of a set of actors considered
    /// </summary>
    public interface IEngulfable
    {
        /// <summary>
        /// Checks <see cref="CanEngulf(HashSet{IEngulfable})"/>, and if it is part of a engulfable group,
        /// and if so, calls <see cref="ProcessEngulf"/> on each <see cref="IEngulfable"/> in the set.
        /// </summary>
        /// <returns>True if the target was engulfed, false otherwise.</returns>
        bool Engulf();

        /// <summary>
        /// Determines whether this <see cref="IEngulfable"/> is part of a valid group of other <see cref="IEngulfable"/>.
        /// </summary>
        /// <param name="engulfing">All of the <see cref="IEngulfable"/> which have previously been 
        /// determined to be part of an engulfable mass, excluding the result of this call. 
        /// If null, it is generated automatically.</param>
        /// <returns>This individual <see cref="IEngulfable"/> is either part of an engulfable mass or would be if all adjacent <see cref="IEngulfable"/>s are.</returns>
        bool CanEngulf(HashSet<IEngulfable> inEngulfMass = null);

        /// <summary>The action to take if this is found to be a part of an <see cref="IEngulfable"/> mass.</summary>
        void ProcessEngulf();
    }
}
