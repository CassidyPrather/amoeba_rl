using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using AmoebaRL.Core.Enemies;
using AmoebaRL.Core.Organelles;
using AmoebaRL.Interfaces;
using AmoebaRL.UI;
using RLNET;
using RogueSharp;
using static AmoebaRL.Systems.CommandSystem;

namespace AmoebaRL.Core.Organelles
{
    public class Nucleus : Upgradable
    {
        public Nucleus()
        {
            Awareness = 3;
            Name = "Nucleus";
            X = 10;
            Y = 10;
            Slime = 1;
            Delay = 16;
            PossiblePaths = new List<UpgradePath>()
            {
                new UpgradePath(1, CraftingMaterial.Resource.CALCIUM, () => new EyeCore()),
                new UpgradePath(2, CraftingMaterial.Resource.ELECTRONICS, () => new SmartCore())
            };
        }

        /// <summary>
        /// 
        /// Responsible for scheduling itself.
        /// </summary>
        public void SetAsActiveNucleus()
        {
            if (Map == null)
                return;
            Map.Context.ActivePlayer = this;
            Map.Context.SchedulingSystem.Remove(this); // Sometimes this is redundant, but it's good to check anyway.
            Map.Context.SchedulingSystem.Add(this);
            List<Actor> otherNucleus = Map.PlayerMass.Where(a => a is Nucleus && a != this).ToList();
            foreach (Nucleus n in otherNucleus)
            {
                Map.Context.SchedulingSystem.Remove(n);
                int buffer = n.Delay;
                n.Delay = Delay;
                Map.Context.SchedulingSystem.Add(n);
                n.Delay = buffer;
            }

            ColorMovingSlime();
        }

        public void ColorMovingSlime()
        {
            if (Map.Context.DMap == null)
                return; // this check shouldn't be necessary
            foreach (Actor already in Map.PlayerMass)
                already.Slime = 1;
            int counter = 1;
            int max = 0;
            bool done = false;
            SlimePathfind root = new SlimePathfind(this, null, 0);
            List<SlimePathfind> last = new List<SlimePathfind>() { root };
            List<SlimePathfind> accountedFor = new List<SlimePathfind>() { root };
            while (!done)
            {
                List<SlimePathfind> frontier = new List<SlimePathfind>();
                foreach (SlimePathfind l in last)
                {
                    List<Actor> pullIn = Map.Context.DMap.Actors.Where(a => a.Slime > 0
                                                                && (!(a is Organelle o) || !o.Anchor)
                                                                && a.AdjacentTo(l.current.X, l.current.Y)
                                                                && !accountedFor.Where(t => t.current == a).Any()).ToList();
                    for (int i = 0; i < pullIn.Count; i++)
                    {
                        max = counter;
                        SlimePathfind node = new SlimePathfind(pullIn[i], l, counter);
                        accountedFor.Add(node);
                        frontier.Add(node);
                    }
                }

                counter++;
                last = frontier;
                if (frontier.Count == 0)
                    done = true;
            }

            IEnumerable<SlimePathfind> best = accountedFor.Where(p => p.dist == max);

            foreach(SlimePathfind tails in best)
            {
                SlimePathfind current = tails;
                bool looping = true; // why can't you come up with better name
                while (looping)
                {
                    current.current.Slime = 2;
                    current = current.dest;
                    if (current == null)
                        looping = false;
                }
            }    
        }

        public override void OnUnslime()
        {
            base.OnUnslime();
            HandleGameOver();
        }

        public override void OnDestroy()
        {
            base.OnDestroy();
            HandleGameOver();
        }

        public void HandleGameOver()
        {
            if (Map.Context.DMap.Actors.Where(a => a is Nucleus).Count() == 0)
            {
                Map.Context.MessageLog.Add($"You lose. Final Score: {Map.PlayerMass.Count}.");
                Map.Context.SchedulingSystem.Clear();
                Map.AddActor(new PostMortem());
                Map.UpdatePlayerFieldOfView();
                Map.Context.MessageLog.Add("Press ESC to quit. Press R to play again.");
            }
        }

        public Organelle Retreat()
        {
            List<Actor> sacrifices = Map.PlayerMass.Where(a => a.Position.TaxiDistance(this.Position) == 1 && !(a is Nucleus) && (a is Organelle)).ToList();
            if (sacrifices.Count > 0)
            {
                List<Actor> bestSacrifices = sacrifices.Where(s => !(Map.Context.DMap.GetEntities(s.X,s.Y).Any(t => t is Reticle))).ToList();
                if (bestSacrifices.Count == 0)
                    bestSacrifices = sacrifices;
                // Prioritize retreating into organelles?
                int r = Map.Context.Rand.Next(0, bestSacrifices.Count - 1);
                Map.Context.DMap.Swap(this, bestSacrifices[r]);
                return bestSacrifices[r] as Organelle;
            }
            return null;
        }

