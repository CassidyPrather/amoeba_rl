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
        public int WaveRate { get; set; } = Game.DefaultSpawnRate;

        public int TurnsToNextWave { get; set; } = Game.DefaultSpawnRate;

        public int ScoutCost { get; set; } = 2;
        
        public int HunterCost { get; set; } = 3;

        public int TankCost { get; set; } = 2;

        public int MechCost { get; set; } = 3;

        public int WaveNumber { get; set; } = 0;

        public int CityLevel { get; set; } = 1;

        Queue<Actor> SpawnQueue { get; set; } = new Queue<Actor>();

        public SpawnTimer Timer { get; protected set; } = null;

        public City()
        {
            Awareness = 0;
            Symbol = 'C';
            Name = "City Gate";
            Color = Palette.City;
            Speed = 16;
            Unforgettable = true;
        }

        public bool Act()
        {
            TurnsToNextWave--;
            ConfigureTimer();
            if (TurnsToNextWave <= 0)
            {
                // Dispatch wave wave #
                SpawnNextWave(CityLevel);
                // Set the city level based on the wave number
                CityLevel = (WaveNumber / Game.EvolutionRate) + 2;
            }
            if (SpawnQueue.Count > 0)
            {
                List<ICell> spawnAreas = Game.DMap.AdjacentWalkable(X, Y);
                if (spawnAreas.Count > 0)
                {
                    Actor baby = SpawnQueue.Dequeue();
                    baby.X = spawnAreas[0].X;
                    baby.Y = spawnAreas[0].Y;
                    Game.DMap.AddActor(baby);
                    if(SpawnQueue.Count == 0)
                    {
                        Game.DMap.RemoveVFX(Timer);
                        Timer = null;
                    }
                }
            }
            return true;
        }

        public void SpawnNextWave(int budget)
        {
            int currentWaveStock = budget;
            while (currentWaveStock > 0)
                currentWaveStock = AddNewMilitia(currentWaveStock);
            // Roll for caravan
            bool waveHasCaravan;
            if (WaveNumber == 0)
                waveHasCaravan = Game.Rand.Next(3) == 0;
            else if(WaveNumber < 4)
                waveHasCaravan = Game.Rand.Next(19) <= 2;
            else
                waveHasCaravan = Game.Rand.Next(19) == 0;
            if (waveHasCaravan)
                SpawnQueue.Enqueue(new Caravan());
            WaveNumber++;
            TurnsToNextWave += WaveRate;

        }


        protected virtual int AddNewMilitia(int budget)
        {
            List<int> allowedSpawnTypes = new List<int>() { 0 };
            if (budget >= MechCost)
                allowedSpawnTypes.Add(1);
            else if (budget >= TankCost)
                allowedSpawnTypes.Add(2);
            if (budget >= HunterCost)
                allowedSpawnTypes.Add(3);
            else if (budget >= ScoutCost)
                allowedSpawnTypes.Add(4);
            int spawnType = allowedSpawnTypes[Game.Rand.Next(allowedSpawnTypes.Count - 1)];
            if(spawnType == 0)
            {
                for (int i = 0; i < Math.Min(budget, MechCost); i++)
                    SpawnQueue.Enqueue(new Militia());
                return budget - Math.Min(budget, MechCost);
            }
            else if(spawnType == 1)
            {
                SpawnQueue.Enqueue(new Mech());
                return budget - MechCost;
            }
            else if (spawnType == 2)
            {
                SpawnQueue.Enqueue(new Tank());
                return budget - TankCost;
            }
            else if (spawnType == 3)
            {
                SpawnQueue.Enqueue(new Hunter());
                return budget - HunterCost;
            }
            else if (spawnType == 4)
            {
                SpawnQueue.Enqueue(new Scout());
                return budget - ScoutCost;
            }



            return 0; // Nothing spawned???
        }

        private void ConfigureTimer()
        {
            if (TurnsToNextWave < 10 || SpawnQueue.Count > 0)
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
                if (!(SpawnQueue.Count > 0))
                {
                    Timer.HasQueue = false;
                    Timer.T = TurnsToNextWave;
                }
                else
                {
                    Timer.HasQueue = true;
                    Timer.T = SpawnQueue.Count;
                }
            }
        }


        public string GetDescription()
        {
            StringBuilder desc = new StringBuilder();
            desc.Append("A doorway to one of the last bastions of humanity. It is protected by advanced technology and can never be destroyed. ");
            if (SpawnQueue.Count > 0)
            {
                if (SpawnQueue.Count == 1)
                    desc.Append("One human is waiting to emerge onto an adjacent space as soon as one becomes available. ");
                else
                    desc.Append($"{SpawnQueue.Count} humans are in line to emerge onto ajacent tiles as soon as one becomes available. ");

                if (CityLevel == 1)
                    desc.Append($"A human will join the queue in {TurnsToNextWave}. ");
                else
                    desc.Append($"In {TurnsToNextWave} more turns, up to {CityLevel} humans will join the queue.");
            }
            else
            {
                if (CityLevel == 1)
                    desc.Append($"A human will try to emerge in {TurnsToNextWave} turns. ");
                else
                    desc.Append($"Up to {CityLevel} humans will begin to emerge in {TurnsToNextWave} turns. ");
            }
            desc.Append("As time goes on, the humans will become more frequent and deadly...");
            return desc.ToString();
        }

        public class SpawnTimer : Animation
        {
            public bool HasQueue = false;

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
                    if (T <= 9)
                        Symbol = (Math.Max(0, T)).ToString()[0];
                    else
                        Symbol = '*';
                    Color = Palette.ReticleForeground;
                    if (!HasQueue)
                        BackgroundColor = Palette.ReticleBackground;
                    else
                        BackgroundColor = Palette.Hunter;
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
