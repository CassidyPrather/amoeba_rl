using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using RogueSharp;
using RLNET;
using AmoebaRL.UI;

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

        public Game()
        {
            InitConsoles();
            // Set up a handler for RLNET's Update event
            _rootConsole.Update += OnRootConsoleUpdate;
            // Set up a handler for RLNET's Render event
            _rootConsole.Render += OnRootConsoleRender;
            // Begin RLNET's game loop
            _rootConsole.Run();
        }

        private void InitConsoles()
        {
            _mapConsole = new MapConsole();
            _playerConsole = new PlayerConsole();
            _infoConsole = new InfoConsole();
            _rootConsole = new RLRootConsole(_fontFileName, _mapConsole.Width + _playerConsole.Width,
                                             _mapConsole.Height + _infoConsole.Height, _fontWidth, _fontHeight, 1f,
                                             _winTitle);
        }

        public void Play() => _rootConsole.Run();

        private void OnRootConsoleUpdate(object sender, UpdateEventArgs e)
        {
            _mapConsole.OnUpdate(sender, e);
            _infoConsole.OnUpdate(sender, e);
            _playerConsole.OnUpdate(sender, e);
        }

        private void OnRootConsoleRender(object sender, UpdateEventArgs e)
        {
            RLConsole.Blit(_mapConsole, 0, 0, _mapConsole.Width, _mapConsole.Height, _rootConsole, 0, 0);
            RLConsole.Blit(_infoConsole, 0, 0, _infoConsole.Width, _infoConsole.Height, _rootConsole, 0, _mapConsole.Height);
            RLConsole.Blit(_playerConsole, 0, 0, _playerConsole.Width, _playerConsole.Height, _rootConsole, _mapConsole.Width, 0);

            _rootConsole.Draw();
        }

    }
}
