using AmoebaRL.Core;
using AmoebaRL.Interfaces;
using AmoebaRL.Systems;
using RogueSharp;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Behaviors
{
    class SpawnGrunt : IBehavior
    {
        public int TurnsToSpawn = Game.SpawnRate;

        public bool Act(Monster spawner, CommandSystem commandSystem)
        {
            
            TurnsToSpawn--;
            if (TurnsToSpawn <= 0)
            { // Spawn a monster
                List<ICell> spawnAreas = Game.DMap.AdjacentWalkable(spawner.X, spawner.Y);
                if(spawnAreas.Count > 0)
                {
                    Militia baby = new Militia()
                    {
                        X = spawnAreas[0].X,
                        Y = spawnAreas[0].Y
                    };
                    Game.DMap.AddActor(baby);
                    TurnsToSpawn += Game.SpawnRate;
                }
            }
            return true;
        }
    }
}
