using AmoebaRL.Core;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Interfaces
{
    public interface IEngulfable
    {
        bool Engulf();

        bool CanEngulf(HashSet<IEngulfable> inEngulfMass = null);

        void ProcessEngulf();
    }
}
