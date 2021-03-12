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
    public class Militia : Actor, IProactive, IEatable, ISlayable, IDescribable
    {
        public Militia()
        {
            Awareness = 3;
            Color = Palette.Militia;
            Symbol = 'm';
            Speed = 16;
            Name = "Militia";
        }

        public virtual string GetDescription()
        {
            return "A meager human who took up arms to defend its pitiful life. Nothing special about it. " +
                "Like all humans, it always tries to attack the nearest organelle. " +
                "Also like all humans, it can only see up to 3 cells away has no memory of anything it can't see.";
        }

        public virtual void Die()
        {
            Game.DMap.RemoveActor(this);
            ICell drop = Game.DMap.NearestLootDrop(X, Y);
            Nutrient transformation = new Nutrient
            {
                X = drop.X,
                Y = drop.Y
            };
            Game.DMap.AddItem(transformation);
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
            if(!Engulf())
            { 
                List<Actor> seenTargets = Seen(Game.PlayerMass);
                if (seenTargets.Count > 0)
                    ActToTargets(seenTargets);
                else
                    Wander();
            }
            return true;
        }
        
        public virtual bool Engulf()
        {
            List<ICell> adj = Game.DMap.Adjacent(X,Y);
            if(adj.Where(a => !Game.DMap.IsWall(a) 
                    && !Game.PlayerMass.Contains(Game.DMap.GetActorAt(a.X, a.Y)))
                .Count() == 0)
            {
                Game.MessageLog.Add($"The {Name} is engulfed!");
                OnEaten();
                return true;
            }
            return false;
            
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
                    Game.MessageLog.Add($"The {Name} contemplates the irrationality of its existence.");
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

            public int Overfill { get; set; } = 0;

            public CapturedMilitia()
            {
                Awareness = 1;
                Slime = 1;
                Color = Palette.Militia;
                Name = "Dissolving Militia";
                Symbol = 'm';
                MaxHP = 8;
                HP = MaxHP;
                Speed = 16;
                Game.PlayerMass.Add(this);
                // Game.DMap.UpdatePlayerFieldOfView();
            }


            public virtual bool Act()
            {
                HP--;
                if (HP <= 0)
                {
                    int numButchers = Game.PlayerMass.Where(k => k is Butcher).Count();
                    for (int i = 0; i < numButchers; i++)
                    {
                        Overfill = MaxHP * 2;
                        ProduceIfOverfull();
                    }
                    Game.DMap.RemoveActor(this);
                    Actor transformation = DigestsTo();
                    transformation.X = X;
                    transformation.Y = Y;
                    Game.DMap.AddActor(transformation);
                    Game.PlayerMass.Add(transformation);
                    Game.DMap.UpdatePlayerFieldOfView();
                }
                return true;
            }

            public bool ProduceIfOverfull()
            {
                if(Overfill >= MaxHP)
                {
                    List<ICell> drops = Game.DMap.NearestNoActor(X, Y);
                    if(drops.Count > 0)
                    {
                        ICell pick = drops[Game.Rand.Next(drops.Count - 1)];
                        if (Game.DMap.GetActorAt(pick.X, pick.Y) != null)
                            Game.MessageLog.Add("A result was instantiated in an occupied space!");
                        Actor bounty = DigestsTo();
                        bounty.X = pick.X;
                        bounty.Y = pick.Y;
                        Game.DMap.AddActor(bounty);
                        Game.PlayerMass.Add(bounty);
                        Overfill = 0;
                        Game.DMap.UpdatePlayerFieldOfView();
                    }
                }
                return false;
            }

            public virtual Actor DigestsTo() => new Cytoplasm();

            public override void OnUnslime() => BecomeActor(new Militia());

            public override void OnDestroy() => BecomeActor(new Militia());

            public override string GetDescription()
            {
                return $"Probably regrets its mediocrity. " + DissolvingAddendum();
            }

            public virtual string DissolvingAddendum()
            {
                if(Overfill == 0)
                { 
                    return $"After {HP} turns, it will become {NameOfResult}. Be careful, it can still be rescued!";
                }
                else
                {
                    return $"Kept alive against its will. It is regenerating up to {MaxHP} HP. After {MaxHP - Overfill} turns," +
                        $" it will produce {NameOfResult}. Be careful, it can still be rescued!";
                }
            }

            public virtual string NameOfResult { get; set; } = "cytoplasm";
        }
    }
}
