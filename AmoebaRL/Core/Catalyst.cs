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
        public abstract string Description { get; }

        public abstract Actor NewOrganelle();

        public virtual void OnEaten()
        {
            Map.RemoveItem(this);
            Actor transformation = NewOrganelle();
            transformation.X = X;
            transformation.Y = Y;
            Map.AddActor(transformation);
            Map.PlayerMass.Add(transformation);
        }
    }
}
