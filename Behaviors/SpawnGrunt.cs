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
        public int MilitiaWeight = 8;
        public int TankWeight = 2;
        public int HunterWeight = 100;

        public bool Act(TutorialMonster spawner, CommandSystem commandSystem)
        {

            TurnsToSpawn--;
            if (TurnsToSpawn <= 0)
            { // Spawn a monster
                List<ICell> spawnAreas = Game.DMap.AdjacentWalkable(spawner.X, spawner.Y);
                if (spawnAreas.Count > 0)
                {
                    Militia baby = NewMilitia();
                    baby.X = spawnAreas[0].X;
                    baby.Y = spawnAreas[0].Y;
                    Game.DMap.AddActor(baby);
                    TurnsToSpawn += Game.SpawnRate;
                    TankWeight++;
                    HunterWeight = TankWeight / 2;
                }
            }
            return true;
        }

        private Militia NewMilitia()
        {
            int sel = Game.Rand.Next(MilitiaWeight + TankWeight + HunterWeight - 1);
            if(sel < MilitiaWeight)
            {
                return new Militia();
            }
            else if(sel < MilitiaWeight + TankWeight)
            {
                return new Tank();
            }
            else
            {
                return new Hunter();
            }
        }
    }
}
