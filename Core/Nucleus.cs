using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using AmoebaRL.UI;

namespace AmoebaRL.Core
{
    public class Nucleus : Actor
    {
        public Nucleus()
        {
            Awareness = 15;
            Name = "Nucleus";
            Color = Palette.Player;
            Symbol = '@';
            X = 10;
            Y = 10;
        }
    }
}
