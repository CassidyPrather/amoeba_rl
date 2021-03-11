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
            Color = Palette.RootOrganelle;
            Symbol = 'B';
            Name = "Membrane";
            Slime = true;
            Awareness = 0;
            PossiblePaths = new List<UpgradePath>()
            {
                new UpgradePath(2, CraftingMaterial.Resource.CALCIUM, () => new ReinforcedMembrane()),
                new UpgradePath(1, CraftingMaterial.Resource.ELECTRONICS, () => new Maw())
            };
        }

        public override string GetDescription()
        {
            return "Barbed wire integrated into the cytoplasm membrane is quite frightful for anybody who tries to directly attack it. Unless they were a literal tank, of course.";
        }

        public override List<Item> OrganelleComponents() => new List<Item>() { new BarbedWire(), new Nutrient() };
    }

    public class ReinforcedMembrane : Membrane
    {
        public ReinforcedMembrane()
        {
            Color = Palette.Calcium;
            Symbol = 'B';
            Name = "Reinforced Membrane";
            Slime = true;
            Awareness = 0;
            PossiblePaths = new List<UpgradePath>()
            {
                new UpgradePath(3, CraftingMaterial.Resource.CALCIUM, () => new ForceField()),
                new UpgradePath(1, CraftingMaterial.Resource.ELECTRONICS, () => new NonNewtonianMembrane()),
            };
        }

        public override string GetDescription()
        {
            return "It menaces with spikes of calcium, and will kill even the mightiest of tanks who try to attack it.";
        }

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
            Color = Palette.Hunter;
            Symbol = 'M';
            Name = "Maw";
            Slime = true;
            Awareness = 1;
            Speed = 16;
            PossiblePaths = new List<UpgradePath>()
            {
                new UpgradePath(3, CraftingMaterial.Resource.CALCIUM, () => new ReinforcedMaw()),
                new UpgradePath(3, CraftingMaterial.Resource.ELECTRONICS, () => new Tentacle()),
            };
        }

        public override string GetDescription()
        {
            return "A mouth in the side of the ooze welcomes in any delicious meal which gets too close to it. Immune to melee attacks, other than those from tanks." +
                " Automatically attacks adjacent enemies, except for tanks.";
        }

        public virtual bool Act()
        {
            List<Actor> seenTargets = Seen(Game.DMap.Actors).Where(s=> s is Militia).ToList();
            if (!seenTargets.All(t => t is Tank))
                seenTargets = seenTargets.Where(t => !(t is Tank)).ToList();
            if (seenTargets.Count > 0)
                ActToTargets(seenTargets);
            return true;
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
                    ICell nextStep = actionPaths[pick].StepForward();
                    Game.CommandSystem.AttackMoveOrganelle(this, nextStep.X, nextStep.Y);
                }
                catch (NoMoreStepsException)
                {
                    Game.MessageLog.Add($"{Name} wimpers sadly");
                }
            } // else, wait a turn.
        }

        public override List<Item> OrganelleComponents() => new List<Item>() { new BarbedWire(), new Nutrient(), new SiliconDust() };
    }

    public class ForceField : ReinforcedMembrane
    {
        public ForceField()
        {
            Color = Palette.Calcium;
            Symbol = 'F';
            Name = "Force Field";
            Slime = true;
            Awareness = 0;
            PossiblePaths.Clear();
        }

        public override string GetDescription()
        {
            return "The high concentration of calcium has resulted in a localized gravity distortion which prevents all adjacent allies other than tanks from attacks."
                + " It is immune even to tanks itself, but vulnerable to ranged attacks.";
        }

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
            Color = Palette.Calcium;
            Symbol = 'B';
            Name = "Non-Newtonian Membrane";
            Slime = true;
            Awareness = 0;
            PossiblePaths.Clear();
        }

        public override string GetDescription()
        {
            return "This slime dances along the lines of reality and imagination. It will phase into existence to block any melee attacks made against its neighbors, and is immune to them." +
                "While the neighbor might be disoriented by having its location swapped, it will continue to operate as usual.";
        }

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
            Color = Palette.Calcium;
            Symbol = 'M';
            Name = "Reinforced Maw";
            Slime = true;
            Awareness = 1;
            Speed = 16;
            PossiblePaths.Clear();
        }

        public override string GetDescription()
        {
            return "Built strong bones, and the bones were teeth. Automatically attacks adjacent enemies, including tanks. Immune to melee.";
        }

        public override bool Act()
        {
            List<Actor> seenTargets = Seen(Game.DMap.Actors).Where(s => s is Militia).ToList();
            if (seenTargets.Count > 0)
                ActToTargets(seenTargets);
            return true;
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
            Color = Palette.Hunter;
            Symbol = 'T';
            Name = "Tentacle";
            Slime = true;
            Awareness = 3;
            Speed = 4;
            PossiblePaths.Clear();
        }

        public override string GetDescription()
        {
            return "This reckless pseudopod thrashes about in hunger. It is very fast and can see very far, and will approach and attack any enemies it sees for free four times per turn." +
                "However, it will try to avoid placing itself adjacent to tanks, which it cannot defeat. It is immune to other melee attacks, though.";
        }

        // Eat everything you see that you can. Retreat from tanks. Don't commit suicide.
        public override bool Act()
        {
            List<Actor> seenTargets = Seen(Game.DMap.Actors).Where(s => s is Militia).ToList();
            bool brave = true;
            if (!seenTargets.All(t => t is Tank))
            {
                List<Tank> tanks = seenTargets.Where(t => t is Tank).Cast<Tank>().ToList();
                int nearestTankDistance = tanks.Min(t => DungeonMap.TaxiDistance(t, this));
                List<Tank> closest = tanks.Where(t => DungeonMap.TaxiDistance(t, this) <= TerrorRadius).ToList(); 
                if(closest.Count > 0)
                {
                    if (MinimizeTerror(tanks.Cast<Actor>()))
                        brave = false;
                }
                if(brave)
                    seenTargets = seenTargets.Where(t => !(t is Tank)).ToList();
            }
            if (seenTargets.Count > 0 && brave)
                ActToTargets(seenTargets);
            return true;
        }
        
        /// <summary>
        /// Take a single step to be as far away as possible from sources of terror.
        /// </summary>
        /// <param name="terrorizers">Things to be afraid of.</param>
        /// <returns>Whether the source of terror was escaped.</returns>
        public virtual bool MinimizeTerror(IEnumerable<Actor> terrorizers)
        {
            int mySafety = 0;
            foreach(Actor t in terrorizers)
                mySafety += DungeonMap.TaxiDistance(t, this);
            List<Actor> sacrifices = Game.DMap.AdjacentActors(X, Y).Where(a => !(a is Tank)).ToList();
            List<ICell> freeSpaces = Game.DMap.AdjacentWalkable(X, Y);
            List<Actor> safestSacrifices = new List<Actor>();
            List<ICell> safestFreeSpaces = new List<ICell>();
            int safestSacrificeVal = 0;
            int safestFreeSpaceVal = 0;
            
            // Find the safest sacrifice.
            foreach(Actor s in sacrifices)
            {
                int safety = 0;
                foreach (Actor t in terrorizers)
                    safety += DungeonMap.TaxiDistance(t, s);
                if(safety >= safestSacrificeVal)
                {
                    safestSacrificeVal = safety;
                    safestSacrifices.Add(s);
                }    
            }

            // Find the safest place to walk to.
            foreach (ICell s in freeSpaces)
            {
                int safety = 0;
                foreach (Actor t in terrorizers)
                    safety += DungeonMap.TaxiDistance(Game.DMap.GetCell(t.X, t.Y), s);
                if (safety >= safestFreeSpaceVal)
                {
                    safestFreeSpaceVal = safety;
                    safestFreeSpaces.Add(s);
                }
            }

            // If waiting is the safest option, return false.
            if (mySafety >= safestSacrificeVal && mySafety >= safestFreeSpaceVal)
                return false;

            // Otherwise, move to the safest spot and return true.
            bool takeSacrifice = safestSacrificeVal > safestFreeSpaceVal;
            if(safestFreeSpaceVal == safestSacrificeVal)
            {
                takeSacrifice = Game.Rand.Next(1) == 0;
            }
            if (takeSacrifice)
            {
                Actor targ = safestSacrifices[Game.Rand.Next(safestSacrifices.Count - 1)];
                Game.CommandSystem.AttackMoveOrganelle(this, targ.X, targ.Y);
            }
            else
            {
                ICell targ = safestFreeSpaces[Game.Rand.Next(safestFreeSpaces.Count - 1)];
                Game.CommandSystem.AttackMoveOrganelle(this, targ.X, targ.Y);
            }
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
            Color = Palette.OrganelleInactive;
            Symbol = 'b';
            Name = "Barbed Wire";
        }

        public override string GetDescription()
        {
            return "Humans set these up to protect their cities. You can probably put it to better use.";
        }

        public override Actor NewOrganelle()
        {
            return new Membrane();
        }
    }
}
