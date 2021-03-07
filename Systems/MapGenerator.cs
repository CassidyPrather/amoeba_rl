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

    class MapGenerator
    {
        private readonly int _width;
        private readonly int _height;
        private readonly int _mapBoulders;
        private readonly int _boulderMaxSize;
        private readonly int _boulderMinSize;

        private readonly DungeonMap _map;

        // Constructing a new MapGenerator requires the dimensions of the maps it will create
        // as well as the sizes and maximum number of rooms
        public MapGenerator(int width, int height,
        int maxBoulders, int boulderMaxSize, int boulderMinSize)
        {
            _width = width;
            _height = height;
            _mapBoulders = maxBoulders;
            _boulderMaxSize = boulderMaxSize;
            _boulderMinSize = boulderMinSize;
            _map = new DungeonMap();
        }

        // Generate a new map that places rooms randomly
        public DungeonMap CreateMap()
        {
            // Set the properties of all cells to false
            _map.Initialize(_width, _height);
            Arena(); // Build a box full of boulders.

            PlaceBoulders();

            ConnectPockets();

            PlacePlayer();

            return _map;
        }

        /// <summary>
        /// Connects all pockets of space which cannot be walked to from each other.
        /// </summary>
        private void ConnectPockets()
        {
            List<List<ICell>> pockets = CalculatePockets();
            List<List<Tuple<ICell, ICell, int>>> bestBridges = GenerateBestPocketBridges(pockets);
            // Find the shortest bridge between all pockets.
            while (pockets.Count > 1)
            {
                Tuple<int, int> mergePockets = new Tuple<int, int>(0, 1);
                Tuple<ICell, ICell, int> toMerge = bestBridges[0][0];
                for (int i = 0; i < bestBridges.Count; i++)
                {
                    for (int j = 0; j < bestBridges[i].Count; j++)
                    {
                        if (bestBridges[i][j].Item3 < toMerge.Item3)
                        {
                            toMerge = bestBridges[i][j];
                            mergePockets = new Tuple<int, int>(i, i + j + 1);
                        }
                    }
                }
                // Connect and merge
                RandomElbowTunnel(toMerge.Item1, toMerge.Item2);
                pockets[mergePockets.Item1].AddRange(pockets[mergePockets.Item2]);
                pockets.RemoveAt(mergePockets.Item2);
                // Parent pocket steals best bridges from consumed pocket.
                // Bridges pointing to the deleted pocket
                for(int i = 0; i < mergePockets.Item2; i++)
                {
                    if(i < mergePockets.Item1)
                    { // Point to the parent using the stolen bridge
                        Tuple<ICell, ICell, int> stolenBridge = bestBridges[i][mergePockets.Item2 - i - 1];
                        Tuple<ICell, ICell, int> toReplace = bestBridges[i][mergePockets.Item1 - i - 1];
                        if (stolenBridge.Item3 < toReplace.Item3)
                            bestBridges[i][mergePockets.Item1 - i - 1] = stolenBridge;
                    }

                    bestBridges[i].RemoveAt(mergePockets.Item2 - i - 1);
                }
                // Bridges pointing from the deleted pocket.
                if(bestBridges.Count > 0 && mergePockets.Item2 < bestBridges.Count)
                { 
                    for(int i = 0; i < bestBridges[mergePockets.Item2].Count; i++)
                    {
                        Tuple<ICell, ICell, int> stolenBridge = bestBridges[mergePockets.Item2][i];
                        Tuple<ICell, ICell, int> toReplace = bestBridges[mergePockets.Item1][mergePockets.Item2 + i];
                        if (stolenBridge.Item3 < toReplace.Item3)
                            bestBridges[mergePockets.Item1][mergePockets.Item2 + i] = stolenBridge;
                    }
                    bestBridges.RemoveAt(mergePockets.Item2);
                }
            }
        }

        private List<List<ICell>> CalculatePockets()
        {
            List<List<ICell>> pockets = new List<List<ICell>>();
            int[] lastColIdx = new int[_map.Height];
            for (int x = 0; x < _map.Width; x++)
            {
                int currentPocketIndex = -1;
                for (int y = 0; y < _map.Height; y++)
                {
                    ICell current = _map.GetCell(x, y);
                    if (current.IsWalkable)
                    {
                        int lastCol = lastColIdx[y];
                        if (currentPocketIndex == -1) // Wall above
                        {
                            if (x > 0)
                            {
                                if (lastCol != -1) // Wall above, pocket left
                                {
                                    pockets[lastCol].Add(current);
                                    currentPocketIndex = lastCol;
                                }
                                else // Wall above, wall left
                                {
                                    currentPocketIndex = pockets.Count;
                                    lastColIdx[y] = currentPocketIndex;
                                    pockets.Add(new List<ICell> { current });
                                }
                            }
                            else // Wall above, wall left.
                            {
                                currentPocketIndex = pockets.Count;
                                lastColIdx[y] = currentPocketIndex;
                                pockets.Add(new List<ICell> { current });
                            }
                        }
                        else // Pocket above.
                        {
                            pockets[currentPocketIndex].Add(current);

                            if (x > 0 && lastCol != -1 && lastCol != currentPocketIndex)
                            { // Different pockets above and left; merge
                                pockets[lastCol].AddRange(pockets[currentPocketIndex]);
                                pockets.RemoveAt(currentPocketIndex);
                                for (int i = 0; i < lastColIdx.Length; i++)
                                { 
                                    
                                    if(lastColIdx[i] == currentPocketIndex)
                                    {
                                        if(currentPocketIndex > lastColIdx[y])
                                        {
                                            lastColIdx[i] = lastColIdx[y];
                                        }
                                        else
                                        {
                                            lastColIdx[i] = lastColIdx[y] - 1;
                                        }
                                    }
                                    else if (lastColIdx[i] > currentPocketIndex)
                                        lastColIdx[i]--;
                                }
                                currentPocketIndex = lastColIdx[y];
                            }
                            lastColIdx[y] = currentPocketIndex;
                        }
                    }
                    else // this is a wall.
                    {
                        lastColIdx[y] = -1;
                        currentPocketIndex = -1;
                    }
                }
            }
            return pockets;
        }

        private static List<List<Tuple<ICell, ICell, int>>> GenerateBestPocketBridges(List<List<ICell>> pockets)
        {
            // Connect with bridges.
            List<List<Tuple<ICell, ICell, int>>> bestBridges = new List<List<Tuple<ICell, ICell, int>>>();
            for (int i = 0; i < pockets.Count - 1; i++)
            {
                List<Tuple<ICell, ICell, int>> iOut = new List<Tuple<ICell, ICell, int>>();
                for (int j = i+1; j < pockets.Count; j++)
                {
                    List<ICell> from = pockets[i];
                    List<ICell> to = pockets[j];
                    Tuple<ICell, ICell, int> bestBridge = new Tuple<ICell, ICell, int>(from[0], to[0], DungeonMap.TaxiDistance(from[0], to[0]));
                    foreach (ICell f in pockets[i])
                    {
                        foreach (ICell t in pockets[j])
                        {
                            int newDistance = DungeonMap.TaxiDistance(f, t);
                            if (newDistance < bestBridge.Item3)
                                bestBridge = new Tuple<ICell, ICell, int>(f, t, newDistance);
                        }
                    }
                    iOut.Add(bestBridge);
                }
                bestBridges.Add(iOut);
            }

            return bestBridges;
        }

        private void RandomElbowTunnel(ICell from, ICell to)
        {
            if (Game.Rand.Next(0, 1) == 0)
            {
                CreateHorizontalTunnel(from.X, to.X, from.Y);
                CreateVerticalTunnel(from.Y, to.Y, from.X);
            }
            else
            {
                CreateVerticalTunnel(from.Y, to.Y, from.X);
                CreateHorizontalTunnel(from.X, to.X, from.Y);
            }
        }

        // Carve a tunnel out of the map parallel to the x-axis
        private void CreateHorizontalTunnel(int xStart, int xEnd, int yPosition)
        {
            for (int x = Math.Min(xStart, xEnd); x <= Math.Max(xStart, xEnd); x++)
            {
                _map.SetCellProperties(x, yPosition, true, true);
            }
        }

        // Carve a tunnel out of the map parallel to the y-axis
        private void CreateVerticalTunnel(int yStart, int yEnd, int xPosition)
        {
            for (int y = Math.Min(yStart, yEnd); y <= Math.Max(yStart, yEnd); y++)
            {
                _map.SetCellProperties(xPosition, y, true, true);
            }
        }

        private void PlaceBoulders()
        {
            // Try to place as many rooms as the specified maxRooms
            // Note: Only using decrementing loop because of WordPress formatting
            for (int r = _mapBoulders; r > 0; r--)
            {
                // Determine the size and position of the room randomly
                int boulderWidth = Game.Rand.Next(_boulderMinSize, _boulderMaxSize);
                int boulderHeight = Game.Rand.Next(_boulderMinSize, _boulderMaxSize);
                int boulderXPosition = Game.Rand.Next(0, _width - boulderWidth - 1);
                int boulderYPosition = Game.Rand.Next(0, _height - boulderHeight - 1);

                // All of our rooms can be represented as Rectangles
                var newRoom = new Rectangle(boulderXPosition, boulderYPosition,
                  boulderWidth, boulderHeight);

                // TODO allow boudler intersection?

                // Check to see if the room rectangle intersects with any other rooms
                bool newRoomIntersects = _map.Boulders.Any(room => newRoom.Intersects(room));

                // As long as it doesn't intersect add it to the list of rooms
                if (!newRoomIntersects)
                {
                    _map.Boulders.Add(newRoom);
                }
            }
            // Iterate through each room that we wanted placed
            // call CreateRoom to make it
            foreach (Rectangle boulder in _map.Boulders)
            {
                AddBoulder(boulder);
            }
        }

        private void Arena()
        {
            foreach (Cell cell in _map.GetAllCells())
            {
                _map.SetCellProperties(cell.X, cell.Y, true, true, false);
            }

            // Set the first and last rows in the map to not be transparent or walkable
            foreach (Cell cell in _map.GetCellsInRows(0, _height - 1))
            {
                _map.SetCellProperties(cell.X, cell.Y, false, false, false);
            }

            // Set the first and last columns in the map to not be transparent or walkable
            foreach (Cell cell in _map.GetCellsInColumns(0, _width - 1))
            {
                _map.SetCellProperties(cell.X, cell.Y, false, false, false);
            }

            // Set the first and last columns in the map to not be transparent or walkable
            foreach (Cell cell in _map.GetCellsInColumns(_width/2, _width / 2))
            {
                _map.SetCellProperties(cell.X, cell.Y, false, false, false);
            }
        }

        // Given a rectangular area on the map
        // set the cell properties for that area to true
        private void AddBoulder(Rectangle boulder)
        {
            for (int x = boulder.Left; x <= boulder.Right; x++)
            {
                for (int y = boulder.Top; y <= boulder.Bottom; y++)
                {
                    _map.SetCellProperties(x, y, false, false, false);
                }
            }
        }

        // Place the nucleus at a random spot.
        private void PlacePlayer()
        {
            Nucleus player = Game.Player;
            if (player == null)
            {
                player = new Nucleus();
            }

            do
            {
                player.X = Game.Rand.Next(0, _width - 1);
                player.Y = Game.Rand.Next(0, _height - 1);
            } while (!_map.GetCell(player.X,player.Y).IsWalkable);
            
            _map.AddPlayer(player);
        }
    }
}
