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
        private static readonly int _fontWidth = 12;
        private static readonly int _fontHeight = 12;
        #endregion

        private static readonly string _fontFileName = "terminal12x12_gs_ro.png";

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
        public static int DefaultSpawnRate = 50;

        public static int EvolutionRate = 6;

        public static int MaxBudget = 5;

        public static int CityArmor = 100;
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

        public static OrganelleLog OrganelleLog { get; private set; }

        public static Cursor ExamineCursor { get; private set; } = null;
        #endregion

        private bool _renderRequired = true;

        private DateTime _lastGraphicalTime;

        public Game()
        {
            _lastGraphicalTime = DateTime.UtcNow;
            StartNewGame();
            // Set up a handler for RLNET's Update event
            _rootConsole.Update += OnRootConsoleUpdate;
            // Set up a handler for RLNET's Render event
            _rootConsole.Render += OnRootConsoleRender;
        }

        static public void Clean()
        {
            Rand = null;
            DMap = null;
            Player = null;
            PlayerMass = null;
            CommandSystem = null;
            SchedulingSystem = null;
            MessageLog = null;
            OrganelleLog = null;
            ExamineCursor = null;
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
            MessageLog.Add("Arrow keys: Move / Select");
            MessageLog.Add("Space: Wait");
            MessageLog.Add("X: Toggle examine mode");
            MessageLog.Add("Z: Toggle organelle mode");
            MessageLog.Add("ESC: Back to player mode");
            MessageLog.Add("A, D: Cycle active nucleus");
            MessageLog.Add("Destroy all cities to win");
            MessageLog.Add("Consult the \"README\" file to review these instructions and more");
            OrganelleLog = new OrganelleLog();

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

            if (ExamineCursor != null)
                didPlayerAct = UserInputExamine(keyPress);
            else if (MessageLog.Showing == MessageLog.Mode.ORGANELLE)
                didPlayerAct = UserInputOrganellePane(keyPress);
            else if (Player != null)
                didPlayerAct = UserInputLive(keyPress);
            else
                didPlayerAct = UserInputMeta(keyPress);

            

            if (didPlayerAct)
            {
                _renderRequired = true;
                CommandSystem.EndPlayerTurn();
            }
        }

        private bool UserInputExamine(RLKeyPress keyPress)
        {
            if (keyPress != null)
            {
                if (keyPress.Key == RLKey.Up)
                {
                    ExamineCursor.Move(ExamineCursor.X, ExamineCursor.Y - 1);
                }
                else if (keyPress.Key == RLKey.Down)
                {
                    ExamineCursor.Move(ExamineCursor.X, ExamineCursor.Y + 1);
                }
                else if (keyPress.Key == RLKey.Left)
                {
                    ExamineCursor.Move(ExamineCursor.X - 1, ExamineCursor.Y);
                }
                else if (keyPress.Key == RLKey.Right)
                {
                    ExamineCursor.Move(ExamineCursor.X + 1, ExamineCursor.Y);
                }
                else if (keyPress.Key == RLKey.X)
                {
                    DMap.RemoveVFX(ExamineCursor);
                    ExamineCursor = null;
                    MessageLog.Toggle();
                    // Exit examine mode
                }
                else if (keyPress.Key == RLKey.Z)
                {
                    DMap.RemoveVFX(ExamineCursor);
                    ExamineCursor = null;
                    MessageLog.Toggle();
                    MessageLog.Toggle(); // toggle 2x = go to organelle inspection mode
                    // Exit examine mode
                }
                else if (keyPress.Key == RLKey.Escape)
                {
                    DMap.RemoveVFX(ExamineCursor);
                    ExamineCursor = null;
                    MessageLog.Toggle();
                    // _rootConsole.Close();
                    // Quit the game
                }
            }
            return false;
        }

        /// <summary>
        /// Returns true if the player moved.
        /// </summary>
        /// <param name="keyPress"></param>
        /// <returns></returns>
        private bool UserInputLive(RLKeyPress keyPress)
        {
            if (keyPress != null)
            {
                if (keyPress.Key == RLKey.Up)
                {
                    return CommandSystem.AttackMovePlayer(Player, Direction.Up);
                }
                else if (keyPress.Key == RLKey.Down)
                {
                    return CommandSystem.AttackMovePlayer(Player, Direction.Down);
                }
                else if (keyPress.Key == RLKey.Left)
                {
                    return CommandSystem.AttackMovePlayer(Player, Direction.Left);
                }
                else if (keyPress.Key == RLKey.Right)
                {
                    return CommandSystem.AttackMovePlayer(Player, Direction.Right);
                }
                else if (keyPress.Key == RLKey.Space || keyPress.Key == RLKey.Period
                    || keyPress.Key == RLKey.KeypadPeriod
                    || keyPress.Key == RLKey.Keypad5)
                {
                    return CommandSystem.Wait();
                }
                else if(keyPress.Key == RLKey.A)
                {
                    CommandSystem.NextNucleus(-1);
                }
                else if(keyPress.Key == RLKey.D)
                {
                    CommandSystem.NextNucleus(1);
                }
                else if (keyPress.Key == RLKey.Z)
                {
                    MessageLog.Toggle();
                    // Toggle Information Pane (Organelle/Log)
                }
                else if (keyPress.Key == RLKey.X)
                {
                    MessageLog.ExamineMode();
                    ExamineCursor = new Cursor()
                    {
                        X = Player.X,
                        Y = Player.Y
                    };
                    DMap.AddVFX(ExamineCursor);
                    // Enter Examine Mode
                }
                else if (keyPress.Key == RLKey.Escape)
                {
                    if (MessageLog.Showing == MessageLog.Mode.ORGANELLE)
                        MessageLog.Toggle();
                }
            }
            return false;
        }

        private bool UserInputOrganellePane(RLKeyPress keyPress)
        {
            if (keyPress != null)
            {
                if (keyPress.Key == RLKey.Up || keyPress.Key == RLKey.Left)
                {
                    OrganelleLog.Scroll(-1);
                }
                else if (keyPress.Key == RLKey.Down || keyPress.Key == RLKey.Right)
                {
                    OrganelleLog.Scroll(1);
                }
                else if (keyPress.Key == RLKey.Q)
                {
                    OrganelleLog.Page(-1);
                }
                else if (keyPress.Key == RLKey.E)
                {
                    OrganelleLog.Page(1);
                }
                else if (keyPress.Key == RLKey.Z)
                {
                    MessageLog.Toggle();
                    // Toggle Information Pane (Organelle/Log)
                }
                else if (keyPress.Key == RLKey.X)
                {
                    ExamineCursor = new Cursor()
                    {
                        X = OrganelleLog.Highlighted.X,
                        Y = OrganelleLog.Highlighted.Y
                    };
                    DMap.AddVFX(ExamineCursor);
                    MessageLog.ExamineMode();
                    // Enter Examine Mode
                }
                else if (keyPress.Key == RLKey.Escape)
                {
                    MessageLog.Toggle();
                    // Go back to play mode
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
                else if (keyPress.Key == RLKey.R)
                {
                    Program.PlayAgain = true;
                    _rootConsole.Close();
                }
                return true; // Let the user idle after their death I guess.
            }
            return false;
        }

        private void OnRootConsoleRender(object sender, UpdateEventArgs e)
        {
            // Update Animations
            DateTime renderInstant = DateTime.UtcNow;
            TimeSpan delta = renderInstant - _lastGraphicalTime;
            _lastGraphicalTime = renderInstant;
            if (DMap.Animate(_mapConsole, delta))
                _rootConsole.Draw();
            // Update Main Game
            if(_renderRequired)
            {
                DMap.Draw(_mapConsole);
                //Player.Draw(_mapConsole, DMap);
                MessageLog.Draw(_infoConsole);
                OrganelleLog.Draw(_playerConsole);

                RLConsole.Blit(_mapConsole, 0, 0, _mapConsole.Width, _mapConsole.Height, _rootConsole, 0, 0);
                RLConsole.Blit(_infoConsole, 0, 0, _infoConsole.Width, _infoConsole.Height, _rootConsole, 0, _mapConsole.Height);
                RLConsole.Blit(_playerConsole, 0, 0, _playerConsole.Width, _playerConsole.Height, _rootConsole, _mapConsole.Width, 0);

                _rootConsole.Draw(); // Must come after "inner draws"

                _renderRequired = false;
            }
        }
    }
}
