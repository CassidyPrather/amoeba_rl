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
        public int SpawnRate { get; set; } = Game.DefaultSpawnRate;

        public int TurnsToSpawn { get; set; } = Game.DefaultSpawnRate;

        public int HunterCost { get; set; } = 3;

        public int TankCost { get; set; } = 3;

        public int NotOODWeight = 12;


        protected int Phase { get
            {
                if (WaveNumber >= Phase2Begin)
                    return 1;
                else
                    return 0;
            }
        }

        public int Phase2Begin { get; set; } = 5;

        public int WaveNumber { get; set; } = 0;

        public int WaveIntensity { get; set; } = 1;

        Queue<Actor> SpawnQueue { get; set; } = new Queue<Actor>();

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
            ConfigureTimer();
            if (TurnsToSpawn <= 0)
            {
                int currentWaveStock = WaveIntensity;
                while(currentWaveStock > 0)
                    currentWaveStock = AddNewMilitia(currentWaveStock);
                WaveNumber++;
                if (WaveNumber <= 2)
                    WaveIntensity = 2;
                else
                    WaveIntensity = 2 + (WaveNumber)/3;
                TurnsToSpawn += SpawnRate;
            }
            if(SpawnQueue.Count > 0)
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

        protected virtual int AddNewMilitia(int budget)
        {
            if(WaveNumber < 3)
            {
                // In the early waves, may go over-budget to spawn OOD.
                int gamble = Game.Rand.Next(NotOODWeight + WaveNumber + 1);
                if(gamble > NotOODWeight)
                {
                    int pick = Game.Rand.Next(1);
                    if (pick == 0)
                        SpawnQueue.Enqueue(new Tank());
                    else
                        SpawnQueue.Enqueue(new Hunter());
                    return 0; // Use up all of the budget if this is done.
                }
                else
                {
                    SpawnQueue.Enqueue(new Militia());
                    return budget - 1;
                }
            }

            int mostExpensive = Math.Max(TankCost, HunterCost);
            int spawnType = Game.Rand.Next(2);
            if(spawnType == 0 || budget < mostExpensive)
            {
                for(int i = 0; i < Math.Min(budget, mostExpensive); i++)
                    SpawnQueue.Enqueue(new Militia());
                return Math.Max(budget - mostExpensive, 0);
            }
            else if(spawnType == 1)
            {
                SpawnQueue.Enqueue(new Tank());
                return budget - TankCost;
            }
            else if(spawnType == 2)
            {
                SpawnQueue.Enqueue(new Hunter());
                return budget - HunterCost;
            }

            return 0; // Nothing spawned???
        }

        private void ConfigureTimer()
        {
            if (TurnsToSpawn < 10 || SpawnQueue.Count > 0)
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
                    Timer.T = TurnsToSpawn;
                }
                else
                {
                    Timer.HasQueue = true;
                    Timer.T = SpawnQueue.Count;
                }
            }
        }

        protected void Evolve()
        {
            WaveNumber++;
        }

        public string GetDescription()
        {
            StringBuilder desc = new StringBuilder();
            desc.Append("One of the last bastions of humanity. It is protected by advanced technology and can never be destroyed. ");
            if (SpawnQueue.Count > 0)
            {
                if (SpawnQueue.Count == 1)
                    desc.Append("One human is waiting to emerge onto an adjacent space as soon as one becomes available. ");
                else
                    desc.Append($"{SpawnQueue.Count} humans are in line to emerge onto ajacent tiles as soon as one becomes available. ");

                if (WaveIntensity == 1)
                    desc.Append($"A human will join the queue in {TurnsToSpawn}. ");
                else if (WaveIntensity < Math.Max(HunterCost, TankCost))
                    desc.Append($"In {TurnsToSpawn} more turns, up to {WaveIntensity} humans will join the queue.");
                else
                    desc.Append($"In {TurnsToSpawn} more turns, Between {WaveIntensity / Math.Max(HunterCost, TankCost)} and {WaveIntensity} humans will join the queue. ");
            }
            else
            {
                if (WaveIntensity == 1)
                    desc.Append($"A human will try to emerge in {TurnsToSpawn} turns. ");
                else if (WaveIntensity < Math.Max(HunterCost, TankCost))
                    desc.Append($"Up to {WaveIntensity} humans will begin to emerge in {TurnsToSpawn} turns. ");
                else
                    desc.Append($"Between {WaveIntensity / Math.Max(HunterCost, TankCost)} and {WaveIntensity} humans will begin to emerge in {TurnsToSpawn} turns. ");

            }
            desc.Append("As time goes on, the humans will grow more frequent and deadly...");
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
