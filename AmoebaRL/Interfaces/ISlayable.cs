using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Interfaces
{
    /// <summary>
    /// Can be effectively removed from the map and its <see cref="Systems.SchedulingSystem"/>, potentially with side effects.
    /// </summary>
    public interface ISlayable
    {
        /// <summary>Remove from its context and enact relevant side effects.</summary>
        void Die();
    }
}
