using AmoebaRL.Core.Enemies;
using AmoebaRL.Interfaces;
using AmoebaRL.UI;
using RLNET;
using RogueSharp;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Core
{
    public class Cursor : Entity
    {
        public bool Move(int x, int y)
        {
            if(x < Map.Width && x >= 0 && y < Map.Height && y >= 0)
            {
                X = x;
                Y = y;
                return true;
            }
            return false;
        }

        public IDescribable Under()
        {
            Entity Hovering = Map.GetActorOrItem(X,Y);
            if (Hovering != null && Hovering is IDescribable d
                && ((Hovering is City && Map.IsExplored(X,Y)) || Map.IsInFov(X,Y)))
                return d;
            return null;
        }
    }
}
