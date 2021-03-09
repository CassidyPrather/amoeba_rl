using AmoebaRL.Behaviors;
using AmoebaRL.Interfaces;
using AmoebaRL.Systems;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Core
{
    public class Monster : Actor
    {
        public int? TurnsAlerted { get; set; }

        public virtual void PerformAction(CommandSystem commandSystem)
        {
            IBehavior behavior = new StandardMoveAndAttack();
            behavior.Act(this, commandSystem);
        }
    }
}
