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

        public static Cursor HighlightCursor { get; protected set; } = null;

        public Organelle Highlighted { get; private set; }

        public List<Actor> GetLoggable() => Game.PlayerMass.Where(a => !(a is Cytoplasm)).ToList();

        // Draw each line of the MessageLog queue to the console
        public void Draw(RLConsole console)
        {
            if(Game.MessageLog.Showing == MessageLog.Mode.ORGANELLE)
            {
                if(Highlighted != null)
                {
                    if (HighlightCursor == null)
                    {
                        HighlightCursor = new Cursor();
                        Game.DMap.AddVFX(HighlightCursor);
                    }
                    HighlightCursor.X = Highlighted.X;
                    HighlightCursor.Y = Highlighted.Y;
                }
            }
            else
            {
                if(HighlightCursor != null)
                {
                    Game.DMap.RemoveVFX(HighlightCursor);
                    HighlightCursor = null;
                }
            }

            List<Actor> loggable = GetLoggable();
            console.Clear();
            console.SetBackColor(0, 0, console.Width, console.Height, Palette.DarkSlime);
            console.Print(1, 1, "Organelles", Palette.TextHeading);
            console.Print(1, 2, $"Mass: {Game.PlayerMass.Count}", Palette.TextBody);
            for (int i = page * _maxLines; i < loggable.Count(); i++)
            {
                Actor target = loggable[i];
                int row = i + 4 - page * _maxLines;

                if (i == idx && Game.MessageLog.Showing == MessageLog.Mode.ORGANELLE)
                {
                    Highlighted = target as Organelle;
                    console.Print(1, row, ">", Palette.TextHeading);
                    console.Print(3, row, target.Name, Palette.TextHeading);
                }
                else
                {
                    console.Print(3, row, target.Name, Palette.TextBody);
                }

                if (target is IDigestable dig)
                {
                    int digestionCutoff = (int)Math.Floor(_nameWidth * (1-((float)dig.HP / (float)dig.MaxHP)));
                    console.SetBackColor(3, row, digestionCutoff, 1, Palette.Slime);
                    console.SetColor(3, row, digestionCutoff, 1, Palette.MembraneInactive);
                    console.SetBackColor(3 + digestionCutoff, row, _nameWidth - digestionCutoff, 1, Palette.Membrane);
                    console.SetColor(3 + digestionCutoff, row, _nameWidth - digestionCutoff, 1, Palette.Militia);
                }
                else if(target is Upgradable up && up.CurrentPath != null)
                {
                    int upgradeCutoff = (int)Math.Floor(_nameWidth * (1 - ((float)up.Progress / (float)up.CurrentPath.AmountRequired)));
                    RLColor barBG = Palette.Membrane;
                    RLColor bar = Palette.Slime;
                    RLColor text = Palette.Militia;
                    RLColor barText = Palette.MembraneInactive;
                    if(up.CurrentPath.TypeRequired == CraftingMaterial.Resource.CALCIUM)
                    {
                        barBG = Palette.RestingTank;
                        bar = Palette.Calcium;
                        text = Palette.TextHeading;
                        barText = Palette.TextHeading;
                    }
                    else if(up.CurrentPath.TypeRequired == CraftingMaterial.Resource.ELECTRONICS)
                    {
                        barBG = Palette.ReticleBackground;
                        bar = Palette.Hunter;
                        text = Palette.TextHeading;
                        barText = Palette.TextHeading;
                    }
                    console.SetBackColor(3, row, upgradeCutoff, 1, bar);
                    console.SetColor(3, row, upgradeCutoff, 1, barText);
                    console.SetBackColor(3 + upgradeCutoff, row, _nameWidth - upgradeCutoff, 1, barBG);
                    console.SetColor(3 + upgradeCutoff, row, _nameWidth - upgradeCutoff, 1, text);
                }
            }
        }

        public void Scroll(int by)
        {
            int numItems = GetLoggable().Count();
            if(numItems > 0)
                idx = (idx + by) % numItems;
            if (idx < 0)
                idx += numItems;
        }

        public void Page(int by)
        {
            int numPages = GetLoggable().Count() / _maxLines;
            if(numPages > 0)
                page = (page + by) % numPages;
            if (page < 0)
                page += numPages;
        }
    }
}
