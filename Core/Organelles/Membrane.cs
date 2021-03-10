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
            Name = "Membrane";
            Slime = true;
            Awareness = 0;
        }

        public override void OnDestroy()
        {
            BecomeItem(new BarbedWire());
        }
    }

    public class BarbedWire : Catalyst
    {
        public BarbedWire()
        {
            Color = Palette.MembraneInactive;
            Symbol = 'b';
            Name = "Barbed Wire";
        }

        public override Actor NewOrganelle()
        {
            return new Membrane();
        }
    }
}
