using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using AmoebaRL.UI;

namespace AmoebaRL.Core
{
    public class Cytoplasm : Actor
    {
        public Cytoplasm()
        { 
            Awareness = 0;
            Name = "Cytoplasm";
            Color = Palette.Slime;
            Symbol = ' ';
            X = 10;
            Y = 10;
            Slime = true;
        }
    }
}
