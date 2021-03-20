using AmoebaRL.Core.Organelles;
using AmoebaRL.UI;
using RogueSharp;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Core.Enemies
{
    public class Tank : Militia
    {
        public int StaminaPoolSize { get; set; } = 3;

        public int Exhaustion { get; set; } = 3;

        public override void Init()
        {
            Armor = 1;
            Awareness = 3;
            Color = Palette.RestingTank;
            Symbol = 't';
            Speed = 16;
            Name = "Tank";
        }

        public override string Flavor => $"A terrifying moving fortress wrapped in strong armor.";

        public override string DescBody => SlowArmorAddendum();

        protected string SlowArmorAddendum()
        {
            return "It cannot be killed or eaten by most means. It is vulnerable to friendly fire. It can " +
                "be engulfed by surrounding it on all sides with slime or walls. Cities and humans will not " +
                "help to engulf it, but any humans can be engulfed in contiguous groups! " +
                $"It acts every {StaminaPoolSize} turns ({Exhaustion} remaining).";
        }

        public override void Act()
        {
            if (!Engulf())
            {
                if (Exhaustion == 1)
                    Color = Palette.Calcium;
                if (Exhaustion > 0)
                    Exhaustion--;
                else
                {
                    Color = Palette.RestingTank;
                    Exhaustion = StaminaPoolSize;
                    base.Act();
                }
            }
        }

        public override List<Item> BecomesOnDie => new List<Item>() { new CalciumDust() };

        public override Actor BecomesOnEaten => new CapturedTank();

        public class CapturedTank : DissolvingNPC
        {
            public override void Init()
            {
                Color = Palette.Calcium;
                Name = "Dissolving Tank";
                Symbol = 't';
                MaxHP = 24;
                HP = MaxHP;
                Speed = 16;
                Awareness = 0;
                Slime = 1;
            }

            public override string Flavor => $"Much less scary now that its armor has been overcome.";

            public override Actor DigestsTo => new Calcium();

            public override Actor RescuesTo => new Tank();
        }
    }

    public class Mech : Tank
    {
        public override void Init()
        {
            Armor = 1;
            Awareness = 3;
            Color = Palette.RestingTank;
            Symbol = 'M';
            Speed = 16;
            Name = "Mech";
            Exhaustion = 1;
            StaminaPoolSize = 1;
        }

        public override string Flavor => "The humans attached legs to this tank to respond to the growing threat, " +
                "making it faster.";

        public override Actor BecomesOnEaten => new CapturedMech();

        public class CapturedMech : CapturedTank
        {
            public CapturedMech()
            {
                Awareness = 0;
                Slime = 1;
                Color = Palette.Calcium;
                Name = "Dissolving Mech";
                Symbol = 'M';
                MaxHP = 24;
                HP = MaxHP;
                Speed = 16;
            }

            public override string Flavor => "You can't just attach legs to a tank and expect it to go faster.";

            public override Actor DigestsTo => new Calcium();

            public override Actor RescuesTo => new Mech(); 
        }
    }

    public class Caravan : Tank
    {
        public override void Init()
        {
            Armor = 1;
            Awareness = 3;
            Color = Palette.RestingMilitia;
            Symbol = 'v';
            Speed = 16;
            Name = "Caravan";
            Exhaustion = 1;
            StaminaPoolSize = 1;
        }

        public override string Flavor => "It wants to be very far away from danger, so it probably has some precious cargo " +
                "inside of it. It was hastily retrofitted with armor and as such is much slower than it " +
                "was supposed to be.";

        public override List<Item> BecomesOnDie => new List<Item>() { Cargo() };

        public override Actor BecomesOnEaten => new CapturedCaravan();

        public Item Cargo()
        {
            if (Game.Rand.Next(1) == 0)
                return new CalciumDust();
            else
                return new SiliconDust();
        }

        public override void Act()
        {
            if(!Engulf())
            {
                if (Exhaustion == 1)
                    Color = Palette.Militia;
                if (Exhaustion > 0)
                    Exhaustion--;
                else
                {
                    Color = Palette.RestingMilitia;
                    Exhaustion = StaminaPoolSize;
                    List<Actor> seenTargets = Seen(Game.PlayerMass);
                    if (seenTargets.Count > 0)
                    {
                        ICell fleeTarget = MinimizeTerrorStep(seenTargets, false);
                        Game.CommandSystem.AttackMove(this, fleeTarget);
                    }
                    else
                        Wander();
                }
            }
        }

        public class CapturedCaravan : DissolvingNPC
        {
            public override void Init()
            {
                Color = Palette.Militia;
                Name = "Dissolving Caravan";
                Symbol = 'v';
                MaxHP = 24;
                HP = MaxHP;
                Speed = 16;
                Awareness = 0;
                Slime = 1;
            }

            public override string Flavor => $"A glimmer of loot is visible beneath the cracking armor.";

            public override Actor DigestsTo
            {
                get
                {
                    if (Game.Rand.Next(1) == 0)
                        return new Calcium();
                    else
                        return new Electronics();
                }
            }

            public override Actor RescuesTo => new Caravan();
        }
    }
}
