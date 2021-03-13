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
            Name = "Electronics";
            Color = Palette.Hunter;
            Symbol = '$';
            Slime = 1;
            Speed = 1;
        }

        public override Resource Provides { get; set; } = Resource.ELECTRONICS;

        public override List<Item> Components() => new List<Item>() { new SiliconDust(), new Nutrient() };

        public override string GetDescription()
        {
            return "Not found in nature. Automatically consumed by adjacent organelles when an upgrade is possible.";
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

        public override string GetDescription()
        {
            return "These rocks contain the magic of humanity, and could be used to accelerate evolution.";
        }

        public override Actor NewOrganelle()
        {
            return new Electronics();
        }
    }
}
