using AmoebaRL.Interfaces;
using AmoebaRL.UI;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Core.Organelles
{
    public class Membrane : Organelle
    {
        public Membrane()
        {
            Color = Palette.Membrane;
            Symbol = 'B';
            Slime = true;
            Awareness = 0;
        }
    }

    public class BarbedWire : Catalyst
    {
        public BarbedWire()
        {
            Color = Palette.Membrane;
            Symbol = 'b';
        }

        public override Actor NewOrganelle()
        {
            return new Membrane();
        }
    }
}
