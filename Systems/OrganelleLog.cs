using AmoebaRL.Core;
using AmoebaRL.Core.Organelles;
using AmoebaRL.Interfaces;
using AmoebaRL.UI;
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
        private static readonly int _maxLines = PlayerConsole.PLAYER_HEIGHT - 4;
        private static readonly int _nameWidth = PlayerConsole.PLAYER_WIDTH - 4;

        public int idx = 0; // Select an organelle
        public int page = 0; // Scroll through huge organelle lists

        // Draw each line of the MessageLog queue to the console
        public void Draw(RLConsole console)
        {
            List<Actor> loggable = Game.PlayerMass.Where(a => !(a is Cytoplasm)).ToList();
            console.Clear();
            console.SetBackColor(0, 0, console.Width, console.Height, Palette.DarkSlime);
            console.Print(1, 1, "Organelles", Palette.TextHeading);
            console.Print(1, 2, $"Mass: {Game.PlayerMass.Count}", Palette.TextBody);
            for (int i = page * _maxLines; i < loggable.Count(); i++)
            {
                Actor target = loggable[i];
                int row = i + 4 - page * _maxLines;

                if (i == idx)
                {
                    console.Print(1, row, ">", Palette.TextHeading);
                    console.Print(3, row, target.Name, Palette.TextHeading);
                }
                else
                {
                    console.Print(3, row, target.Name, Palette.TextBody);
                }

                if (target is IDigestable I)
                {
                    int digestionCutoff = (int)Math.Floor(_nameWidth * (1-((float)I.HP / (float)I.MaxHP)));
                    console.SetBackColor(3, row, digestionCutoff, 1, Palette.Slime);
                    console.SetColor(3, row, digestionCutoff, 1, Palette.MembraneInactive);
                    console.SetBackColor(3 + digestionCutoff, row, _nameWidth - digestionCutoff, 1, Palette.Membrane);
                    console.SetColor(3 + digestionCutoff, row, _nameWidth - digestionCutoff, 1, Palette.Militia);
                }
            }
        }
    }
}
