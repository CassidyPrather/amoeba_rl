﻿using AmoebaRL.Interfaces;
using RogueSharp;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Core.Organelles
{
    public abstract class CraftingMaterial : Organelle, IProactive
    {
        

        public enum Resource
        {
            CALCIUM,
            ELECTRONICS
        }

        public static string ResourceName(Resource toName)
        {
            if (toName == Resource.CALCIUM)
                return "Calcium";
            else if (toName == Resource.ELECTRONICS)
                return "Electronics";
            else
                return "???";
        }

        public abstract Resource Provides { get; set; }

        public virtual void Act()
        {
            // Craft with adjacent organelles when allowed.
            List<ICell> adj = Game.DMap.Adjacent(X, Y);
            List<IUpgradable> adjUpg = new List<IUpgradable>();
            foreach (ICell a in adj)
            { 
                Actor act = Game.DMap.GetActorAt(a.X, a.Y);
                if (act != null && act is IUpgradable u && !(act is Nucleus))
                    adjUpg.Add(u);
            }
            while(adjUpg.Count > 0)
            {
                IUpgradable picked = adjUpg[Game.Rand.Next(0, adjUpg.Count-1)];
                adjUpg.Remove(picked);
                if(picked.Upgrade(Provides))
                {
                    Cytoplasm byproduct = new Cytoplasm
                    {
                        X = X,
                        Y = Y
                    };
                    Game.PlayerMass.Add(byproduct);
                    Game.DMap.RemoveActor(this);
                    BecomeActor(byproduct);
                    Game.DMap.UpdatePlayerFieldOfView();
                    break;
                }
            }
        }

        public virtual bool TryUpgrade(Actor recepient)
        {
            if(recepient is IUpgradable u)
            {
                if (u.Upgrade(Provides))
                {
                    Cytoplasm byproduct = new Cytoplasm
                    {
                        X = X,
                        Y = Y
                    };
                    Game.PlayerMass.Add(byproduct);
                    Game.DMap.RemoveActor(this);
                    BecomeActor(byproduct);
                    Game.DMap.UpdatePlayerFieldOfView();
                    return true;
                }
                return false;
            }
            return false;
        }
    }
}