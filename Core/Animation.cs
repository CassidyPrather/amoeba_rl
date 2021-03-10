using RLNET;
using RogueSharp;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Core
{
    public abstract class Animation : VFX
    {
        /// <summary>
        /// The number of differnet frames the animation has.
        /// </summary>
        public int Frames { get; protected set; }

        /// <summary>
        /// The number of <see cref="DungeonMap.ANIMATION_RATE"/>s to pass before updating the animation.
        /// Higher numbers = slower transitions.
        /// </summary>
        public int Speed { get; protected set; }

        public override void Draw(RLConsole console, IMap map)
        {
            SetFrame((Game.DMap.AnimationFrame / Speed) % Frames);
            base.Draw(console, map);
        }

        public abstract void SetFrame(int idx);
    }
}
