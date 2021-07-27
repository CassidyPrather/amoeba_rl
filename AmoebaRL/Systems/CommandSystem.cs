using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using RogueSharp;
using RLNET;
using AmoebaRL.Core;
using AmoebaRL.Interfaces;
using AmoebaRL.Core.Organelles;
using AmoebaRL.Core.Enemies;

namespace AmoebaRL.Systems
{
    public class CommandSystem
    {
        private const int LONG_RANGE_FF_DIST = 3;

        public class SlimePathfind
        {
            public Actor current;
            public SlimePathfind dest;
            public int dist;

            public SlimePathfind(Actor a, SlimePathfind d, int di)
            {
                current = a;
                dest = d;
                dist = di;
            }
        }

        public Game CommandTo { get; set; }

        public bool IsPlayerTurn { get; set; }

        public CommandSystem(Game g)
        {
            CommandTo = g;
        }

        public void EndPlayerTurn()
        {
            IsPlayerTurn = false;
        }

        public void Win()
        {
            // TODO
            CommandTo.SchedulingSystem.Clear();
            CommandTo.DMap.AddActor(new PostMortem());

        }

        public void AdvanceTurn()
        {
            // Why was the original design for this recursive? Just asking
            // for stack problems.
            do
            {
                ISchedulable nextUp = CommandTo.SchedulingSystem.Get();
                if (nextUp is Nucleus n)
                {
                    CommandTo.DMap.UpdatePlayerFieldOfView();
                    IsPlayerTurn = true;
                    
                    //CommandTo.SchedulingSystem.Add(nextUp); // We want the same nucleus to go again, if possible.
                    n.SetAsActiveNucleus();
                    
                }
                else if (nextUp is PostMortem)
                {
                    CommandTo.DMap.UpdatePlayerFieldOfView();
                    IsPlayerTurn = true;
                    CommandTo.ActivePlayer = null;
                    CommandTo.SchedulingSystem.Add(nextUp);
                }
                else if (nextUp is IProactive behavior)
                {
                    behavior.Act();
                    // Bandaid for cases where things self-destruct: Could use global class "alive" variable?
                    if (CommandTo.DMap.Actors.Contains(nextUp))
                        CommandTo.SchedulingSystem.Add(nextUp);
                }
                else
                {
                    // ISchedulables with no behaviors are very strange indeed...
                    CommandTo.SchedulingSystem.Add(nextUp);
                }
                if (nextUp is IPostSchedule post)
                    post.DoPostSchedule();
            } while (!IsPlayerTurn);
        }

        public void AttackMove(NPC monster, ICell cell)
        {
            if (!CommandTo.DMap.SetActorPosition(monster, cell.X, cell.Y))
            {
                Actor target = CommandTo.DMap.GetActorAt(cell.X, cell.Y);
                if (CommandTo.DMap.PlayerMass.Contains(target))
                { // would be fine to just attack all slimed things instead?
                    Attack(monster, target);
                }
            }
        }

