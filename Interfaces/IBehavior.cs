using AmoebaRL.Core;
using AmoebaRL.Systems;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Interfaces
{
    public interface IBehavior
    {
        bool Act(TutorialMonster monster, CommandSystem commandSystem);
    }
}
