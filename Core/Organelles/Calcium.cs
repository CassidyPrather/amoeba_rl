using AmoebaRL.Interfaces;
using AmoebaRL.UI;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Core.Organelles
{
    public class Calcium : CraftingMaterial
    {
        public Calcium()
        {
            Awareness = 0;
            Name = "Calcium";
            Color = Palette.Tank;
            Symbol = '$';
            Slime = true;
            Speed = 1;
        }

        public override Resource Provides { get; set; } = Resource.CALCIUM;

        public override void OnDestroy()
        {
            BecomeItem(new CalciumDust());
        }
    }

    public class CalciumDust : Catalyst
    {
        public CalciumDust()
        {
            Color = Palette.Tank;
            Symbol = '%';
            Name = "Calcium Dust";
        }

        public override Actor NewOrganelle()
        {
            return new Calcium();
        }
    }
}
