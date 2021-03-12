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
            Color = Palette.Calcium;
            Symbol = '$';
            Slime = 1;
            Speed = 1;
        }

        public override Resource Provides { get; set; } = Resource.CALCIUM;

        public override List<Item> Components() => new List<Item> { new CalciumDust(), new Nutrient() };

        public override string GetDescription()
        {
            return "Builds strong bones. Automatically consumed by adjacent organelles when an upgrade is possible.";
        }
    }

    public class CalciumDust : Catalyst
    {
        public CalciumDust()
        {
            Color = Palette.Calcium;
            Symbol = '%';
            Name = "Calcium Dust";
        }

        public override string GetDescription()
        {
            return "This precious powder could be used to build strong bones.";
        }

        public override Actor NewOrganelle()
        {
            return new Calcium();
        }
    }
}
