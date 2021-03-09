using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using RogueSharp;
using RLNET;
using AmoebaRL.Core;
using AmoebaRL.Interfaces;

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
            ISchedulable nextUp = Game.SchedulingSystem.Get();
            if (nextUp is Nucleus)
            {
                IsPlayerTurn = true;
                Game.SchedulingSystem.Add(nextUp);
            }
            else if(nextUp is TutorialMonster monster)
            {
                // The "monster" archetype is an artifact from the tutorial I'd like to move away from.
                monster.PerformAction(this);
                // Bandaid for cases where things self-destruct: Could use global class "alive" variable?
                if (Game.DMap.Actors.Contains(nextUp))
                    Game.SchedulingSystem.Add(nextUp);
                AdvanceTurn();
            }
            else if(nextUp is IProactive behavior)
            {
                behavior.Act(); 
                // Bandaid for cases where things self-destruct: Could use global class "alive" variable?
                if(Game.DMap.Actors.Contains(nextUp))
                    Game.SchedulingSystem.Add(nextUp);
                AdvanceTurn();
            }
            else
            {
                // ISchedulables with no behaviors are very strange indeed...
                Game.SchedulingSystem.Add(nextUp);
                AdvanceTurn();
            }
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
            if(victim is Nucleus)
            { // how about only retreat if you're the last nucleus?
                Actor newVictim = (victim as Nucleus).Retreat();
                if(newVictim == null)
                {
                    // Game over!
                    Game.MessageLog.Add($"{victim.Name} had nowhere to run and was destroyed. Peace is restored.");
                }
                else
                {
                    Game.MessageLog.Add($"The {victim.Name} retreated into the nearby { newVictim.Name }, avoiding death.");
                    //Game.DMap.Swap(monster, newVictim);
                    Game.DMap.RemoveActor(newVictim);
                }
            }
            else
            {
                Game.MessageLog.Add($"A { victim.Name } is destroyed.");
                //Game.DMap.Swap(monster, victim);
                Game.DMap.RemoveActor(victim);
            }
        }

        public void Eat(Actor eating, Actor eaten)
        {
            // Also eat the item underneath, if it was present.
            if(eaten is IEatable e)
            { 
                Game.DMap.Swap(eating, eaten);
                e.OnEaten();
            }
            else
            {
                Game.DMap.RemoveActor(eaten);
            }
        }

        public bool Ingest(Actor eating, Item eaten)
        {
            if(eaten is IEatable)
            {
                List<Actor> candidates = new List<Actor>() { eating };
                List<Actor> seen = new List<Actor>();
                List<Actor> frontier = new List<Actor>();
                List<Cytoplasm> selection = new List<Cytoplasm>();
                if (eating is Cytoplasm)
                    selection.Add(eating as Cytoplasm);
                while (selection.Count == 0)
                {
                    seen.AddRange(candidates);
                    frontier.Clear();
                    foreach(Actor c in candidates)
                        frontier.AddRange(Game.PlayerMass.Where(a => a.AdjacentTo(c.X, c.Y) && !seen.Contains(a)));
                    candidates.Clear();
                    candidates.AddRange(frontier);
                    foreach (Actor potential in candidates)
                    {
                        if (potential is Cytoplasm)
                            selection.Add(potential as Cytoplasm);
                    }
                    if (candidates.Count == 0)
                        return false;
                }
                int pick = Game.Rand.Next(0, selection.Count - 1);
                Actor recepient = selection[pick];
                Point newOrganellePos = new Point(recepient.X, recepient.Y);
                Game.DMap.RemoveActor(recepient);
                eaten.X = recepient.X;
                eaten.Y = recepient.Y;
                (eaten as IEatable).OnEaten();
                
            }
            else
            {
                Game.DMap.RemoveItem(eaten);
            }
            MoveNucleus(eaten.X, eaten.Y);
            return true;
        }

        public bool Wait()
        {
            return true; // may need to do more stuff here.
        }

        // Return value is true if the player was able to move
        // false when the player couldn't move, such as trying to move into a wall
        public bool MovePlayer(Direction direction)
        {
            int x = Game.Player.X;
            int y = Game.Player.Y;

            Point mod = ApplyDirection(new Point(x, y), direction);
            x = mod.X;
            y = mod.Y;

            Actor targetActor = Game.DMap.GetActorAt(x, y);
            if (targetActor != null)
            {
                if (targetActor.Slime == true)
                {
                    // swap
                    Game.DMap.Swap(Game.Player, targetActor);
                    return true;
                }
                else if (targetActor is IEatable)
                {
                    Eat(Game.Player, targetActor);
                    return true;
                }
                
            }
            else
            {
                Item targetItem = Game.DMap.GetItemAt(x, y);
                if (targetItem != null)
                {
                    if (targetItem.Name == "Nutrient") // stupid constant string
                    {
                        Actor n = new Cytoplasm()
                        {
                            X = targetItem.X,
                            Y = targetItem.Y
                        };
                        Game.DMap.RemoveItem(targetItem);
                        Game.DMap.AddActor(n);
                        Game.PlayerMass.Add(n);
                        Game.DMap.Swap(Game.Player, n);
                        return true;
                    }
                }
                else // No actor and no item; move the player
                {
                    return MoveNucleus(x, y);
                }
            }


            return false;
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

        private static bool MoveNucleus(int x, int y)
        {
            int counter = 1;
            int max = 0;
            bool done = false;
            SlimePathfind root = new SlimePathfind(Game.Player, null, 0);
            List<SlimePathfind> last = new List<SlimePathfind>() { root };
            List<SlimePathfind> accountedFor = new List<SlimePathfind>() { root };
            while (!done)
            {
                List<SlimePathfind> frontier = new List<SlimePathfind>();
                foreach (SlimePathfind l in last)
                {
                    List<Actor> pullIn = Game.DMap.Actors.Where(a => a.Slime == true
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
            bool looping = true; // why can't you come up with better namescl
            while (looping)
            {
                path.Add(selected.current);
                selected = selected.dest;
                if (selected == null)
                    looping = false;
            }

            path.Reverse();

            Point lastPoint = new Point(Game.Player.X, Game.Player.Y);
            if (Game.DMap.SetActorPosition(Game.Player, x, y))
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
    }
}
