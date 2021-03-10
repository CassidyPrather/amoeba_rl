using AmoebaRL.Interfaces;
using AmoebaRL.UI;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Core.Organelles
{
    class Membrane : Organelle
    {
        public Membrane()
        {
            Color = Palette.Membrane;
            Symbol = 'B';
            Slime = true;
            Awareness = 0;
        }
    }

    class BarbedWire : Catalyst
    {
        BarbedWire()
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
