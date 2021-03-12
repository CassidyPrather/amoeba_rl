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

namespace AmoebaRL.Systems
{
    public class CommandSystem
    {
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

        public bool IsPlayerTurn { get; set; }

        public void EndPlayerTurn()
        {
            IsPlayerTurn = false;
        }

        public void AdvanceTurn()
        {
            // Why was the original design for this recursive? Just asking
            // for stack problems.
            do
            {
                ISchedulable nextUp = Game.SchedulingSystem.Get();
                if (nextUp is Nucleus n)
                {
                    IsPlayerTurn = true;
                    //Game.SchedulingSystem.Add(nextUp); // We want the same nucleus to go again, if possible.
                    n.SetAsActiveNucleus();
                    
                }
                else if (nextUp is PostMortem)
                {
                    IsPlayerTurn = true;
                    Game.Player = null;
                    Game.SchedulingSystem.Add(nextUp);
                }
                else if (nextUp is TutorialMonster monster)
                {
                    // The "monster" archetype is an artifact from the tutorial I'd like to move away from.
                    monster.PerformAction(this);
                    // Bandaid for cases where things self-destruct: Could use global class "alive" variable?
                    if (Game.DMap.Actors.Contains(nextUp))
                        Game.SchedulingSystem.Add(nextUp);
                }
                else if (nextUp is IProactive behavior)
                {
                    behavior.Act();
                    // Bandaid for cases where things self-destruct: Could use global class "alive" variable?
                    if (Game.DMap.Actors.Contains(nextUp))
                        Game.SchedulingSystem.Add(nextUp);
                }
                else
                {
                    // ISchedulables with no behaviors are very strange indeed...
                    Game.SchedulingSystem.Add(nextUp);
                }
                if (nextUp is IPostSchedule post)
                    post.DoPostSchedule();
            } while (!IsPlayerTurn);
        }

