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
                new UpgradePath(1, CraftingMaterial.Resource.CALCIUM, () => new ReinforcedMembrane()),
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
            source.AddRange(new[] { new CalciumDust() });
            return source;
        }
    }

    public class Maw : Membrane, IProactive
    {
        /// <summary>
        /// The range at which the maw will attempt to swap through organelles to get to things it wants to eat.
        /// </summary>
        public int HungerRange { get; set; } = 2;

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

        public override string Description => "A mouth in the side of the ooze welcomes in any delicious meal which gets too close. " +
            "Immune to melee attacks except those from tanks and mechs. " +
                $"Automatically attacks adjacent enemies and consumables, except for caravans, tanks, and mechs. " +
            $"Otherwise, swaps with organelles to get closer to targets it can smell ({HungerRange} tile range).";

        /// <summary>
        /// Checks to see if this Maw wants to eat <paramref name="check"/>.
        /// </summary>
        /// <param name="check">The <see cref="Entity"/> to check for attractiveness.</param>
        /// <returns>Whether this is hungry for <paramref name="check"/>.</returns>
        public virtual bool IsHungryFor(Entity check)
        {
            return (check is Militia && !(check is Tank)) || check is Item;
        }

        public virtual void Act()
        {
            IEnumerable<ICell> smellArea = Map.GetCellsInDiamond(X, Y, HungerRange);
            IEnumerable<Entity> desireTiles = smellArea.Select(smelledCell => Map.GetActorOrItem(smelledCell)).Where(smelledCell => IsHungryFor(smelledCell));
            if(desireTiles.Any())
            { 
                int minTaxicabDistance = desireTiles.Min(c => DungeonMap.TaxiDistance(this, c));
                IEnumerable<Entity> nearestDesireTiles = desireTiles.Where(c => DungeonMap.TaxiDistance(this, c) == minTaxicabDistance);
                /*List<Actor> seenTargets = Seen(Map.Actors).Where(s=> s is Militia).ToList();
                if (!seenTargets.All(t => t is Tank))
                    seenTargets = seenTargets.Where(t => !(t is Tank)).ToList();
                if (seenTargets.Count > 0)
                    ActToTargets(seenTargets);*/
                ActToTargets(nearestDesireTiles);
            }
        }

        public virtual void ActToTargets(IEnumerable<Entity> seenTargets) => ActToTargets(seenTargets, x => x is Organelle);

        public virtual void ActToTargets(IEnumerable<Entity> seenTargets, Func<Entity,bool> through)
        {
            List<Path> actionPaths = PathsThroughToNearest<Entity>(seenTargets, through);
            if (actionPaths.Count > 0)
            {
                int pick = Map.Context.Rand.Next(0, actionPaths.Count - 1);
                try
                {
                    //Formerly: path.Steps.First()
                    ICell nextStep = actionPaths[pick].StepForward();
                    Entity stepDesire = Map.GetActorOrItem(nextStep.X, nextStep.Y);
                    if (IsHungryFor(stepDesire) || stepDesire is Organelle)
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

        public override string Description => "The high concentration of calcium has resulted in a localized gravity distortion which protects " +
            " allies in range 1 from all melee attacks, except those made by tanks and mechs, and allies in range 3 from militia melee."
                + " It survives and kills any enemy that attacks it in melee.";

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

        public override string Description => "Built strong bones, and the bones were teeth. Automatically attacks adjacent enemies, including caravans, tanks, and mechs. Immune to melee." +
            $"Otherwise, swaps with organelles to get closer to targets it can smell ({HungerRange} tile range).";

        public override bool IsHungryFor(Entity check)
        {
            return base.IsHungryFor(check) || check is Tank;
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
                ActToTargets(seenTargets, x => x is Actor x_act && x_act.Slime > 0);
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
