﻿using AmoebaRL.Core.Enemies;
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
    // Chloroplast Upgrade Tree:
    // Bioreactor: Rate /= 2
    //   Biometal Forge: Rate *= 4, produces rare item
    //   Primordial Soup: Rate *= 4, produces T1 Organelle
    // Cultivator: Stop adjacent digestions, produce loot on 1/2 basis.
    //   Butcher: Replace adjacent digestions for 3x loot.
    //   Extractor: Cultivator which pulls nearby digested towards it.

    public class Chloroplast : Upgradable, IProactive
    {

        public int NextFood { get; set; } = 20;

        public Chloroplast()
        {
            Name = "Chloroplast";
            Slime = 1;
            Awareness = 0;
            base.Delay = 16;
            PossiblePaths = new List<UpgradePath>()
            {
                new UpgradePath(1, CraftingMaterial.Resource.CALCIUM, () => new Bioreactor()),
                new UpgradePath(1, CraftingMaterial.Resource.ELECTRONICS, () => new Cultivator())
            };
        }

        public override string Description => $"Converts solar radiation into nutrition. Next product in {NextFood}.";

        public override List<Item> OrganelleComponents() => new List<Item>() { new Plant(), new Nutrient() };

        public virtual void Act()
        {
            NextFood--;
            if(NextFood <= 0)
                if(Produce())
                    NextFood = Delay;
        }

        public virtual bool Produce()
        {
            ICell target = GetSpawnSpot();
            if (target == null)
                return false;
            Cytoplasm c = new Cytoplasm()
            {
                X = target.X,
                Y = target.Y
            };
            Map.AddActor(c);
            Map.PlayerMass.Add(c);
            return true;
        }

        protected ICell GetSpawnSpot()
        {
            List<ICell> targets = Map.NearestNoActor(X, Y);
            if (targets.Count == 0)
                return null;
            return targets[Map.Context.Rand.Next(targets.Count - 1)];
        }
    }

    public class Plant : Catalyst
    {
        public Plant()
        {
            Name = "Plant";
        }

        public override string Description => "A cute green plant. Its ability to use the sun to produce food is fascinating and could be exploited.";

        public override Actor NewOrganelle() => new Chloroplast();
    }

    public class Bioreactor : Chloroplast
    {
        public Bioreactor()
        {
            Name = "Bioreactor";
            Slime = 1;
            Awareness = 0;
            Delay = 10;
            Delay = 16;
            NextFood = Delay;
            PossiblePaths = new List<UpgradePath>()
            {
                new UpgradePath(3, CraftingMaterial.Resource.CALCIUM, () => new BiometalForge()),
                new UpgradePath(2, CraftingMaterial.Resource.ELECTRONICS, () => new PrimordialSoup())
            };
        }

        public override string Description => "A strong bone shell around this chloroplast has enabled a variety of physical and chemcial processes. " +
                "It is twice as fast at producing cytoplasm as its predecessor.";

        public override List<Item> OrganelleComponents()
        {
            List<Item> net = base.OrganelleComponents();
            net.AddRange(new List<Item>() { new CalciumDust() });
            return net;
        }
    }

    public class Cultivator : Upgradable, IProactive
    {
        public int OverfillRate { get; set; } = 1;

        public Cultivator()
        {
            Name = "Cultivator";
            Slime = 1;
            Awareness = 0;
            Delay = 16;
            PossiblePaths = new List<UpgradePath>()
            {
                new UpgradePath(2, CraftingMaterial.Resource.CALCIUM, () => new Extractor()),
                new UpgradePath(2, CraftingMaterial.Resource.ELECTRONICS, () => new Butcher())
            };
        }

        public virtual void Act()
        {
            // Check adjacent digestibles.
            List<DissolvingNPC> adj = Map.AdjacentActors(X, Y)
                .Where(a => a is DissolvingNPC)
                .Cast<DissolvingNPC>().ToList();

            foreach(DissolvingNPC m in adj)
            {
                HandleCaptured(m);
            }
        }

        protected virtual void HandleCaptured(DissolvingNPC m)
        {
            // Restore digestable HP by 1 (counteract its own digestion too)
            if (m.HP < m.MaxHP)
                m.HP += Math.Min(2, m.MaxHP-m.HP);
            // Overfill digestible by 1
            m.Overfill += OverfillRate;
            // Produce from overfull digestibles.
            m.ProduceIfOverfull();
        }

        public override string Description => "Forbids adjacent humans from dying, utilizing force-feeding to turn them into cattle. Restores the HP of adjacent dissolving targets. " +
                "Causes adjacent dissolving targets to produce their loot at their digestion rates for free without expending them. Be careful, they can " +
                "still be rescued!";

        public override List<Item> OrganelleComponents() => new List<Item>() { new Plant(), new Nutrient(), new SiliconDust() };
    }

    public class BiometalForge : Bioreactor
    {
        public BiometalForge()
        {
            Name = "Biometal Forge";
            Slime = 1;
            Awareness = 0;
            Delay = 45;
            Delay = 16;
            NextFood = Delay;
            PossiblePaths.Clear();
        }

        public override bool Produce()
        {
            ICell target = GetSpawnSpot();
            if (target == null)
                return false;

            Organelle produced;
            if (Map.Context.Rand.Next(1) == 0)
                produced = new Calcium();
            else
                produced = new Electronics();

            produced.X = target.X;
            produced.Y = target.Y;
            Map.AddActor(produced);
            Map.PlayerMass.Add(produced);
            return true;
        }

        public override string Description => $"The extremely dense shell around this bioreactor allows it to produce calcium and electronics. However, it is somewhat slow. " +
                $"Next product in {NextFood} turns.";

        public override List<Item> OrganelleComponents()
        {
            List<Item> net = base.OrganelleComponents();
            net.AddRange(new List<Item>() { new CalciumDust(), new CalciumDust(), new CalciumDust() });
            return net;
        }
    }

    public class PrimordialSoup : Bioreactor
    {
        public PrimordialSoup()
        {
            Name = "Primordial Soup";
            Slime = 1;
            Awareness = 0;
            Delay = 60;
            Delay = 16;
            NextFood = Delay;
            PossiblePaths.Clear();
        }

        public override bool Produce()
        {
            ICell target = GetSpawnSpot();
            if (target == null)
                return false;

            // This will need to be manually updated when new organelles are added.
            Organelle produced;
            int choice;
            if (Map.PlayerMass.Count(x => x.Name == "Nucleus") > 4)
                choice = Map.Context.Rand.Next(3);
            else
                choice = Map.Context.Rand.Next(4);
            if (choice < 2)
                produced = new Membrane();
            else if (choice < 4)
                produced = new Chloroplast();
            else
                produced = new Nucleus();

            produced.X = target.X;
            produced.Y = target.Y;
            Map.AddActor(produced);
            Map.PlayerMass.Add(produced); // This line was missing
            return true;
        }

        public override string Description => $"By integrating nanomachines into the protien folding process, new organelles can be produced. " +
            $"This can result in the production of new DNA, so long as no more than 4 unupgraded nuclei are owned." +
            $" However, it is very slow. Next product in {NextFood} turns.";

        public override List<Item> OrganelleComponents()
        {
            List<Item> net = base.OrganelleComponents();
            net.AddRange(new List<Item>() { new SiliconDust(), new SiliconDust() });
            return net;
        }
    }

    public class Extractor : Cultivator
    {

        public Extractor()
        {
            Name = "Extractor";
            Slime = 1;
            Awareness = 0;
            Delay = 16;
            PossiblePaths.Clear();
        }

        public override void Act()
        {
            List<Actor> adjUseless = Map.AdjacentActors(X, Y).Where(a => !(a is IDigestable)).ToList();
            List<ICell> adjRaw = Map.AdjacentWalkable(X, Y);
            foreach (Actor a in adjUseless)
                adjRaw.Add(Map.GetCell(a.X, a.Y));
            List<ICell> adjWantToEnter = new List<ICell>();
            // Randomize order
            while (!(adjRaw.Count == 0))
            {
                int pick = Map.Context.Rand.Next(adjRaw.Count - 1);
                adjWantToEnter.Add(adjRaw[pick]);
                adjRaw.RemoveAt(pick);
            }
            // Let each cell pull in a candidate.
            foreach (ICell dest in adjWantToEnter)
            {
                List<Organelle> wants = NeedsToReach(dest);
                if (wants.Count > 0)
                {
                    bool stuck = false;
                    Path p = null;
                    Organelle getsToGo = null;
                    do
                    {
                        stuck = false;
                        getsToGo = wants[Map.Context.Rand.Next(wants.Count - 1)];
                        p = getsToGo.PathIgnoring<Organelle>(x =>
                                Map.PlayerMass.Contains(x) &&
                                !(x is Extractor) &&
                                !(x is DissolvingNPC && x.AdjacentTo(X, Y)),
                            dest.X, dest.Y);
                        if (p == null)
                        {
                            stuck = true;
                            wants.Remove(getsToGo);
                        }
                    } while (stuck && wants.Count > 0);
                    if (p != null)
                    {
                        try
                        {
                            ICell step = p.StepForward();
                            Map.Context.CommandSystem.AttackMoveOrganelle(getsToGo, step.X, step.Y);
                        }
                        catch (RogueSharp.NoMoreStepsException)
                        {
                            Map.Context.MessageLog.Add($"A dissolving human was pulled into the space it was already in!");
                        }
                    }
                }
                else
                    break;

            }
            base.Act();
        }

        private List<Organelle> NeedsToReach(ICell dest)
        {
            List<Organelle> wants = Map.PlayerMass.Where(
                c => c is IDigestable && !c.AdjacentTo(X, Y) && c is Organelle
                ).Cast<Organelle>().ToList();
            wants = wants.Where(
                w => DungeonMap.TaxiDistance(Map.GetCell(w.X, w.Y), dest)
                == wants.Min(x => DungeonMap.TaxiDistance(Map.GetCell(w.X, w.Y), dest))
                )
                .ToList();
            return wants;
        }

        public override string Description
        {
            get
            {
                return "This cultivator is ravenous and will pull dissolving targets into each adjacent space! This happens no more than once per unoccupied space per turn. " +
                    "Restores the HP of adjacent dissolving targets. " +
                    "Causes adjacent dissolving targets to produce their loot at their digestion rates for free without expending them. Be careful, they can " +
                    "still be rescued!";
            }
        }

        public override List<Item> OrganelleComponents()
        {
            List<Item> net = base.OrganelleComponents();
            net.AddRange(new List<Item>() { new CalciumDust(), new CalciumDust() });
            return net;
        }
    }

    public class Butcher : Organelle
    {
        public Butcher()
        {
            Name = "Butcher";
            Slime = 1;
            Awareness = 0;
            Delay = 16;
        }

        public override string Description => "This butcher processes dissolved corpses efficiently. Every time a corpse is fully dissolved, it produces an extra product.";

        public override List<Item> Components() => new List<Item>() { new Plant(), new Nutrient(), new SiliconDust(), new SiliconDust(), new SiliconDust() };
    }
}
