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
        public int StaminaPoolSize { get; set; } = 3 ;

        public int Stamina { get; set; } = 3;

        public Tank()
        {
            Awareness = 3;
            Color = Palette.RestingTank;
            Symbol = 't';
            Speed = 16;
            Name = "Tank";
        }

        public override string GetDescription()
        {
            return $"A terrifying fortress wrapped in strong armor. It cannot be killed or eaten " +
                $"by most means, although all that armor means it can only act every {StaminaPoolSize} turns. " + ArmorAddendum();
                
        }

        protected string ArmorAddendum()
        {
            return "Fortunately, it is not expecting friendly fire, and it is vulnerable to it. It can " +
                "also be engulfed by surrounding it on all sides with slime or walls. Cities and humans will not help to engulf it! " +
                $"It will act in {Stamina} turns.";
        }

        public override bool Act()
        {
            if(!Engulf())
            {
                
                if (Stamina == 1)
                {
                    Color = Palette.Calcium;
                }
                if (Stamina > 0)
                {
                    Stamina--;
                }
                else
                {
                    Color = Palette.RestingTank;
                    Stamina = StaminaPoolSize;
                    return base.Act();
                }
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
                Slime = 1;
                Color = Palette.Calcium;
                Name = "Dissolving Tank";
                Symbol = 't';
                MaxHP = 24;
                HP = MaxHP;
                Speed = 16;
                // Already called by parent?
                // Game.PlayerMass.Add(this);
            }

            public override string GetDescription()
            {
                return $"Much less scary now that its armor has been overcome. " + DissolvingAddendum();
            }

            public override string NameOfResult { get; set; } = "calcium";

            public override Actor DigestsTo() => new Calcium();

            public override void OnUnslime() => BecomeActor(new Tank());

            public override void OnDestroy() => BecomeActor(new Tank());
        }
    }

    public class Mech : Tank
    {
        public Mech()
        {
            Awareness = 3;
            Color = Palette.RestingTank;
            Symbol = 'M';
            Speed = 16;
            Name = "Mech";
            Stamina = 1;
            StaminaPoolSize = 1;
        }

        public override string GetDescription()
        {
            return "The humans attached legs to this tank to respond to the growing threat, " +
                "making it faster. It now acts once every two turns. " + ArmorAddendum();
        }

        public override void Die()
        {
            Game.DMap.RemoveActor(this);
            ICell drop = Game.DMap.NearestLootDrop(X, Y);
            Item transformation = Drops();
            transformation.X = drop.X;
            transformation.Y = drop.Y;
            Game.DMap.AddItem(transformation);
        }

        public override void OnEaten()
        {
            Game.DMap.RemoveActor(this);
            Actor transformation = new CapturedMech
            {
                X = X,
                Y = Y
            };
            Game.DMap.AddActor(transformation);
        }

        public Item Drops() => new CalciumDust();

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
                // Already called by parent?
                // Game.PlayerMass.Add(this);
            }

            public override string GetDescription()
            {
                return $"You can't just attach legs to a tank and expect it to go faster. " + DissolvingAddendum();
            }

            public override string NameOfResult { get; set; } = "calcium";

            public override Actor DigestsTo() => new Calcium();

            public override void OnUnslime() => BecomeActor(new Mech());

            public override void OnDestroy() => BecomeActor(new Mech());
        }
    }

    public class Caravan : Tank
    {
        public Caravan()
        {
            Awareness = 3;
            Color = Palette.RestingMilitia;
            Symbol = 'v';
            Speed = 16;
            Name = "Caravan";
            Stamina = 1;
            StaminaPoolSize = 1;
        }

        public override string GetDescription()
        {
            return "It wants to be very far away from danger, so it probably has some precious cargo " +
                "inside of it. It was hastily retrofitted with armor and as such is much slower than it " +
                "was supposed to be, acting only every third turn. " + ArmorAddendum();
        }

        public override void Die()
        {
            Game.DMap.RemoveActor(this);
            ICell drop = Game.DMap.NearestLootDrop(X, Y);
            Item transformation = Drops();
            transformation.X = drop.X;
            transformation.Y = drop.Y;
            Game.DMap.AddItem(transformation);
        }

        public override void OnEaten()
        {
            Game.DMap.RemoveActor(this);
            Actor transformation = new CapturedCaravan
            {
                X = X,
                Y = Y
            };
            Game.DMap.AddActor(transformation);
        }

        public Item Drops()
        {
            if (Game.Rand.Next(1) == 0)
                return new CalciumDust();
            else
                return new SiliconDust();
        }

        public override bool Act()
        {
            if(!Engulf())
            {
                if (Stamina == 1)
                    Color = Palette.Militia;
                if (Stamina > 0)
                    Stamina--;
                else
                {
                    Color = Palette.RestingMilitia;
                    Stamina = StaminaPoolSize;
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
            return true;
        }

        public class CapturedCaravan : CapturedTank
        {
            public CapturedCaravan()
            {
                Awareness = 0;
                Slime = 1;
                Color = Palette.Militia;
                Name = "Dissolving Caravan";
                Symbol = 'v';
                MaxHP = 24;
                HP = MaxHP;
                Speed = 16;
                // Already called by parent?
                // Game.PlayerMass.Add(this);
            }

            public override string GetDescription()
            {
                return $"A glimmer of loot is visible beneath the cracking armor. " + DissolvingAddendum();
            }

            public override string NameOfResult { get; set; } = "calcium or electronics (50% chance of each)";

            public override Actor DigestsTo()
            {
                if (Game.Rand.Next(1) == 0)
                    return new Calcium();
                else
                    return new Electronics();
            }

            public override void OnUnslime() => BecomeActor(new Caravan());


            public override void OnDestroy() => BecomeActor(new Caravan());
        }
    }
}