        /// <summary>
        /// Destroy a player tile, or react appropriately if it is special.
        /// </summary>
        /// <param name="monster"></param>
        /// <param name="victim"></param>
        public void Attack(NPC monster, Actor victim)
        {
            if(victim is Nucleus n)
            {
                bool saved = CheckAndSave(monster, victim);
                if (!saved)
                {
                    Actor newVictim = n.Retreat();
                    if (newVictim == null)
                    {
                        CommandTo.MessageLog.Add($"{victim.Name} could not retreat and was destroyed");
                        n.Destroy();
                    }
                    else
                    {
                        CommandTo.MessageLog.Add($"The {victim.Name} retreated into the nearby { newVictim.Name }, thereby avoiding death");
                        Attack(monster, newVictim);
                    }
                }
            }
            else if(victim is Membrane m && monster is ISlayable i)
            {
                if(monster.Armor > 0)
                {
                    if(victim is ReinforcedMembrane || victim is ReinforcedMaw)
                    {
                        CommandTo.MessageLog.Add($"The {monster.Name} is impaled by sharp {victim.Name} proteins!");
                        i.Die();
                    }
                    else
                    {
                        CommandTo.MessageLog.Add($"The {monster.Name} shrugs off the {victim.Name}'s proteins");
                        m.Destroy();
                    }
                }
                else
                {
                    CommandTo.MessageLog.Add($"The {monster.Name} is impaled by sharp {victim.Name} proteins!");
                    i.Die();
                }
            }    
            else if(victim is Organelle o)
            {
                // Check for protection.
                bool saved = CheckAndSave(monster, victim);
                if (!saved)
                {
                    CommandTo.MessageLog.Add($"The { monster.Name } destroys the {victim.Name}");
                    o.Destroy();
                }
            }
            else
            {
                CommandTo.MessageLog.Add($"A { victim.Name } is destroyed by a {monster.Name}");
                CommandTo.DMap.RemoveActor(victim);
            }
        }

        private bool CheckAndSave(Actor monster, Actor victim)
        {
            bool saved = false;
            List<Actor> adj = CommandTo.DMap.AdjacentActors(victim.X, victim.Y);
            IEnumerable<Actor> ffCheck = CommandTo.DMap.GetCellsInDiamond(victim.X, victim.Y, LONG_RANGE_FF_DIST).Select(x => CommandTo.DMap.GetActorAt(x.X, x.Y));
            if (!(monster is Tank))
            {
                saved = ffCheck.Any(x => x is ForceField);
                if(saved)
                {
                    CommandTo.MessageLog.Add($"An energy mantle force protects the {victim.Name} from the {monster.Name}");
                }
            }
            if(!saved)
            {
                saved = adj.Where(a => a is ForceField).Count() > 0;
                if (saved)
                {
                    CommandTo.MessageLog.Add($"An energy mantle force protects the {victim.Name} from the {monster.Name}");
                }
            }
            if (!saved)
            {
                List<NonNewtonianMembrane> nnms = adj.Where(a => a is NonNewtonianMembrane).Cast<NonNewtonianMembrane>().ToList();
                if (nnms.Count > 0)
                {
                    saved = true;
                    NonNewtonianMembrane savior = nnms[CommandTo.Rand.Next(nnms.Count() - 1)];
                    CommandTo.DMap.Swap(savior, victim);
                    CommandTo.MessageLog.Add($"The { savior.Name } rematerializes and protects the {victim.Name} from the {monster.Name}!");
                    if(monster is NPC n)
                    {
                        n.Die();
                        CommandTo.MessageLog.Add($"{monster.Name} is killed by the rematerializing phase membrane!!");
                    }


                }
            }

            return saved;
        }

        public void EatActor(Actor eating, Actor eaten)
        {
            // Also eat the item underneath, if it was present.
            Item under = CommandTo.DMap.GetItemAt(eaten.X, eaten.Y);
            if(eaten is IEatable e)
            {
                CommandTo.DMap.Swap(eating, eaten);
                e.OnEaten();
            }
            else
            {
                CommandTo.DMap.RemoveActor(eaten);
            }
            if (under != null && under is IEatable && !(under is Nutrient))
            {
                // We can do two moves is long as long as we don't accidentally eat a nutrient.
                Ingest(eating, under);
            }
        }

