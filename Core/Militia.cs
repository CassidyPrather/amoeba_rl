using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using AmoebaRL.UI;

namespace AmoebaRL.Core
{
    public class Militia : Monster
    {
        public Militia()
        {
            Awareness = 3;
            Color = Palette.Militia;
        }
    }
}
