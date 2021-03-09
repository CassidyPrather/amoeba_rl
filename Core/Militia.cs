using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using AmoebaRL.Interfaces;
using AmoebaRL.UI;

namespace AmoebaRL.Core
{
    public class Militia : Monster, IEatable
    {
        public Militia()
        {
            Awareness = 3;
            Color = Palette.Militia;
            Symbol = 'm';
            Speed = 16;
        }

        public void OnEaten()
        {
            Game.DMap.RemoveActor(this);
            CapturedMilitia transformation = new CapturedMilitia
            {
                X = X,
                Y = Y
            };

            Game.DMap.AddActor(transformation);
        }

        public class CapturedMilitia : Actor
        {
            public CapturedMilitia()
            {
                Awareness = 1;
                Slime = true;
                Color = Palette.Militia;
                Symbol = 'm';
            }
        }
    }
}
