using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
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
        public virtual RLColor ActiveColor { get; set; } = Palette.Player;
        public virtual RLColor InactiveColor { get; set; } = Palette.PlayerInactive;

        public Nucleus()
        {
            Awareness = 3;
            Name = "Nucleus";
            Color = InactiveColor;
            Symbol = '@';
            X = 10;
            Y = 10;
            Slime = 1;
            Speed = 16;
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
            Game.Player = this;
            Color = ActiveColor;
            Game.SchedulingSystem.Remove(this); // Sometimes this is redundant, but it's good to check anyway.
            Game.SchedulingSystem.Add(this);
            List<Actor> otherNucleus = Game.PlayerMass.Where(a => a is Nucleus && a != this).ToList();
            foreach (Nucleus n in otherNucleus)
            {
                n.Color = n.InactiveColor; // or Pallette.Player.Inactive
                Game.SchedulingSystem.Remove(n);
                int buffer = n.Speed;
                n.Speed = Speed;
                Game.SchedulingSystem.Add(n);
                n.Speed = buffer;
            }

            ColorMovingSlime();
        }

        public void ColorMovingSlime()
        {
            if (Game.DMap == null)
                return; // this check shouldn't be necessary
            foreach (Actor already in Game.PlayerMass)
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
                    List<Actor> pullIn = Game.DMap.Actors.Where(a => a.Slime > 0
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
            if (Game.DMap.Actors.Where(a => a is Nucleus).Count() == 0)
            {
                Game.MessageLog.Add($"You lose. Final Score: {Game.PlayerMass.Count}.");
                Game.DMap.AddActor(new PostMortem());
            }
        }

        public Organelle Retreat()
        {
            List<Actor> sacrifices = Game.PlayerMass.Where(a => DungeonMap.TaxiDistance(this, a) == 1 && !(a is Nucleus) && (a is Organelle)).ToList();
            if (sacrifices.Count > 0)
            {
                List<Actor> bestSacrifices = sacrifices.Where(s => !(Game.DMap.GetVFX(s.X,s.Y) is Hunter.Reticle)).ToList();
                if (bestSacrifices.Count == 0)
                    bestSacrifices = sacrifices;
                // Prioritize retreating into organelles?
                int r = Game.Rand.Next(0, bestSacrifices.Count - 1);
                Game.DMap.Swap(this, bestSacrifices[r]);
                return bestSacrifices[r] as Organelle;
            }
            return null;
        }

        public override List<Item> OrganelleComponents() => new List<Item>() { new DNA(), new Nutrient() };
        
        public override string GetDescription()
        {
            return "Might as well be the powerhouse of the cell. Can eat from the ground, attack, and move freely. However, they are very competitive, " +
                "and as such only one can move per turn. They are also cowards, and will retreat rather than be destroyed.";
        }

        protected string NucleusAddendum()
        {
            return "As a nucleus, it conducts actions when active and retreats when it would ordinarily be destroyed when possible.";
        }
    }

    public class DNA : Catalyst
    {
        public DNA()
        {
            Name = "DNA";
            Color = Palette.Player;
            Symbol = 'X';
        }

        public override string GetDescription()
        {
            return "Short for Deoxyribonucleic Acid. It would be possible to fasion a new nucleus out of this.";
        }

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
        public override RLColor ActiveColor { get; set; } = Palette.Calcium;
        public override RLColor InactiveColor { get; set; } = Palette.RestingTank;

        public EyeCore()
        {
            Awareness = 6;
            Name = "Eye Core";
            Color = InactiveColor;
            Symbol = '@';
            Slime = 1;
            Speed = 16;
            PossiblePaths = new List<UpgradePath>()
            {
                new UpgradePath(2, CraftingMaterial.Resource.CALCIUM, () => new LaserCore()),
                new UpgradePath(2, CraftingMaterial.Resource.ELECTRONICS, () => new TerrorCore())
            };
        }

        public override string GetDescription()
        {
            return "A huge eye grants additional sight beyond that of a regular nucleus. " + NucleusAddendum();
        }

        public override List<Item> OrganelleComponents()
        {
            List<Item> net = base.OrganelleComponents();
            net.AddRange(new List<Item>() { new CalciumDust() });
            return net;
        }
    }

    public class SmartCore : Nucleus
    {
        public override RLColor ActiveColor { get; set; } = Palette.Hunter;
        public override RLColor InactiveColor { get; set; } = Palette.SmartCoreInactive;

        public SmartCore()
        {
            Awareness = 3;
            Name = "Smart Core";
            Color = InactiveColor;
            Symbol = '@';
            Slime = 1;
            Speed = 8;
            PossiblePaths = new List<UpgradePath>()
            {
                new UpgradePath(2, CraftingMaterial.Resource.CALCIUM, () => new GravityCore()),
                new UpgradePath(3, CraftingMaterial.Resource.ELECTRONICS, () => new QuantumCore())
            };
        }

        public override string GetDescription()
        {
            return "Stealing optimization algorithms from the humans has caused this nucleus to move twice as fast. " + NucleusAddendum();
        }

        public override List<Item> OrganelleComponents()
        {
            List<Item> net = base.OrganelleComponents();
            net.AddRange(new List<Item>() { new SiliconDust() });
            return net;
        }
    }

    public class LaserCore : EyeCore
    {
        public override RLColor ActiveColor { get; set; } = Palette.RootOrganelle;
        public override RLColor InactiveColor { get; set; } = Palette.OrganelleInactive;

        public LaserCore()
        {
            Awareness = 6;
            Name = "Laser Core";
            Color = Palette.PlayerInactive;
            Symbol = '@';
            Slime = 1;
            Speed = 16;
            PossiblePaths.Clear();
        }

        public override string GetDescription()
        {
            return "It built strong bones, and the bones were eyeballs capable of shooting tank-melting lasers. Unlike a regular nucleus, " +
                "this one can attack tanks directly. It also retains its predecessor's visual range. " + NucleusAddendum();
        }

        public override List<Item> OrganelleComponents()
        {
            List<Item> net = base.OrganelleComponents();
            net.AddRange(new List<Item>() { new CalciumDust(), new CalciumDust() });
            return net;
        }
    }

    public class TerrorCore : EyeCore, IPostAttackMove, IPostSchedule
    {
        public override RLColor ActiveColor { get; set; } = Palette.TerrorCoreActive;

        public override RLColor InactiveColor { get; set; } = Palette.Wall;

        public List<Tuple<Actor, int>> Terrified { get; protected set; } = new List<Tuple<Actor, int>>();

        public TerrorCore()
        {
            Awareness = 6;
            Name = "Terror Core";
            Color = Palette.PlayerInactive;
            Symbol = '@';
            Slime = 1;
            Speed = 16;
            PossiblePaths.Clear();
        }

        public override string GetDescription()
        {
            return "An eye of this size is unnatural, and when it enters a space adjacent to a human, that human wastes a turn cowering in fear. " +
                "It maintains the vision boost of its predecessor. " + NucleusAddendum();
        }

        public override List<Item> OrganelleComponents()
        {
            List<Item> net = base.OrganelleComponents();
            net.AddRange(new List<Item>() { new SiliconDust(), new SiliconDust() });
            return net;
        }

        public void DoPostAttackMove()
        {
            Terrified.Clear();
            foreach(Actor a in Game.DMap.AdjacentActors(X,Y).Where(a => a is Militia).Cast<Militia>())
            {
                int untilTurn = Game.SchedulingSystem.ScheduledFor(a).Value - Game.SchedulingSystem.GetTime();
                Game.SchedulingSystem.Remove(a);
                Terrified.Add(new Tuple<Actor,int>(a, a.Speed));
                a.Speed += untilTurn;
            }
        }

        public void DoPostSchedule()
        {
            foreach (Tuple<Actor, int> a in Terrified)
            {
                Game.SchedulingSystem.Add(a.Item1);
                a.Item1.Speed = a.Item2;
                SetAsActiveNucleus();
            }
        }
    }

    public class GravityCore : SmartCore, IPostAttackMove
    {
        public override RLColor ActiveColor { get; set; } = Palette.DarkSlime;
        public override RLColor InactiveColor { get; set; } = Palette.InactiveGravityCore;

        public int GravityAttempts { get; protected set; } = 2;

        public int MaxRange { get; protected set; } = 2;

        public GravityCore()
        {
            Awareness = 3;
            Name = "Gravity Core";
            Color = Palette.InactiveGravityCore;
            Symbol = '@';
            Slime = 1;
            Speed = 8;
            PossiblePaths.Clear();
        }

        public override string GetDescription()
        {
            return $"After moving, other organelles attempt to fill the spaces adjacent to it. " +
                $"This occurs up to {GravityAttempts} times per time the Gravity Core moves " +
                $"with a maximum range of {MaxRange}. Despite its density, this core also " +
                $"moves twice per turn. " + NucleusAddendum();
        }

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
                List<ICell> adj = Game.DMap.AdjacentWalkable(X,Y);
                if(adj.Count > 0)
                {
                    bool gravityIgnore(Actor x) => x is Militia && !(x is Tank);
                    ICell gravityTo = adj[Game.Rand.Next(adj.Count - 1)];
                    // Find the nearest organelles (other than this or those adjacent to this) to gravityTo
                    // which can reach it without switching places with slime that is closer.
                    List<Organelle> nearest = Game.DMap.NearestActors(X, Y, a =>
                            a is Organelle &&
                            DungeonMap.TaxiDistance(Game.DMap.GetCell(a.X, a.Y), gravityTo) <= MaxRange && 
                            !(a == this) &&
                            a.PathExists(gravityIgnore, gravityTo.X, gravityTo.Y)
                            ).Cast<Organelle>().ToList();
                    while(nearest.Count > 0)
                    {
                        Organelle sel = nearest[Game.Rand.Next(nearest.Count - 1)];
                        nearest.Remove(sel);
                        Path p = null;
                        try
                        {
                            p = sel.PathIgnoring(gravityIgnore, gravityTo.X, gravityTo.Y);
                            //p = DungeonMap.QuickShortestPath(Game.DMap,
                            //    Game.DMap.GetCell(X, Y),
                            //    Game.DMap.GetCell(gravityTo.X, gravityTo.Y));
                        }
                        catch (PathNotFoundException) { }
                        if(p != null)
                        {
                            ICell next = p.StepForward();
                            Game.CommandSystem.AttackMoveOrganelle(sel, next.X, next.Y);
                            nearest.Clear();
                        }
                    }
                }
            }
            Anchor = false;
        }
    }

    public class QuantumCore : SmartCore, IPreMove, IPostSchedule
    {
        public int BaseSpeed { get; protected set; } = 8;

        public override RLColor ActiveColor { get; set; } = Palette.Cursor;
        public override RLColor InactiveColor { get; set; } = Palette.InactiveQuantumCore;

        public QuantumCore()
        {
            Awareness = 3;
            Name = "Quantum Core";
            Color = Palette.InactiveQuantumCore;
            Symbol = '@';
            Slime = 1;
            Speed = 8;
            PossiblePaths.Clear();
        }

        public override string GetDescription()
        {
            return "This nucleus is quantum-entangled with all of the slime! If it is moving to the space of a fellow organelle (including cytoplasm), its does so twice as quickly. " +
                "This is in addition to the speed bonus of the smart core. " + NucleusAddendum();
        }

        public override List<Item> OrganelleComponents()
        {
            List<Item> net = base.OrganelleComponents();
            net.AddRange(new List<Item>() { new SiliconDust(), new SiliconDust(), new SiliconDust() });
            return net;
        }

        public void DoPreMove()
        {
            Speed = BaseSpeed;
        }

        public void DoPostSchedule()
        {
            Speed = BaseSpeed;
        }
    }
}