        public override List<Item> OrganelleComponents() => new List<Item>() { new DNA(), new Nutrient() };

        public override string Description => "Might as well be the powerhouse of the cell. Can eat from the ground, attack, and move freely. However, they are very competitive, " +
                "and as such only one can move per turn. They are also cowards, and will retreat rather than be destroyed. They also won't upgrade unless " +
                "they move into a crafting material, unlike other organelles.";

        protected string NucleusAddendum()
        {
            return "As a nucleus, it conducts actions when active and retreats when it would ordinarily be destroyed when possible, and must step into materials to upgrade.";
        }
    }

    public class DNA : Catalyst
    {
        public DNA()
        {
            Name = "DNA";
        }

        public override string Description => "Short for Deoxyribonucleic Acid. It would be possible to fasion a new nucleus out of this.";

        public override Actor NewOrganelle() => new Nucleus();
    }

    // Nucleus upgrade tree:
    // Eye Core: Line of Sight + 3
    //   Laser Core: Kill tank
    //   Terror Core: Add one turn to schedule of adjacent enemies on move.
    // Smart Core: Speed / 2
    //   Gravity Core: Nearest slime attempts to move to nearest adjacent empty on move (x2)
    //   Quantum Core: Superfast Internal (Speed / 4)

    public class EyeCore : Nucleus
    {
        public EyeCore()
        {
            Awareness = 6;
            Name = "Eye Core";
            Slime = 1;
            Delay = 16;
            PossiblePaths = new List<UpgradePath>()
            {
                new UpgradePath(2, CraftingMaterial.Resource.CALCIUM, () => new LaserCore()),
                new UpgradePath(2, CraftingMaterial.Resource.ELECTRONICS, () => new TerrorCore())
            };
        }

        public override string Description => "A huge eye grants additional sight beyond that of a regular nucleus. " + NucleusAddendum();

        public override List<Item> OrganelleComponents()
        {
            List<Item> net = base.OrganelleComponents();
            net.AddRange(new List<Item>() { new CalciumDust() });
            return net;
        }
    }

    public class SmartCore : Nucleus
    {

        public SmartCore()
        {
            Awareness = 3;
            Name = "Smart Core";
            Slime = 1;
            Delay = 8;
            PossiblePaths = new List<UpgradePath>()
            {
                new UpgradePath(2, CraftingMaterial.Resource.CALCIUM, () => new GravityCore()),
                new UpgradePath(3, CraftingMaterial.Resource.ELECTRONICS, () => new QuantumCore())
            };
        }

        public override string Description => "Stealing optimization algorithms from the humans has caused this nucleus to move twice as fast. " + NucleusAddendum();

        public override List<Item> OrganelleComponents()
        {
            List<Item> net = base.OrganelleComponents();
            net.AddRange(new List<Item>() { new SiliconDust() });
            return net;
        }
    }

    public class LaserCore : EyeCore
    {
        public LaserCore()
        {
            Awareness = 6;
            Name = "Laser Core";
            Slime = 1;
            Delay = 16;
            PossiblePaths.Clear();
        }

        public override string Description => "It built strong bones, and the bones were eyeballs capable of shooting armor-melting lasers. Unlike a regular nucleus, " +
                "this one can attack caravans, tanks, and mechs directly. It also retains its predecessor's visual range. " + NucleusAddendum();

        public override List<Item> OrganelleComponents()
        {
            List<Item> net = base.OrganelleComponents();
            net.AddRange(new List<Item>() { new CalciumDust(), new CalciumDust() });
            return net;
        }
    }

    public class TerrorCore : EyeCore, IPostAttackMove, IPostSchedule
    {
        public List<Tuple<Actor, int>> Terrified { get; protected set; } = new List<Tuple<Actor, int>>();

        public TerrorCore()
        {
            Awareness = 6;
            Name = "Terror Core";
            Slime = 1;
            Delay = 16;
            PossiblePaths.Clear();
        }

        public override string Description => "An eye of this size is unnatural, and when it enters a space adjacent to a human, that human wastes a turn cowering in fear. " +
                "It maintains the vision boost of its predecessor and it has a chance to act before enemies it terrified recover. " + NucleusAddendum();

        public override List<Item> OrganelleComponents()
        {
            List<Item> net = base.OrganelleComponents();
            net.AddRange(new List<Item>() { new SiliconDust(), new SiliconDust() });
            return net;
        }

