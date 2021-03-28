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
            Slime = 1;
            Delay = 1;
        }

        public override Resource Provides { get; set; } = Resource.ELECTRONICS;

        public override List<Item> Components() => new List<Item>() { new SiliconDust(), new Nutrient() };

        public override string Description => "Not found in nature. Automatically consumed by adjacent organelles when an upgrade is possible.";
    }

    public class SiliconDust : Catalyst
    {
        public SiliconDust()
        {
            Name = "Silicon Dust";
        }

        public override string Description => "These rocks contain the magic of humanity, and could be used to accelerate evolution.";

        public override Actor NewOrganelle()
        {
            return new Electronics();
        }
    }
}
