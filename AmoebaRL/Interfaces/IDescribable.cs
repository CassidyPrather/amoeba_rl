using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Interfaces
{
    /// <summary>
    /// Can be described with words.
    /// </summary>
    public interface IDescribable : INamed
    {
        /// <summary>
        /// A string describing this <see cref="IDescribable"/> to be displayed in a viewport like <see cref="Systems.MessageLog"/>.
        /// Does not need to include <see cref="INamed.Name"/>.
        /// </summary>
        /// <remarks>May eventually be updated to a SadConsole-like formatting standard to enable colors.</remarks>
        string Description { get; }
    }
}
