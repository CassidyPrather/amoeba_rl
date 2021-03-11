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
    public class Tank : Militia
    {
        int ready = 1;

        public Tank()
        {
            Awareness = 3;
            Color = Palette.Calcium;
            Symbol = 't';
            Speed = 16;
            Name = "Tank";
        }

        public override bool Act()
        {
            if(!Engulf())
            {     
                if (ready > 0)
                {
                    Color = Palette.Calcium;
                    ready--;
                    return true;
                }
                Color = Palette.RestingTank;
                ready++;
                return base.Act();
            }
            return true;
        }

        public override void Die()
        {
            Game.DMap.RemoveActor(this);
            ICell drop = Game.DMap.NearestLootDrop(X, Y);
            CalciumDust transformation = new CalciumDust
            {
                X = drop.X,
                Y = drop.Y
            };
            Game.DMap.AddItem(transformation);
        }

        public override void OnEaten()
        {
            Game.DMap.RemoveActor(this);
            CapturedTank transformation = new CapturedTank
            {
                X = X,
                Y = Y
            };
            Game.DMap.AddActor(transformation);
        }

        public class CapturedTank : CapturedMilitia
        {
            public CapturedTank()
            {
                Awareness = 0;
                Slime = true;
                Color = Palette.Calcium;
                Name = "Dissolving Tank";
                Symbol = 't';
                MaxHP = 24;
                HP = MaxHP;
                Speed = 16;
                // Already called by parent?
                // Game.PlayerMass.Add(this);
            }

            public override Actor DigestsTo() => new Calcium();

            public override void OnUnslime() => BecomeActor(new Tank());


            public override void OnDestroy() => BecomeActor(new Tank());
        }
    }
}
