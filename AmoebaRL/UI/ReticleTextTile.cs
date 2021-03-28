using AmoebaRL.Core;
using AmoebaRL.Interfaces;
using RLNET;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.UI
{
    public class ReticleTextTile : TextTile
    {

        public bool ForceInvisible { get; set; } = false;

        public override VisibilityCondition Visibility
        {
            get
            {
                if (ForceInvisible)
                    return VisibilityCondition.INVISIBLE;
                return base.Visibility;
            }
            set => base.Visibility = value;
        }

        public ReticleTextTile(Entity e, char symbol, RLColor foreground, RLColor background, VisibilityCondition visibility) : base(e)
        {
            Symbol = symbol;
            Color = foreground;
            BackgroundColor = background;
            Visibility = visibility;
            Speed = 3;
            Frames = 2;
            DetermineBackup(e);
        }

        /// <summary>
        /// Sets <see cref="Backup"/> to whatever <see cref="Represents"/> is covering. May be <c>null</c>.
        /// </summary>
        private void DetermineBackup(Entity source)
        {
            Entity under = source.Map.GetActorOrItem(source.X, source.Y);
            if (under != null)
                Backup = TextTilePalette.Represent(under);
            else
                Backup = null;
        }

        public override void SetFrame(int idx)
        {
            ForceInvisible = idx != 0; 
        }


    }
}
