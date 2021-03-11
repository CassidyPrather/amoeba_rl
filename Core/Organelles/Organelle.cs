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
        /// <summary>
        /// Returns a new list of the <see cref="Item"/>s which were used to create this organelle.
        /// </summary>
        /// <returns>A <see cref="List{Item}"/> of everything used to make the organelle.</returns>
        public virtual List<Item> Components() => new List<Item>();

        public void Unslime()
        {
            Game.DMap.RemoveActor(this);
            OnUnslime();
        }

        /// <summary>
        /// Neatly turn into a pile of everything that was used to create this.
        /// </summary>
        public virtual void OnUnslime()
        {
            foreach(Item drop in Components())
            {
                BecomeItem(drop);
            }
        }

        public void Destroy()
        {
            Game.DMap.RemoveActor(this);
            OnDestroy();
        }

        /// <summary>
        /// Only turn into a single item used to create this (usually an organelle seed).
        /// </summary>
        public virtual void OnDestroy() 
        {
            List<Item> components = Components();
            if (components.Count > 0)
                BecomeItem(components[0]);
        }

        public void BecomeItem(Item i)
        {
            ICell lands = Game.DMap.NearestLootDrop(X, Y);
            i.X = lands.X;
            i.Y = lands.Y;
            Game.DMap.AddItem(i);
        }

        public virtual Actor BecomeActor(Actor a)
        {
            a.X = X;
            a.Y = Y;
            Game.DMap.AddActor(a);
            return a;
        }
    }
}
