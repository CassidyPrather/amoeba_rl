using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Interfaces
{
    public interface IPostAttackMove
    {
        void DoPostAttackMove();
    }

    public interface IPreMove
    {
        void DoPreMove();
    }
}
