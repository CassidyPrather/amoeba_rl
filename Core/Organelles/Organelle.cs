using AmoebaRL.Interfaces;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Core.Organelles
{
    public abstract class Organelle : Actor, IOrganelle
    {

        public void Destroy()
        {
            Game.DMap.RemoveActor(this);
            OnDestroy();
        }

        public virtual void OnDestroy() { }

        public virtual void BecomeItem(Item i)
        {
            i.X = X;
            i.Y = Y;
            Game.DMap.AddItem(i);
        }

        public virtual void BecomeActor(Actor a)
        {
            a.X = X;
            a.Y = Y;
            Game.DMap.AddActor(a);
        }
    }
}
