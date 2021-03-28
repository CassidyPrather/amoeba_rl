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
            Slime = 1;
            Delay = 1;
        }

        public override Resource Provides { get; set; } = Resource.CALCIUM;

        public override List<Item> Components() => new List<Item> { new CalciumDust(), new Nutrient() };

        public override string Description => "Builds strong bones. Automatically consumed by adjacent organelles when an upgrade is possible.";
    }

    public class CalciumDust : Catalyst
    {
        public CalciumDust()
        {
            Name = "Calcium Dust";
        }

        public override string Description => "This precious powder could be used to build strong bones.";

        public override Actor NewOrganelle()
        {
            return new Calcium();
        }
    }
}
