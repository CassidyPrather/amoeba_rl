using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using RLNET;


namespace AmoebaRL.UI
{
    class PlayerConsole : RLConsole
    {
        public static readonly int PLAYER_WIDTH = 22;
        public static readonly int PLAYER_HEIGHT = MapConsole.MAP_HEIGHT + InfoConsole.INFO_HEIGHT;

        public PlayerConsole() : base(PLAYER_WIDTH, PLAYER_HEIGHT)
        {

        }

        public void OnUpdate(object sender, UpdateEventArgs e)
        {
            SetBackColor(0, 0, Width, Height, RLColor.Brown);
            Print(1, 1, "Organelles", RLColor.White);
        }
    }
}
