using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Interfaces
{
    /// <summary>
    /// Can be consumed into a new group.
    /// </summary>
    public interface IEatable
    {
        /// <summary>
        /// Called when another group consumes this. Often implies becoming a member of another group.
        /// </summary>
        void OnEaten();
    }
}
