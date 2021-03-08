using AmoebaRL.UI;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Core
{
    public class City : Actor
    {
        public City()
        {
            Awareness = 0;
            Symbol = 'C';
            Name = "City";
            Color = Palette.City;
        }
    }
}
