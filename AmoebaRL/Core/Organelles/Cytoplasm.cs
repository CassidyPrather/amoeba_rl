using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using AmoebaRL.UI;

namespace AmoebaRL.Core.Organelles
{
    public class Cytoplasm : Organelle
    {
        public Cytoplasm()
        { 
            Awareness = 0;
            Name = "Cytoplasm";
            Color = Palette.Slime;
            Symbol = ' ';
            Slime = 1;
            Delay = 10000; // aaa
        }

        public override List<Item> Components() => new List<Item>() { new Nutrient() };

        public override string Description => "A terrifying, viscious mass, and your most basic organelle. Not very useful on its own." +
                "Like every organelle, nuclei can swap positions with it. Also like other organelles, if something moves (not swaps), " +
                "this will be pulled along behind it if it is a part of the path to the furthest organelle in the mass. " +
                "This path is visualized as a brighter shade for the selected nucleus.";

        public override void OnDestroy()
        {
            // Cytoplasm drop nothing when destroyed!
        }
    }

    public class Nutrient : Catalyst
    {
        public Nutrient()
        {
            Name = "Nutrient";
            Color = Palette.Slime;
            Symbol = '%';
            X = 10;
            Y = 10;
        }

        public override string Description => "This precious meal is the foundation of growth.";

        public override Actor NewOrganelle() => new Cytoplasm();
    }
}
