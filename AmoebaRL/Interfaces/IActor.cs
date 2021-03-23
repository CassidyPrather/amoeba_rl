using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Interfaces
{
    public interface IActor : INamed
    {
        int Awareness { get; set; }

        int Slime { get; set; }

        int Delay { get; set; }
    }
}