        public void DoPostAttackMove()
        {
            Terrified.Clear();
            foreach(Actor a in Map.Context.DMap.AdjacentActors(X,Y).Where(a => a is Militia).Cast<Militia>())
            {
                int? scheduledForTime = Map.Context.SchedulingSystem.ScheduledFor(a);
                if (scheduledForTime.HasValue)
                {
                    int untilTurn = scheduledForTime.Value - Map.Context.SchedulingSystem.GetTime();
                    Map.Context.SchedulingSystem.Remove(a);
                    Terrified.Add(new Tuple<Actor, int>(a, a.Delay));
                    a.Delay += untilTurn;
                }
                else
                    Map.Context.MessageLog.Add($"{a.Name} is already terrified");
            }
        }

        public void DoPostSchedule()
        {
            foreach (Tuple<Actor, int> a in Terrified)
            {
                Map.Context.SchedulingSystem.Add(a.Item1);
                a.Item1.Delay = a.Item2;
                SetAsActiveNucleus();
            }
        }
    }

    public class GravityCore : SmartCore, IPostAttackMove
    {
        public int GravityAttempts { get; protected set; } = 2;

        public int MaxRange { get; protected set; } = 2;

        public GravityCore()
        {
            Awareness = 3;
            Name = "Gravity Core";
            Slime = 1;
            Delay = 8;
            PossiblePaths.Clear();
        }

        public override string Description => $"After moving outside the bounds of your organelles, other organelles attempt to fill the spaces adjacent to it. " +
                $"This occurs up to {GravityAttempts} times per time the Gravity Core moves " +
                $"with a maximum range of {MaxRange}. Despite its density, this core also " +
                $"moves twice per turn. " + NucleusAddendum();

        public override List<Item> OrganelleComponents()
        {
            List<Item> net = base.OrganelleComponents();
            net.AddRange(new List<Item>() { new CalciumDust(), new CalciumDust() });
            return net;
        }

        public void DoPostAttackMove()
        {
            Anchor = true;
            for(int i = 0; i < GravityAttempts; i++)
            {
                List<ICell> adj = Map.Context.DMap.AdjacentWalkable(X,Y);
                if(adj.Count > 0)
                {
                    bool gravityIgnore(Actor x) => x is Militia && !(x is Tank);
                    ICell gravityTo = adj[Map.Context.Rand.Next(adj.Count - 1)];
                    // Find the nearest organelles (other than this or those adjacent to this) to gravityTo
                    // which can reach it without switching places with slime that is closer.
                    List<Organelle> nearest = Map.Context.DMap.NearestActors(X, Y, a =>
                            a is Organelle &&
                            DungeonMap.TaxiDistance(Map.Context.DMap.GetCell(a.X, a.Y), gravityTo) <= MaxRange && 
                            !(a == this) &&
                            a.PathExists(gravityIgnore, gravityTo.X, gravityTo.Y)
                            ).Cast<Organelle>().ToList();
                    while(nearest.Count > 0)
                    {
                        Organelle sel = nearest[Map.Context.Rand.Next(nearest.Count - 1)];
                        nearest.Remove(sel);
                        Path p = null;
                        try
                        {
                            p = sel.PathIgnoring(gravityIgnore, gravityTo.X, gravityTo.Y);
                            //p = DungeonMap.QuickShortestPath(Map.Context.DMap,
                            //    Map.Context.DMap.GetCell(X, Y),
                            //    Map.Context.DMap.GetCell(gravityTo.X, gravityTo.Y));
                        }
                        catch (PathNotFoundException) { }
                        if(p != null)
                        {
                            ICell next = p.StepForward();
                            Map.Context.CommandSystem.AttackMoveOrganelle(sel, next.X, next.Y);
                            nearest.Clear();
                        }
                    }
                }
            }
            Anchor = false;
        }
    }

    public class QuantumCore : SmartCore, IPreMove, IPostAttackMove
    {
        public int BaseSpeed { get; protected set; } = 8;

        public QuantumCore()
        {
            Awareness = 3;
            Name = "Quantum Core";
            Slime = 1;
            Delay = 8;
            PossiblePaths.Clear();
        }

        public override string Description => "This nucleus is quantum-entangled with all of the slime! If it is moving to the space of a fellow organelle (including cytoplasm), its does so twice as quickly. " +
                "This is in addition to the speed bonus of the smart core. " + NucleusAddendum();

        public override List<Item> OrganelleComponents()
        {
            List<Item> net = base.OrganelleComponents();
            net.AddRange(new List<Item>() { new SiliconDust(), new SiliconDust(), new SiliconDust() });
            return net;
        }

        public void DoPreMove()
        {
            Delay = BaseSpeed;
        }

        public void DoPostAttackMove()
        {
            Delay = BaseSpeed;
        }
    }
}
