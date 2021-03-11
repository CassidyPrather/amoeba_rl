using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Interfaces
{
    public interface IPostMove
    {
        void DoPostMove();
    }

    public interface IPreMove
    {
        void DoPreMove();
    }
}