        public bool Ingest(Actor eating, Item eaten)
        {
            Point mealLocation = new Point(eaten.X, eaten.Y);
            bool moveAndTransform = false;
            if (eaten is Nutrient n)
            { 
                // Nutrients are the only IEatable which do not require cytoplasm hosts (is this good?)
                n.OnEaten();
            }

            else if (eaten is IEatable e)
            {
                // Find a cytoplasm to host the new thing.
                List<Actor> candidates = new List<Actor>() { eating };
                List<Actor> seen = new List<Actor>();
                List<Actor> frontier = new List<Actor>();
                List<Cytoplasm> selection = new List<Cytoplasm>();
                if (eating is Cytoplasm)
                {
                    selection.Add(eating as Cytoplasm);
                    moveAndTransform = true;
                }
                while (selection.Count == 0)
                {
                    // Find the nearest cytoplasm.
                    seen.AddRange(candidates);
                    frontier.Clear();
                    foreach (Actor c in candidates)
                        frontier.AddRange(CommandTo.DMap.PlayerMass.Where(a => a.AdjacentTo(c.X, c.Y) && !seen.Contains(a)));
                    candidates.Clear();
                    candidates.AddRange(frontier);
                    foreach (Actor potential in candidates)
                    {
                        if (potential is Cytoplasm)
                            selection.Add(potential as Cytoplasm);
                    }
                    if (candidates.Count == 0)
                        return false; // No room to eat
                }
                // Pick a random cytoplasm among the nearest.
                int pick = CommandTo.Rand.Next(0, selection.Count - 1);
                Actor recepient = selection[pick];
                Point newOrganellePos = new Point(recepient.X, recepient.Y);
                
                CommandTo.DMap.RemoveActor(recepient);
                eaten.X = newOrganellePos.X;
                eaten.Y = newOrganellePos.Y;
                e.OnEaten();
                if(moveAndTransform)
                    eating = CommandTo.DMap.GetActorAt(newOrganellePos.X, newOrganellePos.Y);
            }
            else
            {
                CommandTo.DMap.RemoveItem(eaten);
            }
            if(eating != null && eating is Organelle o)
                AttackMoveOrganelle(o, mealLocation.X, mealLocation.Y);
            return true;
        }

        public bool Wait()
        {
            return true;
        }

        // Return value is true if the player was able to move
        // false when the player couldn't move, such as trying to move into a wall
        public bool AttackMovePlayer(Organelle player, Direction direction)
        {
            int x = player.X;
            int y = player.Y;

            Point mod = ApplyDirection(new Point(x, y), direction);
            x = mod.X;
            y = mod.Y;

            return AttackMoveOrganelle(player, x, y);
        }

        public bool AttackMoveOrganelle(Organelle player, int x, int y)
        {
            bool success = false;
            if (player is IPreMove pre)
                pre.DoPreMove();
            Actor targetActor = CommandTo.DMap.GetActorAt(x, y);
            if (targetActor != null)
            {
                // Swap with slimed tiles
                if (targetActor.Slime > 0)
                {

                    CommandTo.DMap.Swap(player, targetActor);
                    if (targetActor is CraftingMaterial c)
                        c.TryUpgrade(player);
                    if (player is QuantumCore q)
                    {
                        player.Delay /= 2;
                        q.SetAsActiveNucleus();
                    }
                    success = true;
                }
                // Do complicated damage calculations for targets with armor.
                else if (targetActor is NPC n && n.Armor > 0)
                {
                    if (player is ReinforcedMaw)
                    {
                        CommandTo.MessageLog.Add($"The {targetActor.Name} is crushed by the jaws of the {player.Name}!");
                        EatActor(player, targetActor);
                        success = true;
                    }
                    else if(player is LaserCore)
                    {
                        CommandTo.MessageLog.Add($"The {targetActor.Name} is obliterated by the {player.Name}'s laser beam!");
                        EatActor(player, targetActor);
                        success = true;
                    }
                    else
                    {
                        CommandTo.MessageLog.Add($"The {targetActor.Name}'s armor is too strong for the {player.Name}!");
                        success = false;
                    }
                    
                }
                // Destroy cities only if their armor can be overcome
                else if(targetActor is City c)
                {
                    if(CommandTo.DMap.PlayerMass.Count >= c.Armor)
                    {
                        c.Destroy();
                        success = true;
                    }
                    else
                    {
                        CommandTo.MessageLog.Add($"There is not enough mass to destroy the {targetActor.Name}! (Have {CommandTo.DMap.PlayerMass.Count}, need {c.Armor})");
                    }
                }
                // Eat anything else that can be eaten.
                else if (targetActor is IEatable)
                {
                    CommandTo.MessageLog.Add($"The {player.Name} consumes the {targetActor.Name}.");
                    EatActor(player, targetActor);
                    success = true;
                }

            }
            else
            {
                Item targetItem = CommandTo.DMap.GetItemAt(x, y);
                if (targetItem != null)
                {
                    if (targetItem is IEatable)
                    {
                        if(Ingest(player, targetItem))
                            success = true;
                        else
                            success = MoveOrganelle(player, x, y);
                    }
                }
                else // No actor and no item; move the player
                {
                    success = MoveOrganelle(player, x, y);
                }
            }

            if (success)
            { 
                // Check for engulf near target.
                // Also need to check this when playermass is added to the map.
                foreach(ICell adj in player.Map.Adjacent(player.X, player.Y))
                {
                    Actor mightEngulf = player.Map.GetActorAt(adj.X, adj.Y);
                    if (mightEngulf is NPC n)
                        n.Engulf();
                }
                if(player is IPostAttackMove p)
                    p.DoPostAttackMove();
            }
            return success;
        }

