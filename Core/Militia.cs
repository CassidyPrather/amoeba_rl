using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using AmoebaRL.Behaviors;
using AmoebaRL.Core.Organelles;
using AmoebaRL.Interfaces;
using AmoebaRL.Systems;
using AmoebaRL.UI;
using RogueSharp;

namespace AmoebaRL.Core
{
    public class Militia : Actor, IProactive, IEatable
    {
        public Militia()
        {
            Awareness = 3;
            Color = Palette.Militia;
            Symbol = 'm';
            Speed = 16;
            Name = "Militia";
        }

        public virtual void OnEaten()
        {
            Game.DMap.RemoveActor(this);
            CapturedMilitia transformation = new CapturedMilitia
            {
                X = X,
                Y = Y
            };
            Game.DMap.AddActor(transformation);
        }

        public virtual bool Act()
        {
            List<Actor> seenTargets = Seen(Game.PlayerMass);
            if (seenTargets.Count > 0)
                ActToTargets(seenTargets);
            else
                Wander();
            return true;
        }

        public virtual void ActToTargets(List<Actor> seenTargets)
        {
            List<Path> actionPaths = PathsToNearest(seenTargets);
            if (actionPaths.Count > 0)
            {
                int pick = Game.Rand.Next(0, actionPaths.Count - 1);
                try
                {
                    //Formerly: path.Steps.First()
                    Game.CommandSystem.AttackMove(this, actionPaths[pick].StepForward());
                }
                catch (NoMoreStepsException)
                {
                    Game.MessageLog.Add($"{Name} curses");
                }
            } // else, wait a turn.
        }

        public virtual void Wander()
        {
            List<ICell> adj = Game.DMap.AdjacentWalkable(X, Y);
            int pick = Game.Rand.Next(0, adj.Count);
            if (pick != adj.Count)
                Game.CommandSystem.AttackMove(this, adj[pick]);
        }

        /*public override void PerformAction(CommandSystem commandSystem)
        {
            IBehavior behavior = new MilitiaPatrolAttack();//StandardMoveAndAttack(); //
            behavior.Act(this, commandSystem);
        }*/

        public class CapturedMilitia : Organelle, IProactive, IDigestable
        {
            public int MaxHP { get; set; }

            public int HP { get; set; }

            public CapturedMilitia()
            {
                Awareness = 1;
                Slime = true;
                Color = Palette.Militia;
                Name = "Dissolving Militia";
                Symbol = 'm';
                MaxHP = 8;
                HP = MaxHP;
                Speed = 16;
                Game.PlayerMass.Add(this);
            }

            public bool Act()
            {
                HP--;
                if (HP <= 0)
                {
                    Game.DMap.RemoveActor(this);
                    Cytoplasm transformation = new Cytoplasm
                    {
                        X = X,
                        Y = Y
                    };
                    Game.DMap.AddActor(transformation);
                    Game.PlayerMass.Add(transformation);
                }
                return true;
            }

            public override void OnDestroy() => BecomeActor(new Militia());
        }
    }
}
