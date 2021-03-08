using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using RogueSharp;
using RLNET;
using AmoebaRL.Core;

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