        public Point ApplyDirection(Point basic, Direction dir)
        {
            Point output = new Point(basic.X, basic.Y);
            switch (dir)
            {
                case Direction.Up:
                    output.Y = basic.Y - 1;
                    break;
                case Direction.Down:
                    output.Y = basic.Y + 1;
                    break;
                case Direction.Left:
                    output.X = basic.X - 1;
                    break;
                case Direction.Right:
                    output.X = basic.X + 1;
                    break;
                case Direction.None:
                    break;
                default:
                    throw new InvalidOperationException("Invalid direction.");
            }
            return output;
        }

        private bool MoveOrganelle(Organelle player, int x, int y)
        {
            int counter = 1;
            int max = 0;
            bool done = false;
            SlimePathfind root = new SlimePathfind(player, null, 0);
            List<SlimePathfind> last = new List<SlimePathfind>() { root };
            List<SlimePathfind> accountedFor = new List<SlimePathfind>() { root };
            while (!done)
            {
                List<SlimePathfind> frontier = new List<SlimePathfind>();
                foreach (SlimePathfind l in last)
                {
                    List<Actor> pullIn = CommandTo.DMap.Actors.Where(a => a.Slime > 0
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

            List<SlimePathfind> best = accountedFor.Where(p => p.dist == max).ToList();

            int randSelect = CommandTo.Rand.Next(0, best.Count - 1);
            SlimePathfind selected = best[randSelect];
            List<Actor> path = new List<Actor>();
            bool looping = true; // why can't you come up with better name
            while (looping)
            {
                path.Add(selected.current);
                selected = selected.dest;
                if (selected == null)
                    looping = false;
            }

            path.Reverse();

            Point lastPoint = new Point(player.X, player.Y);
            if (CommandTo.DMap.WithinBounds(x,y) && CommandTo.DMap.SetActorPosition(player, x, y))
            {// Player was moved; cascade vaccume
                for (int i = 1; i < path.Count; i++)
                {
                    Point buffer = new Point(path[i].X, path[i].Y);
                    CommandTo.DMap.SetActorPosition(path[i], lastPoint.X, lastPoint.Y);
                    lastPoint = buffer;
                }
                return true;
            }

            return false;
        }

        public void NextNucleus(int shift)
        {
            List<Actor> nuclei = CommandTo.DMap.PlayerMass.Where(a => a is Nucleus).ToList();
            int curIdx = nuclei.IndexOf(CommandTo.ActivePlayer);
            int newIdx = (curIdx + shift) % nuclei.Count;
            if (newIdx < 0)
                newIdx += nuclei.Count();
            (nuclei[newIdx] as Nucleus).SetAsActiveNucleus();
        }
    }
}
