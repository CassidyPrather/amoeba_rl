using AmoebaRL.Interfaces;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using AmoebaRL.UI;

namespace AmoebaRL.Core
{
    /// <summary>
    /// Something which exists in on a <see cref="DungeonMap"/>.
    /// </summary>
    public class Entity
    {
        /// <summary>
        /// The <see cref="DungeonMap"/> this is a member of.
        /// </summary>
        public DungeonMap Map { get; set; } = null;

        /// <summary>
        /// The horizontal component of <see cref="Position"/>.
        /// </summary>
        public int X
        {
            get => Position.X;
            set
            {
                Position = new Coord(value, Y);
            }
        }

        /// <summary>
        /// The vertical component of <see cref="Position"/>.
        /// </summary>
        public int Y
        {
            get => Position.Y;
            set
            {
                Position = new Coord(X, value);
            }
        }

        /// <summary>
        /// The location within <see cref="DungeonMap"/> this occupies.
        /// </summary>
        /// <remarks>
        /// This is the first element of <see cref="Positions"/>
        /// </remarks>
        public Coord Position
        {
            get => Positions[0];
            set
            {
                // Call Positions setter:
                Positions = new List<Coord>() { value };
            }
        }

        private List<Coord> positions = new List<Coord>() { new Coord() };

        /// <summary>
        /// The locations within <see cref="DungeonMap"/> this occupies. Multi-tile entities may occupy several.
        /// Initalized to list containing a single empty coordinate.
        /// </summary>
        /// <remarks>
        /// <see cref="DungeonMap.Move(Entity, IEnumerable{Coord})"/> must be called specifically when this collection is modified.
        /// </remarks>
        public List<Coord> Positions {
            get
            {
                return positions;
            }
            set
            {
                if(Map != null)
                    Map.Move(this, value);
                positions = value;
            }
        }

        /// <summary>
        /// Whether this actor has been explored by the active user.
        /// <seealso cref="DungeonMap"/>
        /// </summary>
        /// <returns>This actor has been explored by the active user.</returns>
        public bool IsExplored() => Map.IsExplored(X, Y);

        /// <summary>
        /// Whether this actor has been explored by the active user.
        /// <seealso cref="DungeonMap"/>
        /// </summary>
        /// <returns>This actor has been explored by the active user.</returns>
        public bool IsInFov() => Map.IsInFov(X, Y);
    }
}
