using AmoebaRL.Core.Organelles;
using AmoebaRL.UI;
using RogueSharp;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Core
{
    class Hunter : Militia
    {
        public static readonly int _firingTime = 2;

        public int Firing { get; protected set; } = _firingTime;

        public Point FiringDirection { get; protected set; }

        public List<Reticle> Targeted { get; protected set; } = new List<Reticle>();
        

        public Hunter()
        {
            Awareness = 3;
            Color = Palette.Hunter;
            Symbol = 'h';
            Speed = 16;
            Name = "Hunter";
        }

        public override bool Act()
        {
            if(!Engulf())
            { 
                if(Firing <= 0)
                {
                    Fire();
                }
                if(Firing < _firingTime)
                {
                    Firing--;
                    return true;
                }
                else
                {
                    return base.Act();
                }
            }
            return true;
        }

        public virtual void Fire()
        {
            Firing = _firingTime;
            foreach(Reticle r in Targeted)
            {
                Actor hit = Game.DMap.GetActorAt(r.X, r.Y);
                if(hit != null)
                {
                    if(hit is Organelle o)
                    {
                        o.Destroy();
                    }
                    else if(hit is Militia m)
                    {
                        m.Die();
                    }
                }
                Game.DMap.RemoveVFX(r);
            }
            Targeted.Clear();
            Symbol = 'h';
        }

        public override void ActToTargets(List<Actor> seenTargets)
        {
            List<Path> actionPaths = PathsToNearest(seenTargets);
            if (actionPaths.Count > 0)
            {
                int pick = Game.Rand.Next(0, actionPaths.Count - 1);
                ICell sights = actionPaths[pick].StepForward();
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
                while(!Game.DMap.IsWall(bullet.X, bullet.Y) && Game.DMap.WithinBounds(bullet.X, bullet.Y))
                {
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
            } // else, wait a turn.
        }

        public override void Die()
        {
            foreach (Reticle r in Targeted)
                Game.DMap.RemoveVFX(r);
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
            foreach (Reticle r in Targeted)
                Game.DMap.RemoveVFX(r);
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
                Slime = true;
                Color = Palette.Hunter;
                Name = "Dissolving Hunter";
                Symbol = 'h';
                MaxHP = 16;
                HP = MaxHP;
                Speed = 16;
                // Already called by parent?
                // Game.PlayerMass.Add(this);
            }

            public override Actor DigestsTo() => new Electronics();

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
    }
}