        public void AttackMove(Actor monster, ICell cell)
        {
            if (!Game.DMap.SetActorPosition(monster, cell.X, cell.Y))
            {
                Actor target = Game.DMap.GetActorAt(cell.X, cell.Y);
                if (Game.PlayerMass.Contains(target))
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
        public void Attack(Actor monster, Actor victim)
        {
            if(victim is Nucleus n)
            {
                bool saved = CheckAndSave(monster, victim);
                if (!saved)
                {
                    Actor newVictim = n.Retreat();
                    if (newVictim == null)
                    {
                        Game.MessageLog.Add($"{victim.Name} could not retreat and was destroyed");
                        n.Destroy();
                    }
                    else
                    {
                        Game.MessageLog.Add($"The {victim.Name} retreated into the nearby { newVictim.Name }, thereby avoiding death");
                        Attack(monster, newVictim);
                    }
                }
            }
            else if(victim is Membrane m && monster is ISlayable i)
            {
                if(i is Tank)
                {
                    if(victim is ReinforcedMembrane || victim is ReinforcedMaw)
                    {
                        Game.MessageLog.Add($"The {monster.Name} is impaled by sharp {victim.Name} protiens!");
                        i.Die();
                    }
                    else
                    {
                        Game.MessageLog.Add($"The {monster.Name} shrugs off the {victim.Name}'s protiens");
                        m.Destroy();
                    }
                }
                else
                {
                    Game.MessageLog.Add($"The {monster.Name} is impaled by sharp {victim.Name} protiens!");
                    i.Die();
                }
            }    
            else if(victim is Organelle o)
            {
                // Check for protection.
                bool saved = CheckAndSave(monster, victim);
                if (!saved)
                {
                    Game.MessageLog.Add($"The { monster.Name } destroys the {victim.Name}");
                    o.Destroy();
                }
            }
            else
            {
                Game.MessageLog.Add($"A { victim.Name } is destroyed by a {monster.Name}");
                Game.DMap.RemoveActor(victim);
            }
        }

        private static bool CheckAndSave(Actor monster, Actor victim)
        {
            bool saved = false;
            List<Actor> adj = Game.DMap.AdjacentActors(victim.X, victim.Y);
            if (!(monster is Tank))
            {
                saved = adj.Where(a => a is ForceField).Count() > 0;
                if (saved)
                {
                    Game.MessageLog.Add($"An energy mantle force protects the {victim.Name} from the {monster.Name}");
                }
            }
            if (!saved)
            {
                List<NonNewtonianMembrane> nnms = adj.Where(a => a is NonNewtonianMembrane).Cast<NonNewtonianMembrane>().ToList();
                if (nnms.Count > 0)
                {
                    saved = true;
                    NonNewtonianMembrane savior = nnms[Game.Rand.Next(nnms.Count() - 1)];
                    Game.DMap.Swap(savior, victim);
                    Game.MessageLog.Add($"The { savior.Name } rematerializes and protects the {victim.Name} from the {monster.Name}");
                }
            }

            return saved;
        }

        public void EatActor(Actor eating, Actor eaten)
        {
            // Also eat the item underneath, if it was present.
            Item under = Game.DMap.GetItemAt(eaten.X, eaten.Y);
            if(eaten is IEatable e)
            {
                Game.DMap.Swap(eating, eaten);
                e.OnEaten();
            }
            else
            {
                Game.DMap.RemoveActor(eaten);
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
                        frontier.AddRange(Game.PlayerMass.Where(a => a.AdjacentTo(c.X, c.Y) && !seen.Contains(a)));
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
                int pick = Game.Rand.Next(0, selection.Count - 1);
                Actor recepient = selection[pick];
                Point newOrganellePos = new Point(recepient.X, recepient.Y);
                
                Game.DMap.RemoveActor(recepient);
                eaten.X = newOrganellePos.X;
                eaten.Y = newOrganellePos.Y;
                e.OnEaten();
                if(moveAndTransform)
                    eating = Game.DMap.GetActorAt(newOrganellePos.X, newOrganellePos.Y);
            }
            else
            {
                Game.DMap.RemoveItem(eaten);
            }
            if(eating != null && eating is Organelle o)
                AttackMoveOrganelle(o, mealLocation.X, mealLocation.Y);
            return true;
        }



        public bool Wait()
        {
            return true; // may need to do more stuff here.
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
            Actor targetActor = Game.DMap.GetActorAt(x, y);
            if (targetActor != null)
            {
                if (targetActor.Slime > 0)
                { // Swap
                    Game.DMap.Swap(player, targetActor);
                    if (player is QuantumCore q)
                    {
                        player.Speed /= 2;
                        q.SetAsActiveNucleus();
                    }
                    success = true;
                }
                else if (targetActor is Tank)
                {
                    if (player is ReinforcedMaw)
                    {
                        Game.MessageLog.Add($"The {targetActor.Name} is crushed by the jaws of the {player.Name}!");
                        EatActor(player, targetActor);
                        success = true;
                    }
                    else if(player is LaserCore)
                    {
                        Game.MessageLog.Add($"The {targetActor.Name} is melted by the {player.Name}'s laser beam!");
                        EatActor(player, targetActor);
                        success = true;
                    }
                    else
                    {
                        Game.MessageLog.Add($"The {targetActor.Name}'s armor is too strong for the {player.Name}!");
                        success = false;
                    }
                    
                }
                else if (targetActor is IEatable)
                {
                    Game.MessageLog.Add($"The {player.Name} consumes the {targetActor.Name}.");
                    EatActor(player, targetActor);
                    success = true;
                }

            }
            else
            {
                Item targetItem = Game.DMap.GetItemAt(x, y);
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

            if (success && player is IPostAttackMove p)
                p.DoPostAttackMove();
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

        private static bool MoveOrganelle(Organelle player, int x, int y)
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

            List<SlimePathfind> best = accountedFor.Where(p => p.dist == max).ToList();

            int randSelect = Game.Rand.Next(0, best.Count - 1);
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
            if (Game.DMap.SetActorPosition(player, x, y))
            {// Player was moved; cascade vaccume
                for (int i = 1; i < path.Count; i++)
                {
                    Point buffer = new Point(path[i].X, path[i].Y);
                    Game.DMap.SetActorPosition(path[i], lastPoint.X, lastPoint.Y);
                    lastPoint = buffer;
                }
                return true;
            }

            return false;
        }

        public void NextNucleus(int shift)
        {
            List<Actor> nuclei = Game.PlayerMass.Where(a => a is Nucleus).ToList();
            int curIdx = nuclei.IndexOf(Game.Player);
            int newIdx = (curIdx + shift) % nuclei.Count;
            if (newIdx < 0)
                newIdx += nuclei.Count();
            (nuclei[newIdx] as Nucleus).SetAsActiveNucleus();
        }
    }
}
