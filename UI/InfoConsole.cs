using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using RogueSharp;
using RLNET;


namespace AmoebaRL.UI
{
    class InfoConsole : RLConsole
    {
        public static readonly int INFO_WIDTH = MapConsole.MAP_WIDTH;
        public static readonly int INFO_HEIGHT = 11;

        public InfoConsole() : base(INFO_WIDTH, INFO_HEIGHT)
        {

        }

        public void OnUpdate(object sender, UpdateEventArgs e)
        {
            SetBackColor(0, 0, Width, Height, RLColor.Gray);
            Print(1, 1, "Log", RLColor.White);
        }
    }
}
