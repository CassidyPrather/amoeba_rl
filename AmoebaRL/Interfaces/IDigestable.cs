using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Interfaces
{
    /// <summary>
    /// Something which can be processed when part of a capable group.
    /// </summary>
    public interface IDigestable
    {
        /// <summary>The number of turns until this is processed.</summary>
        int HP { get; set; }

        /// <summary>The initial and maximum value of <see cref="HP"/>.</summary>
        int MaxHP { get; set; }

        /// <summary>The progress towards producing an additional product upon reaching <see cref="MaxHP"/>.</summary>
        int Overfill { get; set; }
    }
}
