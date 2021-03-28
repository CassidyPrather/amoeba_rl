using AmoebaRL.Interfaces;
using RogueSharp;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using AmoebaRL.Systems;

namespace AmoebaRL.Core.Organelles
{
    public abstract class Organelle : Actor, IOrganelle, IDescribable
    {
        /// <summary>
        /// Whether this organelle can move. Most relevant in <see cref="CommandSystem.MoveOrganelle(Organelle, int, int)"/>
        /// </summary>
        public bool Anchor { get; set; } = false;

        /// <summary>
        /// Returns a new list of the <see cref="Item"/>s which were used to create this organelle.
        /// </summary>
        /// <returns>A <see cref="List{Item}"/> of everything used to make the organelle.</returns>
        public virtual List<Item> Components() => new List<Item>();

        public void Unslime()
        {
            Map.RemoveActor(this);
            OnUnslime();
        }

        /// <summary>
        /// Neatly turn into a pile of everything that was used to create this.
        /// </summary>
        public virtual void OnUnslime()
        {
            BecomeItems(Components());
            // foreach(Item drop in Components())
            // {
            //    BecomeItem(drop);
            // }
        }

        public void Destroy()
        {
            Map.RemoveActor(this);
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

        public virtual string Description => $"{DescBody} {Flavor}";

        public virtual string DescBody => "";

        public virtual string Flavor => "";
    }
}
