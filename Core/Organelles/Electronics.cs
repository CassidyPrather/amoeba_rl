using AmoebaRL.Interfaces;
using AmoebaRL.UI;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Core.Organelles
{
    public class Electronics : CraftingMaterial
    {
        public Electronics()
        {
            Awareness = 0;
            Name = "Calcium";
            Color = Palette.Hunter;
            Symbol = '$';
            Slime = true;
            Speed = 1;
        }

        public override Resource Provides { get; set; } = Resource.ELECTRONICS;


        public override void OnDestroy()
        {
            BecomeItem(new SiliconDust());
        }
    }

    public class SiliconDust : Catalyst
    {
        public SiliconDust()
        {
            Color = Palette.Hunter;
            Symbol = '%';
            Name = "Silicon Dust";
        }

        public override Actor NewOrganelle()
        {
            return new Calcium();
        }
    }
}
