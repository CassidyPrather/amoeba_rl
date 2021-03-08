using AmoebaRL.UI;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Core
{
    class Nutrient : Actor
    {
        // Should derive from like, an item class or something, not actor, but fine for now.
        public Nutrient()
        {
            Awareness = 0;
            Name = "Nutrient";
            Color = Palette.Slime;
            Symbol = '%';
            X = 10;
            Y = 10;
            Slime = false;
        }
    }
}
