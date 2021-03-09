using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using AmoebaRL.UI;

namespace AmoebaRL.Core
{
    public class Nucleus : Actor
    {
        public Nucleus()
        {
            Awareness = 4;
            Name = "Nucleus";
            Color = Palette.Player;
            Symbol = '@';
            X = 10;
            Y = 10;
            Slime = true;
            Speed = 16;
        }

        public Actor Retreat()
        {
            List<Actor> sacrifices = Game.PlayerMass.Where(a => DungeonMap.TaxiDistance(this, a) == 1 && !(a is Nucleus)).ToList();
            if(sacrifices.Count > 0)
            {
                int r = Game.Rand.Next(0, sacrifices.Count-1);
                Game.DMap.Swap(this, sacrifices[r]);
                return sacrifices[r];
            }
            return null;
        }
    }
}
