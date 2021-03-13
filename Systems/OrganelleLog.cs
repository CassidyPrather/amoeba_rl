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

        public float NiceTurnBuffer { get; protected set; } = 0;

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
            console.SetBackColor(0, 0, console.Width, console.Height, Palette.OrganelleConsoleBG);
            console.Print(1, 1, "Organelles", Palette.TextHeading);
            console.Print(1, 2, $"Mass: {Game.PlayerMass.Count}", Palette.TextBody);
            float niceturn = ((float)Game.SchedulingSystem.GetTime()) / (16f);
            niceturn = Math.Max(niceturn, NiceTurnBuffer);
            NiceTurnBuffer = niceturn;
            console.Print(1, 3, $"Turn: {niceturn}", Palette.TextBody);
            for (int i = page * _maxLines; i < loggable.Count(); i++)
            {
                Actor target = loggable[i];
                int row = i + 5 - page * _maxLines;

                if(Game.ExamineCursor != null)
                {
                    IDescribable examined = Game.ExamineCursor.Under();
                    if(examined != null && examined == target)
                    {
                        console.Print(1, row, ">", Palette.Cursor);
                    }
                }

                if (i == idx && Game.MessageLog.Showing == MessageLog.Mode.ORGANELLE)
                {
                    Highlighted = target as Organelle;
                    console.Print(1, row, ">", Palette.TextHeading);
                    console.Print(3, row, target.Name, Palette.TextHeading);
                }
                {
                    RLColor nameColor = target.Color;
                    // if (nameColor.r == Palette.DarkSlime.r && nameColor.g == Palette.DarkSlime.g && nameColor.b == Palette.DarkSlime.b)
                    if (nameColor.Equals(Palette.DarkSlime))
                        nameColor = Palette.Slime;
                    console.Print(3, row, target.Name, nameColor);
                }

                if(target is Chloroplast c)
                {
                    int nextProductCutoff = (int)Math.Floor(_nameWidth * (1 - ((float)c.NextFood / (float)c.Delay)));
                    console.SetBackColor(3, row, nextProductCutoff, 1, Palette.Overfill);
                    console.SetColor(3, row, nextProductCutoff, 1, Palette.RootOrganelle);
                }

                if (target is IDigestable dig)
                {
                    int digestionCutoff = (int)Math.Floor(_nameWidth * (1-((float)dig.HP / (float)dig.MaxHP)));
                    console.SetBackColor(3, row, digestionCutoff, 1, Palette.Slime);
                    console.SetColor(3, row, digestionCutoff, 1, target.Color);
                    console.SetBackColor(3 + digestionCutoff, row, _nameWidth - digestionCutoff, 1, Palette.RootOrganelle);
                    console.SetColor(3 + digestionCutoff, row, _nameWidth - digestionCutoff, 1, target.Color);
                    if(dig.Overfill > 0)
                    { 
                        int overfullCutoff = (int)Math.Floor(_nameWidth * ((float)dig.Overfill / (float)(dig.MaxHP)));
                        console.SetBackColor(3, row, overfullCutoff, 1, Palette.Overfill);
                        console.SetColor(3, row, overfullCutoff, 1, target.Color);
                    }
                }
                else if(target is Upgradable up && up.CurrentPath != null)
                {
                    int upgradeCutoff = (int)Math.Floor(_nameWidth * ((float)up.Progress / (float)up.CurrentPath.AmountRequired));
                    RLColor barBG = Palette.RootOrganelle;
                    RLColor bar = Palette.Slime;
                    RLColor text = Palette.Militia;
                    RLColor barText = Palette.RootOrganelle;
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

                    if (!(up is Chloroplast))
                    {
                        // Chloroplasts have a progress bar we don't want to overwrite.
                        console.SetBackColor(3 + upgradeCutoff, row, _nameWidth - upgradeCutoff, 1, barBG);
                        console.SetColor(3 + upgradeCutoff, row, _nameWidth - upgradeCutoff, 1, text);
                    }
                    console.SetBackColor(3, row, upgradeCutoff, 1, bar);
                    console.SetColor(3, row, upgradeCutoff, 1, barText);
                    
                }

                if(target is Nucleus n)
                {
                    List<Actor> nuclei = Game.PlayerMass.Where(a => a is Nucleus).ToList();
                    if(nuclei.Count > 2)
                    { 
                        int curIdx = nuclei.IndexOf(Game.Player);
                        int listedIdx = nuclei.IndexOf(n);
                        int diff = listedIdx - curIdx;
                        if (diff == 0)
                            console.Print(console.Width - 1, row, "@", Palette.Player, Palette.Slime);
                        else if (diff == -1 || curIdx == 0 && listedIdx == nuclei.Count-1)
                            console.Print(console.Width - 1, row, "A", Palette.Player, Palette.Slime);
                        else if (diff == 1 || curIdx == nuclei.Count - 1 && listedIdx == 0)
                            console.Print(console.Width - 1, row, "D", Palette.Player, Palette.Slime);
                    }
                }
            }

            if(loggable.Count > _maxLines)
            {
                console.Print(0, _maxLines - 1, $"Q{(char)30}", Palette.TextHeading, Palette.Slime);
                console.Print(console.Width-2, _maxLines - 1, $"E{(char)31}", Palette.TextHeading, Palette.Slime);
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
