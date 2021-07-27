using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using AmoebaRL.Core.Organelles;
using AmoebaRL.Interfaces;
using AmoebaRL.Systems;
using AmoebaRL.UI;
using RogueSharp;

namespace AmoebaRL.Core.Enemies
{
    public class Militia : NPC
    {
        public override void Init()
        {
            Awareness = 3;
            Delay = 16;
            Name = "Militia";
        }

        public override string DescBody => "A meager human who took up arms to defend its pitiful life. Nothing special about it. " +
                "Like all humans, it always tries to attack the nearest organelle. " +
                "Also like all humans, it can only see up to 3 cells away has no memory of anything it can't see.";

        public override List<Item> BecomesOnDie => new List<Item>() { new Nutrient() };

        public override Actor BecomesOnEaten => new CapturedMilitia();

        public override void Act()
        {
            if(!Engulf())
            {
                //List<Actor> seenTargets = Seen(Map.PlayerMass);
                List<Actor> seenTargets = Seen(act => act.IsPlayerAligned());
                if (seenTargets.Count > 0)
                    ActToTargets(seenTargets);
                else
                    Wander();
            }
        }

        public virtual void ActToTargets(List<Actor> seenTargets)
        {
            List<Path> actionPaths = PathsToNearest(seenTargets);
            if (actionPaths.Count > 0)
            {
                int pick = Map.Context.Rand.Next(0, actionPaths.Count - 1);
                try
                {
                    //Formerly: path.Steps.First()
                    Map.Context.CommandSystem.AttackMove(this, actionPaths[pick].StepForward());
                }
                catch (NoMoreStepsException)
                {
                    Map.Context.MessageLog.Add($"The {Name} contemplates the irrationality of its existence.");
                }
            } // else, wait a turn.
        }

        public virtual void Wander()
        {
            List<ICell> adj = Map.AdjacentWalkable(X, Y);
            int pick = Map.Context.Rand.Next(0, adj.Count);
            if (pick != adj.Count)
                Map.Context.CommandSystem.AttackMove(this, adj[pick]);
        }

        /*public override void PerformAction(CommandSystem commandSystem)
        {
            IBehavior behavior = new MilitiaPatrolAttack();//StandardMoveAndAttack(); //
            behavior.Act(this, commandSystem);
        }*/

        public class CapturedMilitia : DissolvingNPC
        {

            public override void Init()
            {
                Name = "Dissolving Militia";
                MaxHP = 8;
                HP = MaxHP;
                Awareness = 0;
                Slime = 1;
                Delay = 16;
            }

            public override string Flavor => "Probably regrets its mediocrity.";

            public override Actor DigestsTo => new Cytoplasm();

            public override Actor RescuesTo => new Militia();
        }
    }
}
