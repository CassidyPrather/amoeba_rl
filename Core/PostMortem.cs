using AmoebaRL.UI;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Core
{
    public class PostMortem : Actor
    {
        public PostMortem()
        {
            Name = "Post Mortem";
            Symbol = '@';
            Color = Palette.Slime;
            Slime = false;
            Awareness = 0;
            Speed = 0;
            Game.MessageLog.Add("Press ESC to quit.");
        }
    }
}
