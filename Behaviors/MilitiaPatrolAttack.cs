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
    class MilitiaPatrolAttack : IBehavior
    {
        public bool Act(Monster monster, CommandSystem commandSystem)
        {
            DungeonMap dMap = Game.DMap;
            FieldOfView monsterFov = new FieldOfView(dMap);

            List<Actor> seenTargets = new List<Actor>();

            // If the monster has not been alerted, compute a field-of-view
            // Use the monster's Awareness value for the distance in the FoV check
            // If the player is in the monster's FoV then alert it
            // Add a message to the MessageLog regarding this alerted status

            monsterFov.ComputeFov(monster.X, monster.Y, monster.Awareness, true);
            foreach (Actor a in Game.PlayerMass)
            {
                if (monsterFov.IsInFov(a.X, a.Y))
                    seenTargets.Add(a);
            }

            if (seenTargets.Count > 0)
            {
                //List<Actor> nearestTargets = new List<Actor>(); // these two should be a tuple
                List<Path> nearestPaths = new List<Path>();
                Path attempt;
                int nearestTargetDistance = int.MaxValue;
                foreach (Actor candidate in seenTargets)
                {
                    attempt = null;
                    try
                    {
                        attempt = DungeonMap.QuickShortestPath(dMap,
                        dMap.GetCell(monster.X, monster.Y),
                        dMap.GetCell(candidate.X, candidate.Y));
                    }
                    catch (PathNotFoundException)
                    {
                        //Game.MessageLog.Add("Couldn't path to the candidate.");
                    }
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
                            //nearestTargets.Add(candidate);
                        }
                    }
                }



                // In the case that there was a path, tell the CommandSystem to move the monster
                if (nearestPaths.Count > 0)
                {
                    int pick = Game.Rand.Next(0, nearestPaths.Count - 1);
                    //Actor nearestTarget = nearestTargets[pick];
                    Path nearestPath = nearestPaths[pick];
                    try
                    {
                        // TODO: This should be path.StepForward() but there is a bug in RogueSharp V3
                        // The bug is that a Path returned from a PathFinder does not include the source Cell
                        commandSystem.MoveMonster(monster, nearestPath.StepForward()); //path.Steps.First()
                    }
                    catch (NoMoreStepsException)
                    {
                        Game.MessageLog.Add($"{monster.Name} growls in frustration");
                    }
                }
                else
                {
                    Game.MessageLog.Add($"{monster.Name} waits for a turn");
                }
            }
            else
            {
                
                List<ICell> adj = dMap.AdjacentWalkable(monster.X, monster.Y);
                int pick = Game.Rand.Next(0, adj.Count);
                if (pick != adj.Count)
                    commandSystem.MoveMonster(monster, adj[pick]);
                
            }
            return true;
        }
    }
}
