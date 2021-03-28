using AmoebaRL.Core.Enemies;
using AmoebaRL.Interfaces;
using AmoebaRL.UI;
using RogueSharp;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Core.Organelles
{
    public class Membrane : Upgradable
    {
        public Membrane()
        {
            Name = "Membrane";
            Slime = 1;
            Awareness = 0;
            PossiblePaths = new List<UpgradePath>()
            {
                new UpgradePath(2, CraftingMaterial.Resource.CALCIUM, () => new ReinforcedMembrane()),
                new UpgradePath(1, CraftingMaterial.Resource.ELECTRONICS, () => new Maw())
            };
        }

        public override string Description => "Barbed wire integrated into the cytoplasm membrane is quite frightful for anybody who tries to directly attack it. " +
                "Unless they were a literal tank, of course. Goodness forbid a tank with legs.";

        public override List<Item> OrganelleComponents() => new List<Item>() { new BarbedWire(), new Nutrient() };
    }

    public class ReinforcedMembrane : Membrane
    {
        public ReinforcedMembrane()
        {
            Name = "Tough Membrane";
            Slime = 1;
            Awareness = 0;
            PossiblePaths = new List<UpgradePath>()
            {
                new UpgradePath(3, CraftingMaterial.Resource.CALCIUM, () => new ForceField()),
                new UpgradePath(1, CraftingMaterial.Resource.ELECTRONICS, () => new NonNewtonianMembrane()),
            };
        }

        public override string Description => "It menaces with spikes of calcium, and will kill even the mightiest of tanks or mechs who try to attack it.";

        public override List<Item> OrganelleComponents()
        {
            List<Item> source = base.OrganelleComponents();
            source.AddRange(new[] { new CalciumDust(), new CalciumDust() });
            return source;
        }
    }

    public class Maw : Membrane, IProactive
    {
        public Maw()
        {
            Name = "Maw";
            Slime = 1;
            Awareness = 1;
            Delay = 16;
            PossiblePaths = new List<UpgradePath>()
            {
                new UpgradePath(3, CraftingMaterial.Resource.CALCIUM, () => new ReinforcedMaw()),
                new UpgradePath(3, CraftingMaterial.Resource.ELECTRONICS, () => new Tentacle()),
            };
        }

        public override string Description => "A mouth in the side of the ooze welcomes in any delicious meal which gets too close to it. Immune to melee attacks, other than those from tanks and mechs." +
                " Automatically attacks adjacent enemies, except for caravans, tanks, and mechs.";

        public virtual void Act()
        {
            List<Actor> seenTargets = Seen(Map.Context.DMap.Actors).Where(s=> s is Militia).ToList();
            if (!seenTargets.All(t => t is Tank))
                seenTargets = seenTargets.Where(t => !(t is Tank)).ToList();
            if (seenTargets.Count > 0)
                ActToTargets(seenTargets);
        }

        public virtual void ActToTargets(List<Actor> seenTargets) => ActToTargets(seenTargets, IgnoreNone);

        public virtual void ActToTargets(List<Actor> seenTargets, Func<Actor,bool> ignoring)
        {
            List<Path> actionPaths = PathsToNearest(seenTargets, ignoring);
            if (actionPaths.Count > 0)
            {
                int pick = Map.Context.Rand.Next(0, actionPaths.Count - 1);
                try
                {
                    //Formerly: path.Steps.First()
                    ICell nextStep = actionPaths[pick].StepForward();
                    Map.Context.CommandSystem.AttackMoveOrganelle(this, nextStep.X, nextStep.Y);
                }
                catch (NoMoreStepsException)
                {
                    Map.Context.MessageLog.Add($"{Name} wimpers sadly");
                }
            } // else, wait a turn.
        }

        public override List<Item> OrganelleComponents() => new List<Item>() { new BarbedWire(), new Nutrient(), new SiliconDust() };
    }

    public class ForceField : ReinforcedMembrane
    {
        public ForceField()
        {
            Name = "Force Field";
            Slime = 1;
            Awareness = 0;
            PossiblePaths.Clear();
        }

        public override string Description => "The high concentration of calcium has resulted in a localized gravity distortion which protects all adjacent allies from melee attacks, except those made by tanks and mechs."
                + " It is immune tanks and mechs itself, killing those who engage with it in melee, but it is vulnerable to ranged attacks.";

        public override List<Item> OrganelleComponents()
        {
            List<Item> source = base.OrganelleComponents();
            source.AddRange(new[] { new CalciumDust(), new CalciumDust(), new CalciumDust() });
            return source;
        }
    }

    public class NonNewtonianMembrane : ReinforcedMembrane
    {
        public NonNewtonianMembrane()
        {
            Name = "Phase Membrane";
            Slime = 1;
            Awareness = 0;
            PossiblePaths.Clear();
        }

        public override string Description => "This slime dances along the lines of reality and imagination. It will phase into existence to block any melee " +
                "attacks made against its neighbors and kills those who attack it directly in melee. " +
                "While the neighbor might be disoriented by having its location swapped, it will continue to operate as usual.";

        public override List<Item> OrganelleComponents()
        {
            List<Item> source = base.OrganelleComponents();
            source.AddRange(new[] { new SiliconDust() });
            return source;
        }
    }

    public class ReinforcedMaw : Maw
    {
        public ReinforcedMaw()
        {
            Name = "Reinforced Maw";
            Slime = 1;
            Awareness = 1;
            Delay = 16;
            PossiblePaths.Clear();
        }

        public override string Description => "Built strong bones, and the bones were teeth. Automatically attacks adjacent enemies, including caravans, tanks, and mechs. Immune to melee.";

        public override void Act()
        {
            List<Actor> seenTargets = Seen(Map.Context.DMap.Actors).Where(s => s is Militia).ToList();
            if (seenTargets.Count > 0)
                ActToTargets(seenTargets);
        }

        public override List<Item> OrganelleComponents()
        {
            List<Item> source = base.OrganelleComponents();
            source.AddRange(new[] { new CalciumDust(), new CalciumDust(), new CalciumDust() });
            return source;
        }
    }

    public class Tentacle : Maw
    {
        private static readonly int TerrorRadius = 1;

        public Tentacle()
        {
            Name = "Tentacle";
            Slime = 1;
            Awareness = 3;
            Delay = 4;
            PossiblePaths.Clear();
        }

        public override string Description => "This reckless pseudopod thrashes about in hunger. It is very fast and can see very far, and will approach and attack any enemies it sees for free four times per turn." +
                "However, it will try to avoid placing itself adjacent to tanks and mechs, which it cannot defeat. It is immune to other melee attacks, though.";

        // Eat everything you see that you can. Retreat from tanks. Don't commit suicide.
        public override void Act()
        {
            List<Actor> seenTargets = Seen(Map.Context.DMap.Actors).Where(s => s is Militia && !(s is Caravan)).ToList();
            bool brave = true;
            // Future checks to see if t is caravan occur in this function, but I don't think they're needed.
            if (!seenTargets.All(t => !(!(t is Caravan) && t is Tank)))
            {
                List<Tank> tanks = seenTargets.Where(t => !(t is Caravan) && t is Tank).Cast<Tank>().ToList();
                if(tanks.Count > 0) // might be an unecessary condition
                { 
                    int nearestTankDistance = tanks.Min(t => t.Position.TaxiDistance(Position));
                    List<Tank> closest = tanks.Where(t => t.Position.TaxiDistance(Position) <= TerrorRadius).ToList(); 
                    if(closest.Count > 0)
                    {
                        if (MinimizeTerrorMove(tanks.Cast<Actor>()))
                            brave = false;
                    }
                }
                if (brave)
                    seenTargets = seenTargets.Where(t => !(t is Tank)).ToList();
            }
            if (seenTargets.Count > 0 && brave)
                ActToTargets(seenTargets, x => x.Slime > 0);
        }

        public virtual bool MinimizeTerrorMove(IEnumerable<Actor> terrorizers)
        {
            ICell best = ImmediateUphillStep(terrorizers, true);
            if (best.X == X && best.Y == Y)
                return false;
            Map.Context.CommandSystem.AttackMoveOrganelle(this, best.X, best.Y);
            return true;
        }

        public override List<Item> OrganelleComponents()
        {
            List<Item> source = base.OrganelleComponents();
            source.AddRange(new[] { new SiliconDust(), new SiliconDust(), new SiliconDust() });
            return source;
        }
    }

    public class BarbedWire : Catalyst
    {
        public BarbedWire()
        {
            Name = "Barbed Wire";
        }

        public override string Description => "Humans set these up to protect their cities. You can probably put it to better use.";

        public override Actor NewOrganelle() => new Membrane();
    }
}
