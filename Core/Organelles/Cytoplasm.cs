using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using AmoebaRL.UI;

namespace AmoebaRL.Core.Organelles
{
    public class Cytoplasm : Organelle
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
            Speed = 10000; // aaa
        }
    }
}
