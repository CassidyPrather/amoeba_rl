
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

namespace AmoebaRL.Core.Enemies
{
    /// <summary>
    /// Spawns hostiles into the map. The fact that these get progressively harder is the game's "clock".
    /// </summary>
    public class City : Actor, IProactive, IDescribable
    {
        public int? WaveRate { get; set; } = null;

        public int TurnsToNextWave { get; set; } = 50;

        public int ScoutCost { get; set; } = 2;

        public int HunterCost { get; set; } = 3;

        public int TankCost { get; set; } = 2;

        public int MechCost { get; set; } = 3;

        public int WaveNumber { get; set; } = 0;

        public int CityLevel { get; set; } = 1;

        public Queue<Actor> SpawnQueue { get; set; } = new Queue<Actor>();

        private int? armor = null;

        public int Armor {
            get
            {
                if (!armor.HasValue)
                    armor = Map.Context.CityArmor;
                return armor.Value;
            } 
            set => armor = value;
        }

        public City()
        {
            Awareness = 0;
            Name = "City Gate";
            Delay = 16;
        }

        public void Act()
        {
            TurnsToNextWave--;
            if (TurnsToNextWave <= 0)
            {
                // Dispatch wave wave #
                SpawnNextWave(Math.Min(Map.Context.MaxBudget, CityLevel));
                // Set the city level based on the wave number
                CityLevel = (WaveNumber / Map.Context.EvolutionRate) + 2;
            }
            if (SpawnQueue.Count > 0)
            {
                List<ICell> spawnAreas = Map.AdjacentWalkable(X, Y);
                if (spawnAreas.Count > 0)
                {
                    Actor baby = SpawnQueue.Dequeue();
                    baby.X = spawnAreas[0].X;
                    baby.Y = spawnAreas[0].Y;
                    Map.AddActor(baby);
                }
            }
        }

        public void SpawnNextWave(int budget)
        {
            int currentWaveStock = budget;
            while (currentWaveStock > 0)
                currentWaveStock = AddNewMilitia(currentWaveStock);
            // Roll for caravan
            bool waveHasCaravan;
            if (WaveNumber == 0)
                waveHasCaravan = Map.Context.Rand.Next(3) == 0;
            else if (WaveNumber < 4)
                waveHasCaravan = Map.Context.Rand.Next(19) <= 2;
            else
                waveHasCaravan = Map.Context.Rand.Next(19) == 0;
            if (waveHasCaravan)
                SpawnQueue.Enqueue(new Caravan());
            WaveNumber++;
            if (!WaveRate.HasValue)
                WaveRate = Map.Context.DefaultSpawnRate;
            TurnsToNextWave += WaveRate.Value;

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
            int spawnType = allowedSpawnTypes[Map.Context.Rand.Next(allowedSpawnTypes.Count - 1)];
            if (spawnType == 0)
            {
                for (int i = 0; i < Math.Min(budget, MechCost); i++)
                    SpawnQueue.Enqueue(new Militia());
                return budget - Math.Min(budget, MechCost);
            }
            else if (spawnType == 1)
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

        public string Description
        {
            get
            {
                StringBuilder desc = new StringBuilder();
                desc.Append($"A doorway to one of the last bastions of humanity. It is protected by advanced " +
                    $"technology and can only be destroyed when the amoeba mass is at least {Armor}. ");
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
        }

        public void Destroy()
        {
            Map.RemoveCity(this);
            if(Map.Cities.Count == 0)
            {
                Map.Context.MessageLog.Add("The humans try to trigger a cave-in, but you slip through just in time! You escape to the surface and live out the rest of your days in peace. ");
                Map.Context.MessageLog.Add($"Final score: {Map.PlayerMass.Count}. Time to win (A turn is 16 time units): {Map.Context.SchedulingSystem.GetTime()}.");
                Map.Context.MessageLog.Add($"Thanks for playing!.");
                Map.Context.CommandSystem.Win();
                // Player wins!
            }
            else
            {
                Map.Context.MessageLog.Add("The humans trigger a cave-in, blocking off this exit to the surface!");
                Map.Context.MessageLog.Add($"{Map.Cities.Count} cities remain...");
            }
        }
    }
}
