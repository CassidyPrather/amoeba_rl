using AmoebaRL.Core;
using AmoebaRL.Core.Organelles;
using AmoebaRL.Interfaces;
using RLNET;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Systems
{
    public class OrganelleLog
    {
        public float NiceTurnBuffer { get; set; } = 0;

        public int idx = 0; // Select an organelle
        public int page = 0; // Scroll through huge organelle lists

        public Cursor HighlightCursor { get; protected set; } = null;

        public Organelle Highlighted => GetLoggable().Count > idx && GetLoggable()[idx] is Organelle o ? o : null;

        public List<Actor> Tracking { get; set; }

        public List<Actor> GetLoggable() =>
            Tracking.Where(a => !(a is Cytoplasm) && !(a is CraftingMaterial)).ToList();

        public OrganelleLog(List<Actor> toTrack)
        {
            Tracking = toTrack;
        }

        public void Scroll(int by)
        {
            int numItems = GetLoggable().Count();
            if (numItems > 0)
                idx = (idx + by) % numItems;
            if (idx < 0)
                idx += numItems;
        }

        public void Page(int by)
        {
            page += by;
            if (page < 0)
                page = 0;
        }
    }
}
