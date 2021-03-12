using AmoebaRL.Interfaces;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Core
{
    public abstract class Catalyst : Item, IEatable, IDescribable
    {
        public abstract string GetDescription();
        public abstract Actor NewOrganelle();

        public virtual void OnEaten()
        {
            Game.DMap.RemoveItem(this);
            Actor transformation = NewOrganelle();
            transformation.X = X;
            transformation.Y = Y;
            Game.DMap.AddActor(transformation);
            Game.PlayerMass.Add(transformation);
            Game.DMap.UpdatePlayerFieldOfView();
        }
    }
}
