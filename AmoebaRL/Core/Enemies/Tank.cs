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
        public int StaminaPoolSize
        {
            get => Delay / 16;
            set => Delay = value * 16;
        }

        public int TurnsToAct
        {
            get
            {
                int? myScheduledTime = Map.Context.SchedulingSystem.ScheduledFor(this);
                if(myScheduledTime.HasValue)
                { 
                    return (myScheduledTime.Value - Map.Context.SchedulingSystem.GetTime()) / 16;
                }
                return -1;
            }
        }

        public override void Init()
        {
            Armor = 1;
            Awareness = 3;
            Delay = 48;
            Name = "Tank";
        }

        public override string Flavor => $"A terrifying moving fortress wrapped in strong armor.";

        public override string DescBody => SlowArmorAddendum();

        protected string SlowArmorAddendum()
        {
            return "It cannot be killed or eaten by most means. It is vulnerable to friendly fire. It can " +
                "be engulfed by surrounding it on all sides with slime or walls. Cities and humans will not " +
                "help to engulf it, but any humans can be engulfed in contiguous groups! " +
                $"It acts every {StaminaPoolSize} turns ({TurnsToAct} remaining).";
        }

        public override List<Item> BecomesOnDie => new List<Item>() { new CalciumDust() };

        public override Actor BecomesOnEaten => new CapturedTank();

        public class CapturedTank : DissolvingNPC
        {
            public override void Init()
            {
                Name = "Dissolving Tank";
                MaxHP = 24;
                HP = MaxHP;
                Delay = 16;
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
            Delay = 32;
            Name = "Mech";
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
                Name = "Dissolving Mech";
                MaxHP = 24;
                HP = MaxHP;
                Delay = 16;
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
            Delay = 32;
            Name = "Caravan";
        }

        public override string Flavor => "It wants to be very far away from danger, so it probably has some precious cargo " +
                "inside of it. It was hastily retrofitted with armor and as such is much slower than it " +
                "was supposed to be.";

        public override List<Item> BecomesOnDie => new List<Item>() { Cargo() };

        public override Actor BecomesOnEaten => new CapturedCaravan();

        public Item Cargo()
        {
            if (Map.Context.Rand.Next(1) == 0)
                return new CalciumDust();
            else
                return new SiliconDust();
        }

        public override void Act()
        {
            if(!Engulf())
            {
                List<Actor> seenTargets = Seen(act => act.IsPlayerAligned());
                if (seenTargets.Count > 0)
                {
                    ICell fleeTarget = ImmediateUphillStep(seenTargets, false);
                    Map.Context.CommandSystem.AttackMove(this, fleeTarget);
                }
                else
                    Wander();
            }
        }

        public class CapturedCaravan : DissolvingNPC
        {
            public override void Init()
            {
                Name = "Dissolving Caravan";
                MaxHP = 24;
                HP = MaxHP;
                Delay = 16;
                Awareness = 0;
                Slime = 1;
            }

            public override string Flavor => $"A glimmer of loot is visible beneath the cracking armor.";

            public override string NameOfResult => "Calcium (50%) or Electronics (50%)";

            public override Actor DigestsTo
            {
                get
                {
                    if (Map.Context.Rand.Next(1) == 0)
                        return new Calcium();
                    else
                        return new Electronics();
                }
            }

            public override Actor RescuesTo => new Caravan();
        }
    }
}
