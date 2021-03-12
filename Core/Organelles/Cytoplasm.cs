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
            Slime = true;
            Speed = 10000; // aaa
        }

        public override List<Item> Components() => new List<Item>() { new Nutrient() };

        public override string GetDescription()
        {
            return "A terrifying, viscious mass. Not very useful on its own.";
        }

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

        public override string GetDescription()
        {
            return "This precious meal is the foundation of growth.";
        }

        public override Actor NewOrganelle() => new Cytoplasm();
    }
}
