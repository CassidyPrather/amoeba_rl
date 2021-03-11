using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using RLNET;
using RogueSharp;
using AmoebaRL.Interfaces;
using AmoebaRL.UI;

namespace AmoebaRL.Core
{
    public class Actor : IActor, IDrawable, ISchedulable
    {
        // IActor
        public string Name { get; set; }

        public int Awareness { get; set; }

        public int Speed { get; set; } = 1;

        public bool Slime { get; set; }

        public bool Unforgettable { get; set; } = false;

        // IDrawable
        public RLColor Color { get; set; }

        public char Symbol { get; set; }

        public int X { get; set; }

        public int Y { get; set; }

        public void Draw(RLConsole console, IMap map)
        {
            // Don't draw actors in cells that haven't been explored
            if (!map.GetCell(X, Y).IsExplored)
            {
                return;
            }
            
            // Only draw the actor with the color and symbol when they are in field-of-view
            if (map.IsInFov(X, Y) || Unforgettable)
            {
                if(Slime)
                    console.Set(X, Y, Color, Palette.Slime, Symbol);
                else
                    console.Set(X, Y, Color, Palette.FloorBackgroundFov, Symbol);
            }
            else
            {
                // When not in field-of-view just draw a normal floor
                console.Set(X, Y, Palette.Floor, Palette.FloorBackground, '.');
            }
        }

        // ISchedulable
        public int Time
        {
            get
            {
                return Speed;
            }
        }

        #region Helpers
        public FieldOfView FOV()
        {
            // When replacing static handles, Instances of Game.DMap
            // should instead be replaced by object references to this class.
            FieldOfView selfFOV = new FieldOfView(Game.DMap);
            selfFOV.ComputeFov(X, Y, Awareness, true);
            return selfFOV;
        }

        // public List<Actor> Seen() => Seen(Game.PlayerMass);

        public List<Actor> Seen(List<Actor> testSightTo)
        {
            List<Actor> seenTargets = new List<Actor>();
            FieldOfView selfFOV = FOV();
            foreach (Actor a in testSightTo)
            {
                if (selfFOV.IsInFov(a.X, a.Y))
                    seenTargets.Add(a);
            }
            return seenTargets;
        }

        public List<Path> PathsTo(List<Actor> potentialTargets)
        {
            List<Path> results = new List<Path>();
            Path attempt;
            foreach (Actor candidate in potentialTargets)
            {
                attempt = null;
                try
                {
                    attempt = DungeonMap.QuickShortestPath(Game.DMap,
                    Game.DMap.GetCell(X, Y),
                    Game.DMap.GetCell(candidate.X, candidate.Y));
                }
                catch (PathNotFoundException) { }
                if (attempt != null)
                    results.Add(attempt);
            }
            return results;
        }

        public List<Path> PathsToNearest(List<Actor> potentialTargets)
        {
            List<Path> nearestPaths = new List<Path>();
            Path attempt;
            int nearestTargetDistance = int.MaxValue;
            foreach (Actor candidate in potentialTargets)
            {
                attempt = null;
                try
                {
                    attempt = DungeonMap.QuickShortestPath(Game.DMap,
                    Game.DMap.GetCell(X, Y),
                    Game.DMap.GetCell(candidate.X, candidate.Y));
                }
                catch (PathNotFoundException) { }
                if (attempt != null)
                {
                    if (attempt.Length <= nearestTargetDistance)
                    {
                        if (attempt.Length < nearestTargetDistance)
                        {
                            nearestPaths.Clear();
                            nearestTargetDistance = attempt.Length;
                        }
                        nearestPaths.Add(attempt);
                    }
                }
            }
            return nearestPaths;
        }

        public bool AdjacentTo(int tx, int ty)
        {
            if (Math.Abs(X - tx) + Math.Abs(Y - ty) == 1)
                return true;
            return false;
        }
        #endregion
    }
}
