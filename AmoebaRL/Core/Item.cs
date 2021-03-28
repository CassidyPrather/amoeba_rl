using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using RLNET;
using RogueSharp;
using AmoebaRL.Interfaces;
using AmoebaRL.UI;


namespace AmoebaRL.Core
{
    /// <summary>
    /// Something which exists on the floor, but is not autonomous and does not block primary features (e.g. <see cref="Actor"/>).
    /// Only one can exist in a location at a time.
    /// </summary>
    /// <remarks>
    /// Does not use a proper memory system yet, but this is needed so that the user cannot tell if items out of FOV were destroyed.
    /// Maybe add a "memory layer" to the map.
    /// </remarks>
    public class Item : Entity, IItem
    {
        // IItem
        public String Name { get; set; }
        
        // TODO find a way to automatically make this explored only.
    }
}
