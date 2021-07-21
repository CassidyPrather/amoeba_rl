using AmoebaRL.Core.Organelles;
using AmoebaRL.Interfaces;
using AmoebaRL.UI;
using RogueSharp;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Core.Enemies
{
    public class Hunter : Militia
    {
        public int Range { get; protected set; } = 64;

        public int FiringTime { get; set; } = 2;

        public int Firing { get; protected set; } = 2;

        public Point FiringDirection { get; protected set; }

        public List<Reticle> Targeted { get; protected set; } = new List<Reticle>();

        public virtual char BaseChar { get; protected set; } = 'h';

        public override void Init()
        {
            Awareness = 3;
            Delay = 16;
            Name = "Hunter";
        }

        public override string Flavor => "H-69 model robotic troop with a penetrating kinetic gun.";

        public override string DescBody
        {
            get
            {
                string msg = $" If it sees a hostile within {Awareness} tiles orthogonally, " +
                    $"prepare to fire at it in {FiringTime} turns if it isn't killed. This shot penetrates all " +
                    $"tiles until it hits a wall{(Range < Map.Context.DMap.Width * Map.Context.DMap.Height ? $" or travels {Range} spaces" : "")}, " +
                    $"killing friendlies and enemies alike. Fortunately, organelles destroyed " +
                    $"by this shot drop all of the components used to build them.";
                if (Firing < FiringTime)
                    msg += $" Fires in {Firing + 1} turn{(Firing > 0 ? "s" : "")}.";
                return msg;
            }
        }

        public void ClearReticles()
        {
            foreach (Reticle r in Targeted)
                Map.RemoveVFX(r);
        }

        public override void Act()
        {
            if (!Engulf())
            {
                if (Firing <= 0)
                    Fire();
                else if (Firing < FiringTime)
                    Firing--;
                else
                    base.Act();
            }
        }

        public virtual void Fire()
        {
            Firing = FiringTime;
            int hitCount = 0;
            foreach(Reticle r in Targeted)
            {
                Actor hit = Map.GetActorAt(r.X, r.Y);
                if(hit != null)
                {
                    hitCount++;
                    if (hit is Nucleus n)
                    {
                        Organelle newVictim = n.Retreat();
                        if (newVictim == null)
                        {
                            n.Unslime();
                        }
                        else
                        {
                            newVictim.Unslime();
                        }
                    }
                    else if(hit is Organelle o)
                    {
                        o.Unslime();
                    }
                    else if(hit is Militia m)
                    {
                        m.Die();
                    }
                }
            }
            if(hitCount > 0)
                Map.Context.MessageLog.Add($"The {Name} hit {hitCount} mass.");
            ClearReticles();
            Targeted.Clear();
        }

        public override void ActToTargets(List<Actor> seenTargets)
        {
            List<Path> actionPaths = PathsToNearest(seenTargets);
            if (actionPaths.Count > 0)
            {
                Path picked;
                ICell target;
                bool HasFiringPath;
                do
                {
                    int pick = Map.Context.Rand.Next(0, actionPaths.Count - 1);
                    picked = actionPaths[pick];
                    target = picked.Steps.Last();
                    actionPaths.Remove(picked);
                    HasFiringPath = picked.Length <= Range + 1 && (target.X == X || target.Y == Y);
                } while (!HasFiringPath && actionPaths.Count > 0);


                if (HasFiringPath)
                {
                    // This line caused hunters to fire sideways sometimes, e.g. when an ally was in the way.
                    // ICell sights = picked.StepForward();
                    // Funny story, I accidentally coded in diagonal firing while working on this.
                    Point sights = new Point(X, Y);
                    if (target.X > X)
                        sights.X++;
                    else if (target.X < X)
                        sights.X--;
                    else if (target.Y > Y)
                        sights.Y++;
                    else
                        sights.Y--;
                    // Calculate direction of firing
                    FiringDirection = new Point(sights.X - X, sights.Y - Y);

                    // Calculate targets
                    Point bullet = new Point(sights.X, sights.Y);
                    int distanceTravelled = 0;
                    while (Map.Context.DMap.WithinBounds(bullet.X, bullet.Y) && !Map.Context.DMap.IsWall(bullet.X, bullet.Y) && distanceTravelled < Range)
                    {
                        distanceTravelled ++;
                        Reticle r = new Reticle
                        {
                            X = bullet.X,
                            Y = bullet.Y
                        };
                        Targeted.Add(r);
                        Map.AddVFX(r);
                        bullet.X += FiringDirection.X;
                        bullet.Y += FiringDirection.Y;
                    }
                    // Start firing countdown
                    Firing--;
                }
                else
                {
                    base.ActToTargets(seenTargets);
                }
            } // else, wait a turn.
        }

        public override List<Item> BecomesOnDie => new List<Item>() { new SiliconDust() };

        public override Actor BecomesOnEaten => new CapturedHunter();

        public override void Die()
        {
            ClearReticles();
            Targeted.Clear();
            base.Die();
        }

        public override void OnEaten()
        {
            ClearReticles();
            Targeted.Clear();
            base.OnEaten();
        }

        public class CapturedHunter : DissolvingNPC
        {
            public override void Init()
            {
                Name = "Dissolving Hunter";
                MaxHP = 16;
                HP = MaxHP;
                Awareness = 0;
                Slime = 1;
                Delay = 16;
            }

            public override string Flavor => $"It could not fulfill its purpose.";

            public override Actor DigestsTo => new Electronics();

            public override Actor RescuesTo => new Hunter();
        }
    }

    public class Scout : Hunter
    {
        public override void Init()
        {
            Awareness = 4;
            Delay = 16;
            Name = "Scout";
            Range = 3;
        }

        public override string Flavor => "Forward reconassiance sent by the humans to assess a threat. ";

        public override Actor BecomesOnEaten => new CapturedScout();

        public class CapturedScout : DissolvingNPC
        {
            public override void Init()
            {
                Name = "Dissolving Scout";
                MaxHP = 16;
                HP = MaxHP;
                Delay = 16;
                Awareness = 0;
                Slime = 1;
            }

            public override string Flavor => $"It can't do much scouting from there. Its equipment looks high-tech, though.";

            public override Actor DigestsTo => new Electronics();

            public override Actor RescuesTo => new Scout();
        }
    }

    public class Reticle : Entity
    {
        // ...?
    }
}
