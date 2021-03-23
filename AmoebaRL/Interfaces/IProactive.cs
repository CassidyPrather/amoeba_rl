using AmoebaRL.Core;
using AmoebaRL.Systems;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Interfaces
{
    /// <summary>
    /// Something that has a behavior to perform when it is called from <see cref="SchedulingSystem"/>
    /// </summary>
    public interface IProactive
    {
        /// <summary>
        /// Performed when called from <see cref="SchedulingSystem"/>.
        /// </summary>
        void Act();
    }
}
