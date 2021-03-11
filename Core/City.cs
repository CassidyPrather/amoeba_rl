using AmoebaRL.Behaviors;
using AmoebaRL.Interfaces;
using AmoebaRL.Systems;
using AmoebaRL.UI;
using RLNET;
using RogueSharp;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Core
{
    /// <summary>
    /// Spawns hostiles into the map. The fact that these get progressively harder is the game's "clock".
    /// </summary>
    public class City : Actor, IProactive, IDescribable
    {
        public int SpawnRate = Game.DefaultSpawnRate;
        public int TurnsToSpawn = Game.DefaultSpawnRate;
        public int MilitiaWeight = 12;
        public int TankWeight = 1;
        public int HunterWeight = 1;

        public SpawnTimer Timer { get; protected set; } = null;

        public City()
        {
            Awareness = 0;
            Symbol = 'C';
            Name = "City";
            Color = Palette.City;
            Speed = 16;
            Unforgettable = true;
        }

        public bool Act()
        {
            TurnsToSpawn--;
            if(TurnsToSpawn < 10)
            {
                if (Timer == null)
                {
                    Timer = new SpawnTimer
                    {
                        X = X,
                        Y = Y
                    };
                    Game.DMap.AddVFX(Timer);
                }
                Timer.T = TurnsToSpawn;
            }
            if (TurnsToSpawn <= 0)
            { // Spawn a monster
                List<ICell> spawnAreas = Game.DMap.AdjacentWalkable(X, Y);
                if (spawnAreas.Count > 0)
                {
                    Militia baby = NewMilitia();
                    baby.X = spawnAreas[0].X;
                    baby.Y = spawnAreas[0].Y;
                    Game.DMap.AddActor(baby);
                    TurnsToSpawn += Game.DefaultSpawnRate;
                    TankWeight++;
                    HunterWeight = TankWeight / 2;
                    Game.DMap.RemoveVFX(Timer);
                    Timer = null;
                }
            }
            return true;
        }

        private Militia NewMilitia()
        {
            int sel = Game.Rand.Next(MilitiaWeight + TankWeight + HunterWeight - 1);
            if (sel < MilitiaWeight)
            {
                return new Militia();
            }
            else if (sel < MilitiaWeight + TankWeight)
            {
                return new Tank();
            }
            else
            {
                return new Hunter();
            }
        }

        public string GetDescription()
        {
            return $"One of the last bastions of humanity. It is protected by advanced technology and can never be destroyed." +
                $"A human will emerge to fight in {TurnsToSpawn} turns if there is room. As time goes on, the humans will grow more frequent and deadly...";
        }

        public class SpawnTimer : Animation
        {
            public int T { get; set; } = 9;

            public SpawnTimer()
            {
                Symbol = '9';
                Color = Palette.ReticleForeground;
                BackgroundColor = Palette.ReticleBackground;
                Speed = 3;
                Frames = 2;
                AlwaysVisible = true;
            }

            public override void SetFrame(int idx)
            {
                if (idx == 0)
                {
                    Symbol = (Math.Max(0, T)).ToString()[0];
                    Color = Palette.ReticleForeground;
                    BackgroundColor = Palette.ReticleBackground;
                }
                else
                {
                    Symbol = 'C';
                    Color = Palette.City;
                    BackgroundColor = Palette.FloorBackgroundFov;
                }
            }

            public override void Draw(RLConsole console, IMap map)
            {
                if (map.IsExplored(X, Y))
                    base.Draw(console, map);
            }
        }
    }
}
