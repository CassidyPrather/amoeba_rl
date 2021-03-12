using AmoebaRL.Core.Organelles;
using AmoebaRL.Interfaces;
using AmoebaRL.UI;
using RogueSharp;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Core
{
    public class Hunter : Militia
    {
        public int Range { get; protected set; } = 64;

        public static readonly int _firingTime = 2;

        public int Firing { get; protected set; } = _firingTime;

        public FiringTimer FireGraphic { get; protected set; } = null;

        public Point FiringDirection { get; protected set; }

        public List<Reticle> Targeted { get; protected set; } = new List<Reticle>();

        public virtual char BaseChar { get; protected set; } = 'h';

        public Hunter()
        {
            Awareness = 3;
            Color = Palette.Hunter;
            Symbol = 'h';
            Speed = 16;
            Name = "Hunter";
        }

        public override string GetDescription()
        {
            string msg = $"H-69 model robotic troop with a penetrating kinetic gun. If it sees a hostile within {Awareness} tiles orthogonally, " +
                $"it will line up a shot, which is fired after {_firingTime} turns if it isn't killed. This shot penetrates all" +
                $"tiles until it hits a wall, killing friendlies and enemies alike. Fortunately, its high salvage value means organelles destroyed " +
                $"by this shot will be able to be rebuilt from their remains.";
            if (Firing < _firingTime)
                msg += $" Fires in {Firing}";
            return msg;
        }

        public override bool Act()
        {
            if (!Engulf())
            {
                if (Firing <= 0)
                {
                    Fire();
                }
                else if (Firing < _firingTime)
                {
                    if (FireGraphic != null)
                        FireGraphic.T = Firing;
                    Firing--;
                    return true;
                }
                else
                {
                    return base.Act();
                }
            }
            else
            {
                if (FireGraphic != null)
                    Game.DMap.RemoveVFX(FireGraphic);
            }
            return true;
        }

        public virtual void Fire()
        {
            if(FireGraphic != null)
            {
                Game.DMap.RemoveVFX(FireGraphic);
                FireGraphic = null;
            }
            Firing = _firingTime;
            int hitCount = 0;
            foreach(Reticle r in Targeted)
            {
                Actor hit = Game.DMap.GetActorAt(r.X, r.Y);
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
                Game.DMap.RemoveVFX(r);
                Symbol = BaseChar;
            }
            if(hitCount > 0)
                Game.MessageLog.Add($"The {Name} hit {hitCount} mass.");
            Targeted.Clear();
            Symbol = BaseChar;
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
                    int pick = Game.Rand.Next(0, actionPaths.Count - 1);
                    picked = actionPaths[pick];
                    target = picked.Steps.Last();
                    actionPaths.Remove(picked);
                    HasFiringPath = picked.Length <= Range + 1 && (target.X == X || target.Y == Y);
                } while (!HasFiringPath && actionPaths.Count > 0);


                if (HasFiringPath)
                {
                    if (FireGraphic == null)
                    {
                        FireGraphic = new FiringTimer
                        {
                            T = _firingTime,
                            X = X,
                            Y = Y
                        };
                        Game.DMap.AddVFX(FireGraphic);
                        
                    }
                    ICell sights = picked.StepForward();
                    // Calculate direction of firing
                    FiringDirection = new Point(sights.X - X, sights.Y - Y);
                    if (FiringDirection.X > 0)
                        Symbol = (char)16;
                    else if (FiringDirection.X < 0)
                        Symbol = (char)17;
                    else if (FiringDirection.Y > 0)
                        Symbol = (char)31;
                    else if (FiringDirection.Y < 0)
                        Symbol = (char)30;
                    // Calculate targets
                    Point bullet = new Point(sights.X, sights.Y);
                    int distanceTravelled = 0;
                    while (Game.DMap.WithinBounds(bullet.X, bullet.Y) && !Game.DMap.IsWall(bullet.X, bullet.Y) && distanceTravelled < Range)
                    {
                        distanceTravelled ++;
                        Reticle r = new Reticle()
                        {
                            X = bullet.X,
                            Y = bullet.Y
                        };
                        Game.DMap.AddVFX(r);
                        Targeted.Add(r);
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

        public override void Die()
        {
            CleanVFX();
            Game.DMap.RemoveActor(this);
            ICell drop = Game.DMap.NearestLootDrop(X, Y);
            SiliconDust transformation = new SiliconDust()
            {
                X = drop.X,
                Y = drop.Y
            };
            Game.DMap.AddItem(transformation);
        }

        protected void CleanVFX()
        {
            if (FireGraphic != null)
                Game.DMap.RemoveVFX(FireGraphic);
            foreach (Reticle r in Targeted)
                Game.DMap.RemoveVFX(r);
        }

        public override void OnEaten()
        {
            CleanVFX();
            Game.DMap.RemoveActor(this);
            CapturedHunter transformation = new CapturedHunter
            {
                X = X,
                Y = Y
            };
            Game.DMap.AddActor(transformation);
        }

        public class CapturedHunter : CapturedMilitia
        {
            public CapturedHunter()
            {
                Awareness = 0;
                Slime = 1;
                Color = Palette.Hunter;
                Name = "Dissolving Hunter";
                Symbol = 'h';
                MaxHP = 16;
                HP = MaxHP;
                Speed = 16;
                // Game.DMap.UpdatePlayerFieldOfView();
                // Already called by parent?
                // Game.PlayerMass.Add(this);
            }

            public override string GetDescription()
            {
                return $"It could not fulfill its purpose. " + DissolvingAddendum();
            }

            public override string NameOfResult { get; set; } = "electronics";

            public override Actor DigestsTo() => new Electronics();

            public override void OnUnslime() => BecomeActor(new Hunter());

            public override void OnDestroy() => BecomeActor(new Hunter());
        }

        public class Reticle : Animation
        {

            public Reticle()
            {
                Color = Palette.ReticleForeground;
                BackgroundColor = Palette.ReticleBackground;
                Symbol = 'X';
                Frames = 2;
                Speed = 3;
            }

            public override void SetFrame(int idx)
            {
                if (idx == 0)
                    Transparent = false;
                else
                    Transparent = true;
            }
        }

        public class FiringTimer : Animation
        {
            public int T { get; set; } = 2;

            public FiringTimer()
            {
                Symbol = '2';
                Color = Palette.Hunter;
                BackgroundColor = Palette.FloorBackground;
                Speed = 3;
                Frames = 2;
            }

            public override void SetFrame(int idx)
            {
                Symbol = T.ToString()[0];
                if (idx == 0)
                    Transparent = false;
                else
                    Transparent = true;
            }
        }
    }

    public class Scout : Hunter
    {
        public Scout()
        {
            Awareness = 4;
            Color = Palette.Hunter;
            Symbol = 's';
            Speed = 16;
            Name = "Scout";
            Range = 3;
            BaseChar = Symbol;
        }

        public override string GetDescription()
        {
            string msg = $"Forward reconassiance sent by the humans to assess a threat. It has binoculars and can see {Awareness} tiles orthogonally. " +
                $"If it sees an enemy, it lines up a shot which is fired after {_firingTime} turns if it isn't killed. This shot penetrates all" +
                $"tiles until it hits a wall or travels {Range} tiles, killing friendlies and enemies alike. Organelles destroyed " +
                $"by this shot will be able to be fully rebuilt from their remains.";
            if (Firing < _firingTime)
                msg += $" Fires in {Firing}";
            return msg;
        }

        public override void Die()
        {
            CleanVFX();
            Game.DMap.RemoveActor(this);
            ICell drop = Game.DMap.NearestLootDrop(X, Y);
            SiliconDust transformation = new SiliconDust()
            {
                X = drop.X,
                Y = drop.Y
            };
            Game.DMap.AddItem(transformation);
        }

        public override void OnEaten()
        {
            CleanVFX();
            Game.DMap.RemoveActor(this);
            CapturedHunter transformation = new CapturedScout
            {
                X = X,
                Y = Y
            };
            Game.DMap.AddActor(transformation);
        }

        public class CapturedScout : CapturedHunter
        {
            public CapturedScout()
            {
                Awareness = 0;
                Slime = 1;
                Color = Palette.Hunter;
                Name = "Dissolving Scout";
                Symbol = 's';
                MaxHP = 16;
                HP = MaxHP;
                Speed = 16;
                // Game.DMap.UpdatePlayerFieldOfView();
            }

            public override string GetDescription()
            {
                return $"It can't do much scouting from there. Its equipment looks high-tech, though. " + DissolvingAddendum();
            }

            public override string NameOfResult { get; set; } = "electronics";

            public override Actor DigestsTo() => new Electronics();

            public override void OnUnslime() => BecomeActor(new Scout());

            public override void OnDestroy() => BecomeActor(new Scout());
        }
    }
}
