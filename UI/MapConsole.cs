using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using RogueSharp;
using RLNET;

namespace AmoebaRL.UI
{
    class MapConsole : RLConsole
    {
        public static readonly int MAP_WIDTH = 64;
        public static readonly int MAP_HEIGHT = 48;

        public MapConsole() : base(MAP_WIDTH, MAP_HEIGHT)
        {
            //SetBackColor(0, 0, Width, Height, Palette.FloorBackground);
            //Print(1, 1, "Map", Palette.TextHeading);
        }

        public void OnUpdate(object sender, UpdateEventArgs e)
        {
            
        }
    }
}
