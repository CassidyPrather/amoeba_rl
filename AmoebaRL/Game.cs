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
        public GraphicalSystem Graphics { get; protected set; }

        public int seed;

        public enum Mode
        {
            MESSAGE,
            ORGANELLE,
            EXAMINE
        }

        #region Settings
        public static int MapWidth = 48;

        public static int MapHeight = 48;

        public static int DefaultSpawnRate = 50;

        public static int EvolutionRate = 6;

        public static int MaxBudget = 5;

        public static int CityArmor = 100;
        #endregion

        #region Gamewide Handles
        // Replace the random number generator with something better:
        public IRandom Rand { get; private set; }

        public DungeonMap DMap { get; private set; }

        public Nucleus ActivePlayer { get; set; }

        // Should be map-level:
        // public static List<Actor> PlayerMass { get; set; }

        // May need to rethink:
        public CommandSystem CommandSystem { get; private set; }

        public SchedulingSystem SchedulingSystem { get; private set; }

        public MessageLog MessageLog { get; private set; }

        public OrganelleLog OrganelleLog { get; private set; }

        public Cursor ExamineCursor { get; private set; } = null;

        public Mode Showing { get; private set; } = Mode.MESSAGE;

        #endregion

        public Game()
        {
            StartNewGame();
        }

        public void Clean()
        {
            Rand = null;
            DMap = null;
            ActivePlayer = null;
            // PlayerMass = null;
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

            CommandSystem = new CommandSystem(this);
            SchedulingSystem = new SchedulingSystem();
            
            // Fix the numbers in the map generator call later.
            MapGenerator mapGenerator = new MapGenerator(this, MapWidth, MapHeight, 20, 13, 7);
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
            OrganelleLog = new OrganelleLog(DMap.PlayerMass);

            // Set up the graphics.
            // TODO: Automatically infer best GraphicalSystem subclass:
            Graphics = new ASCIIGraphics(this);

            Graphics.Run();
            // Launch the game!
            // CommandSystem.AdvanceTurn();
        }

        /// <summary>
        /// Respond to an input sent by the user interaction layer (<see cref="Graphics"/>).
        /// </summary>
        /// <param name="press">The input specified by the user interaction layer (<see cref="Graphics"/>).</param>
        /// <returns>The input was accepted and the state of the game may have changed.</returns>
        public bool HandleUserInput(RLKeyPress press)
        {
            if (CommandSystem.IsPlayerTurn)
            {
                AcceptUserInput(press);
                return true;
            }
            else
            {
                CommandSystem.AdvanceTurn();
                return false;
            }
        }

        protected void AcceptUserInput(RLKeyPress keyPress)
        {
            bool didPlayerAct;

            if (ExamineCursor != null)
                didPlayerAct = UserInputExamine(keyPress);
            else if (Showing == Mode.ORGANELLE)
                didPlayerAct = UserInputOrganellePane(keyPress);
            else if (ActivePlayer != null)
                didPlayerAct = UserInputLive(keyPress);
            else
                didPlayerAct = UserInputMeta(keyPress);

            if (didPlayerAct)
                CommandSystem.EndPlayerTurn();
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
                    DMap.RemoveEntity(ExamineCursor);
                    ExamineCursor = null;
                    Showing = Mode.MESSAGE;
                    // Exit examine mode
                }
                else if (keyPress.Key == RLKey.Z)
                {
                    DMap.RemoveEntity(ExamineCursor);
                    ExamineCursor = null;
                    Showing = Mode.ORGANELLE;
                    // Exit examine mode
                }
                else if (keyPress.Key == RLKey.Escape)
                {
                    DMap.RemoveEntity(ExamineCursor);
                    ExamineCursor = null;
                    Showing = Mode.MESSAGE;
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
                    return CommandSystem.AttackMovePlayer(ActivePlayer, Direction.Up);
                }
                else if (keyPress.Key == RLKey.Down)
                {
                    return CommandSystem.AttackMovePlayer(ActivePlayer, Direction.Down);
                }
                else if (keyPress.Key == RLKey.Left)
                {
                    return CommandSystem.AttackMovePlayer(ActivePlayer, Direction.Left);
                }
                else if (keyPress.Key == RLKey.Right)
                {
                    return CommandSystem.AttackMovePlayer(ActivePlayer, Direction.Right);
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

                    Showing = Mode.ORGANELLE;
                    // Toggle Information Pane (Organelle/Log)
                }
                else if (keyPress.Key == RLKey.X)
                {
                    Showing = Mode.EXAMINE;
                    ExamineCursor = new Cursor()
                    {
                        X = ActivePlayer.X,
                        Y = ActivePlayer.Y
                    };
                    DMap.AddEntity(ExamineCursor);
                    // Enter Examine Mode
                }
                else if (keyPress.Key == RLKey.Escape)
                {
                    if (Showing == Mode.ORGANELLE) // Should never happen.
                        Showing = Mode.MESSAGE;
                    else if (Showing == Mode.MESSAGE)
                        Graphics.End(); // Quit the game
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
                    Showing = Mode.MESSAGE;
                    // Toggle Information Pane (Organelle/Log)
                }
                else if (keyPress.Key == RLKey.X)
                {
                    ExamineCursor = new Cursor()
                    {
                        X = OrganelleLog.Highlighted.X,
                        Y = OrganelleLog.Highlighted.Y
                    };
                    DMap.AddEntity(ExamineCursor);
                    Showing = Mode.EXAMINE;
                    // Enter Examine Mode
                }
                else if (keyPress.Key == RLKey.Escape)
                {
                    Showing = Mode.MESSAGE;
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
                    Graphics.End();
                }
                else if (keyPress.Key == RLKey.R)
                {
                    Program.PlayAgain = true;
                    Graphics.End();
                }
                return true; // Let the user idle after their death I guess.
            }
            return false;
        }


        public void Toggle()
        {
            if (Showing == Mode.MESSAGE)
                Showing = Mode.ORGANELLE;
            else if (Showing == Mode.ORGANELLE)
                Showing = Mode.MESSAGE;
            else if (Showing == Mode.EXAMINE)
                Showing = Mode.MESSAGE;
        }
    }
}
