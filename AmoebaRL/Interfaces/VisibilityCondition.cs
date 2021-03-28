using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using RLNET;
using RogueSharp;

namespace AmoebaRL.Interfaces
{
    public enum VisibilityCondition
    {
        INVISIBLE,
        LOS_ONLY,
        EXPLORED_ONLY,
        ALWAYS_VISIBLE
    }
}
