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
using AmoebaRL.Core.Organelles;

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

        #region Settings
        public static int SpawnRate = 40;
        #endregion

        #region Static Handles
        // hate that all these are static, but it's what the tutorial taught.

        public static IRandom Rand { get; private set; }

        public static DungeonMap DMap { get; private set; }

        public static Nucleus Player { get; set; }

        public static List<Actor> PlayerMass { get; set; }

        public static CommandSystem CommandSystem { get; private set; }

        public static SchedulingSystem SchedulingSystem { get; private set; }

        public static MessageLog MessageLog { get; private set; }
        #endregion

        private static bool _renderRequired = true;

        public Game()
        {
            StartNewGame();
            // Set up a handler for RLNET's Update event
            _rootConsole.Update += OnRootConsoleUpdate;
            // Set up a handler for RLNET's Render event
            _rootConsole.Render += OnRootConsoleRender;
        }

        /// <summary>
        /// Called once when the game is launched..
        /// </summary>
        protected void StartNewGame()
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
            SchedulingSystem = new SchedulingSystem();
            // Fix the numbers in the map generator call later.
            MapGenerator mapGenerator = new MapGenerator(_mapConsole.Width, _mapConsole.Height, 20, 13, 7);
            DMap = mapGenerator.CreateMap();
            
            // Create a new MessageLog and print the random seed used to generate the level
            MessageLog = new MessageLog();
            MessageLog.Add("Reach 128 mass to win.");
            MessageLog.Add($"Level created with seed '{seed}'");

            // Launch the game!
            CommandSystem.AdvanceTurn();
        }

        public void Play() => _rootConsole.Run();

        private void OnRootConsoleUpdate(object sender, UpdateEventArgs e)
        {
            RLKeyPress keyPress = _rootConsole.Keyboard.GetKeyPress();
            if (CommandSystem.IsPlayerTurn)
            {
                _renderRequired = true;
                UserInput(keyPress);
            }
            else
            { 
                CommandSystem.AdvanceTurn();
                
            }
            _mapConsole.OnUpdate(sender, e);
            _infoConsole.OnUpdate(sender, e);
            _playerConsole.OnUpdate(sender, e);
        }

        private void UserInput(RLKeyPress keyPress)
        {
            bool didPlayerAct;

            if (Player != null)
                didPlayerAct = UserInputLive(keyPress);
            else
                didPlayerAct = UserInputMeta(keyPress);

            

            if (didPlayerAct)
            {
                _renderRequired = true;
                CommandSystem.EndPlayerTurn();
            }
        }

        private bool UserInputLive(RLKeyPress keyPress)
        {
            if (keyPress != null)
            {
                if (keyPress.Key == RLKey.Up)
                {
                    return CommandSystem.AttackMoveOrganelle(Player, Direction.Up);
                }
                else if (keyPress.Key == RLKey.Down)
                {
                    return CommandSystem.AttackMoveOrganelle(Player, Direction.Down);
                }
                else if (keyPress.Key == RLKey.Left)
                {
                    return CommandSystem.AttackMoveOrganelle(Player, Direction.Left);
                }
                else if (keyPress.Key == RLKey.Right)
                {
                    return CommandSystem.AttackMoveOrganelle(Player, Direction.Right);
                }
                else if (keyPress.Key == RLKey.Space || keyPress.Key == RLKey.Period
                    || keyPress.Key == RLKey.KeypadPeriod
                    || keyPress.Key == RLKey.Keypad5)
                {
                    return CommandSystem.Wait();
                }
                else if (keyPress.Key == RLKey.Escape)
                {
                    _rootConsole.Close();
                }
            }
            return false;
        }

        private bool UserInputMeta(RLKeyPress keyPress)
        {
            if (keyPress != null)
            {
                if(keyPress.Key == RLKey.Escape)
                {
                    _rootConsole.Close();
                }
                return true; // Let the user idle after their death I guess.
            }
            return false;
        }

        private void OnRootConsoleRender(object sender, UpdateEventArgs e)
        {
            if(_renderRequired)
            {
                DMap.Draw(_mapConsole);
                //Player.Draw(_mapConsole, DMap);
                MessageLog.Draw(_infoConsole);

                RLConsole.Blit(_mapConsole, 0, 0, _mapConsole.Width, _mapConsole.Height, _rootConsole, 0, 0);
                RLConsole.Blit(_infoConsole, 0, 0, _infoConsole.Width, _infoConsole.Height, _rootConsole, 0, _mapConsole.Height);
                RLConsole.Blit(_playerConsole, 0, 0, _playerConsole.Width, _playerConsole.Height, _rootConsole, _mapConsole.Width, 0);

                _rootConsole.Draw(); // Must come after "inner draws"

                _renderRequired = false;
            }
        }
    }
}
