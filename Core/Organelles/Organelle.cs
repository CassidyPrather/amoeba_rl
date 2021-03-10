using AmoebaRL.Interfaces;
using RogueSharp;
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

        public void BecomeItem(Item i)
        {
            ICell lands = Game.DMap.NearestLootDrop(X, Y);
            i.X = lands.X;
            i.Y = lands.Y;
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
