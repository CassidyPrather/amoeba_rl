using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using RogueSharp;
using RLNET;
using AmoebaRL.UI;
using AmoebaRL.Core;
using AmoebaRL.Systems;
using RogueSharp.Random;

namespace AmoebaRL
{
    public class Game
    {
        // Constants may need to be fiddled with, should be automated where possible.
        #region Constants
        #region Sizes
        private static readonly int _fontWidth = 8;
        private static readonly int _fontHeight = 8;
        #endregion

        private static readonly string _fontFileName = "terminal8x8.png";

        private static readonly string _winTitle = "Amoeba RL";
        #endregion

        // Would like to make classes for each of these.
        #region Consoles
        private RLRootConsole _rootConsole;
        private MapConsole _mapConsole;
        private InfoConsole _infoConsole;
        private PlayerConsole _playerConsole;
        #endregion

        public static int seed;

        public static IRandom Rand { get; private set; }

        public static DungeonMap DMap { get; private set; }

        public static Nucleus Player { get; set; }

        private static bool _renderRequired = true;

        public static CommandSystem CommandSystem { get; private set; }

        public Game()
        {
            Start();
            // Set up a handler for RLNET's Update event
            _rootConsole.Update += OnRootConsoleUpdate;
            // Set up a handler for RLNET's Render event
            _rootConsole.Render += OnRootConsoleRender;
            
        }

        protected void Start()
        {
            seed = (int)DateTime.UtcNow.Ticks; // may save for later?
            Rand = new DotNetRandom(seed);

            _mapConsole = new MapConsole();
            _playerConsole = new PlayerConsole();
            _infoConsole = new InfoConsole();
            _rootConsole = new RLRootConsole(_fontFileName, _mapConsole.Width + _playerConsole.Width,
                                             _mapConsole.Height + _infoConsole.Height, _fontWidth, _fontHeight, 1f,
                                             _winTitle);
            CommandSystem = new CommandSystem();
            // Fix the numbers in the map generator call later.
            MapGenerator mapGenerator = new MapGenerator(_mapConsole.Width, _mapConsole.Height, 20, 13, 7);
            DMap = mapGenerator.CreateMap();
            DMap.UpdatePlayerFieldOfView();
        }

        public void Play() => _rootConsole.Run();

        private void OnRootConsoleUpdate(object sender, UpdateEventArgs e)
        {
            UserInput(sender, e);
            _mapConsole.OnUpdate(sender, e);
            _infoConsole.OnUpdate(sender, e);
            _playerConsole.OnUpdate(sender, e);
        }

        private void UserInput(object sender, UpdateEventArgs e)
        {
            bool didPlayerAct = false;
            RLKeyPress keyPress = _rootConsole.Keyboard.GetKeyPress();

            if (keyPress != null)
            {
                if (keyPress.Key == RLKey.Up)
                {
                    didPlayerAct = CommandSystem.MovePlayer(Direction.Up);
                }
                else if (keyPress.Key == RLKey.Down)
                {
                    didPlayerAct = CommandSystem.MovePlayer(Direction.Down);
                }
                else if (keyPress.Key == RLKey.Left)
                {
                    didPlayerAct = CommandSystem.MovePlayer(Direction.Left);
                }
                else if (keyPress.Key == RLKey.Right)
                {
                    didPlayerAct = CommandSystem.MovePlayer(Direction.Right);
                }
                else if (keyPress.Key == RLKey.Escape)
                {
                    _rootConsole.Close();
                }
            }

            if (didPlayerAct)
            {
                _renderRequired = true;
            }
        }

        private void OnRootConsoleRender(object sender, UpdateEventArgs e)
        {
            if(_renderRequired)
            {
                DMap.Draw(_mapConsole);
                Player.Draw(_mapConsole, DMap);

                RLConsole.Blit(_mapConsole, 0, 0, _mapConsole.Width, _mapConsole.Height, _rootConsole, 0, 0);
                RLConsole.Blit(_infoConsole, 0, 0, _infoConsole.Width, _infoConsole.Height, _rootConsole, 0, _mapConsole.Height);
                RLConsole.Blit(_playerConsole, 0, 0, _playerConsole.Width, _playerConsole.Height, _rootConsole, _mapConsole.Width, 0);

                _rootConsole.Draw(); // Must come after "inner draws"

                _renderRequired = false;
            }
        }
    }
}
