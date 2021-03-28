using AmoebaRL.Core;
using RLNET;
using RogueSharp;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.UI
{
    /// <summary>
    /// An ASCII-based graphical rendering engine.
    /// </summary>
    public class ASCIIGraphics : GraphicalSystem
    {
        // Constants may need to be fiddled with, should be automated where possible.
        #region Constants
        #region Sizes
        private static readonly int _fontWidth = 12;
        private static readonly int _fontHeight = 12;
        #endregion

        private static readonly TimeSpan ANIMATION_RATE = TimeSpan.FromMilliseconds(250);

        private static readonly string _fontFileName = "terminal12x12_gs_ro.png";

        private static readonly string _winTitle = "Amoeba RL";

        public TimeSpan TimeSinceLastAnimation = TimeSpan.Zero;

        #endregion

        #region Consoles
        private RLRootConsole RootConsole { get; set; }
        private MapConsole MapCanvas { get; set; }
        private InfoConsole InfoCanvas { get; set; }
        private PlayerConsole PlayerCanvas { get; set; }
        #endregion


        public int AnimationFrame = 0;

        public List<TextTile> Tiles { get; set; } = new List<TextTile>();

        private bool _renderRequired = true;

        private DateTime _lastGraphicalTime;


        /// <inheritdoc/>
        public ASCIIGraphics(Game toShow) : base(toShow)
        {
            
            MapCanvas = new MapConsole();
            PlayerCanvas = new PlayerConsole();
            InfoCanvas = new InfoConsole();
            RootConsole = new RLRootConsole(_fontFileName, MapCanvas.Width + PlayerCanvas.Width,
                                             MapCanvas.Height + InfoCanvas.Height, _fontWidth, _fontHeight, 1f,
                                             _winTitle);
            
            _lastGraphicalTime = DateTime.UtcNow;
            // Set up a handler for RLNET's Update event
            RootConsole.Update += OnRootConsoleUpdate;
            // Set up a handler for RLNET's Render event
            RootConsole.Render += OnRootConsoleRender;

        }

        public override void Run() => RootConsole.Run();

        private void OnRootConsoleUpdate(object sender, UpdateEventArgs e)
        {
            RLKeyPress keyPress = RootConsole.Keyboard.GetKeyPress();

            _renderRequired = Showing.HandleUserInput(keyPress);

            MapCanvas.OnUpdate(sender, e);
            InfoCanvas.OnUpdate(sender, e);
            PlayerCanvas.OnUpdate(sender, e);
        }

        /// <inheritdoc/>
        public override void End()
        {
            RootConsole.Close();
        }

        private void OnRootConsoleRender(object sender, UpdateEventArgs e)
        {
            // Update Main Game
            if (NeedsAnimationUpdate() || _renderRequired)
            {
                // Get graphical tiles and animations for everything in the game.
                // Until the map class adds such a feature, there is no way to automatically update tracking in graphical tiles
                GenerateRepresentation();
                RenderMapBase(MapCanvas);
                InfoCanvas.DrawContent(Showing);
                PlayerCanvas.DrawContent(Showing);

                RLConsole.Blit(MapCanvas, 0, 0, MapCanvas.Width, MapCanvas.Height, RootConsole, 0, 0);
                RLConsole.Blit(InfoCanvas, 0, 0, InfoCanvas.Width, InfoCanvas.Height, RootConsole, 0, MapCanvas.Height);
                RLConsole.Blit(PlayerCanvas, 0, 0, PlayerCanvas.Width, PlayerCanvas.Height, RootConsole, MapCanvas.Width, 0);

                RootConsole.Draw(); // Must come after "inner draws"

                _renderRequired = false;
            }
        }

        protected virtual void RenderMapBase(RLConsole mapConsole)
        {
            mapConsole.Clear();
            foreach (Cell cell in Showing.DMap.GetAllCells())
            {
                SetConsoleSymbolBackground(mapConsole, cell);
            }
            foreach(TextTile t in Tiles)
            {
                t.Animate(mapConsole, AnimationFrame);
            }
        }

        /// <summary>
        /// Determines whether the graphics need to be refreshed in accordance with frames passing and animations requiring updates.
        /// Manages <see cref="TimeSinceLastAnimation"/>.
        /// </summary>
        /// <param name="delta"></param>
        /// <returns></returns>
        public bool NeedsAnimationUpdate()
        {
            // Get animation data.
            DateTime renderInstant = DateTime.UtcNow;
            TimeSpan delta = renderInstant - _lastGraphicalTime;
            _lastGraphicalTime = renderInstant;
            TimeSinceLastAnimation += delta;
            if (TimeSinceLastAnimation >= ANIMATION_RATE)
            {
                AnimationFrame++;
                TimeSinceLastAnimation = TimeSpan.Zero;
                return true;
            }
            return false;
        }

        /// <summary>
        /// Set the default state of the map to draw.
        /// Mainly from the RogueSharp tutorial: <see href="https://faronbracy.github.io/RogueSharp/articles/05_simple_map_drawing.html"/>
        /// </summary>
        /// <param name="console">The canvas to draw on.</param>
        /// <param name="cell">The location to update the representation of.</param>
        private void SetConsoleSymbolBackground(RLConsole console, Cell cell)
        {
            // When we haven't explored a cell yet, we don't want to draw anything
            if (!cell.IsExplored)
            {
                return;
            }

            // When a cell is currently in the field-of-view it should be drawn with ligher colors
            if (Showing.DMap.IsInFov(cell.X, cell.Y))
            {
                // Choose the symbol to draw based on if the cell is walkable or not
                // '.' for floor and '#' for walls
                if (cell.IsWalkable)
                {
                    console.Set(cell.X, cell.Y, Palette.FloorFov, Palette.FloorBackgroundFov, '.');
                }
                else
                {
                    console.Set(cell.X, cell.Y, Palette.WallFov, Palette.WallBackgroundFov, '#');
                }
            }
            // When a cell is outside of the field of view draw it with darker colors
            else
            {
                if (cell.IsWalkable)
                {
                    console.Set(cell.X, cell.Y, Palette.Floor, Palette.FloorBackground, '.');
                }
                else
                {
                    console.Set(cell.X, cell.Y, Palette.Wall, Palette.WallBackground, '#');
                }
            }
        }

        /// <summary>
        /// Populate <see cref="Tiles"/> and <see cref="AnimatedTiles"/> based on the state of <see cref="GraphicalSystem.Showing"/>
        /// </summary>
        protected virtual void GenerateRepresentation()
        {
            Tiles.Clear();
            DungeonMap toRepresent = Showing.DMap;
            for (int row = 0; row < toRepresent.Height; row++)
            {
                for(int col = 0; col < toRepresent.Width; col++)
                {
                    Entity effect = toRepresent.GetVFX(row, col);
                    if(effect != null)
                    {
                        GenerateAppendRepresentation(effect);
                    }
                    else
                    {
                        Entity top = Showing.DMap.GetActorOrItem(row, col);
                        if (top != null)
                            GenerateAppendRepresentation(top);
                    }
                }
            }
            // This will get drawn last, which is good:
            if(Showing.ExamineCursor != null)
                Tiles.Add(TextTilePalette.Represent(Showing.ExamineCursor));
        }

        /// <summary>
        /// Generate the appropriate <see cref="TextTile"/> for <paramref name="toRepresent"/> 
        /// and append it to <see cref="Tiles"/> or <see cref="AnimatedTiles"/>.
        /// </summary>
        /// <param name="toRepresent">The <see cref="Entity"/> to show.</param>
        protected virtual void GenerateAppendRepresentation(Entity toRepresent)
        {
            TextTile representation = TextTilePalette.Represent(toRepresent);
            Tiles.Add(representation);
        }
    }
}
