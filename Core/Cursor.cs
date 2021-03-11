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
    public class Cursor : Animation
    {
        public Cursor()
        {
            Color = Palette.Cursor;
            BackgroundColor = Palette.FloorBackground;
            Symbol = 'X';
            Frames = 2;
            Speed = 2;
            AlwaysVisible = true;
        }

        public bool Move(int x, int y)
        {
            if(x < Game.DMap.Width && x >= 0 && y < Game.DMap.Height && y >= 0)
            {
                X = x;
                Y = y;
                return true;
            }
            return false;
        }

        public override void SetFrame(int idx)
        {
            if (idx == 0)
                Transparent = false;
            else
                Transparent = true;
        }

        public IDescribable Under()
        {
            IDrawable Hovering = Game.DMap.GetActorOrItem(X,Y);
            if (Hovering != null && Hovering is IDescribable d
                && ((Hovering is City && Game.DMap.IsExplored(X,Y)) || Game.DMap.IsInFov(X,Y)))
                return d;
            return null;
        }
    }
}
