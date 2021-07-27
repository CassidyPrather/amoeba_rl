using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using AmoebaRL.Core;
using AmoebaRL.Core.Organelles;
using AmoebaRL.Interfaces;
using AmoebaRL.Systems;
using RLNET;


namespace AmoebaRL.UI
{
    public class PlayerConsole : RLConsole
    {
        
        public static readonly int PLAYER_WIDTH = 22;
        public static readonly int PLAYER_HEIGHT = MapConsole.MAP_HEIGHT + InfoConsole.INFO_HEIGHT;
        private static readonly int _maxLines = PlayerConsole.PLAYER_HEIGHT - 4;
        private static readonly int _nameWidth = PlayerConsole.PLAYER_WIDTH - 4;

        public PlayerConsole() : base(PLAYER_WIDTH, PLAYER_HEIGHT)
        {
            SetBackColor(0, 0, Width, Height, Palette.DbWood);
            Print(1, 1, "Organelles", Palette.TextHeading);
        }

        public void OnUpdate(object sender, UpdateEventArgs e)
        {
            
        }

        public void DrawContent(Game content)
        {
            // Get info
            OrganelleLog log = content.OrganelleLog;
            List<Actor> loggable = log.GetLoggable();
            IDescribable examined = null;
            if (content.ExamineCursor != null)
                examined = content.ExamineCursor.Under();

            // Clear, set header.
            Clear();
            SetBackColor(0, 0, Width, Height, Palette.OrganelleConsoleBG);
            Print(1, 1, "Organelles", Palette.TextHeading);
            Print(1, 2, $"Mass: {content.DMap.PlayerMass.Count}", Palette.TextBody);
            // -- Header info
            float niceturn = ((float)content.SchedulingSystem.GetTime()) / (16f);
            niceturn = Math.Max(niceturn, log.NiceTurnBuffer);
            log.NiceTurnBuffer = niceturn;
            Print(1, 3, $"Turn: {niceturn}", Palette.TextBody);

            // Determine what page we should show

            int effectivePage = log.page; 
            if (examined != null)
            {
                for (int i = 0; i < loggable.Count; i++)
                { 
                    if(loggable[i] == examined)
                    {
                        effectivePage = i / _maxLines;
                    }
                }
            }
            if(effectivePage != 0)
                effectivePage = effectivePage % ((int)Math.Ceiling((float)loggable.Count() / (float)_maxLines)); 

            // Write each line of the log
            for (int i = effectivePage * _maxLines; i < loggable.Count(); i++)
            {
                Actor target = loggable[i];
                int row = i + 5 - effectivePage * _maxLines;

                if (examined != null && examined == target)
                {
                    Print(1, row, ">", Palette.Cursor);
                }

                if (i == log.idx && content.Showing == Game.Mode.ORGANELLE)
                {
                    Print(1, row, ">", Palette.TextHeading);
                    Print(3, row, target.Name, Palette.TextHeading);
                }
                {
                    RLColor nameColor = TextTilePalette.Represent(target).Color;
                    // if (nameColor.r == Palette.DarkSlime.r && nameColor.g == Palette.DarkSlime.g && nameColor.b == Palette.DarkSlime.b)
                    if (nameColor.Equals(Palette.DarkSlime))
                        nameColor = Palette.Slime;
                    Print(3, row, target.Name, nameColor);
                }

                if (target is Chloroplast c)
                {
                    int nextProductCutoff = (int)Math.Floor(_nameWidth * (1 - ((float)c.NextFood / (float)c.Delay)));
                    SetBackColor(3, row, nextProductCutoff, 1, Palette.Overfill);
                    SetColor(3, row, nextProductCutoff, 1, Palette.RootOrganelle);
                }

                if (target is IDigestable dig)
                {
                    int digestionCutoff = (int)Math.Floor(_nameWidth * (1 - ((float)dig.HP / (float)dig.MaxHP)));
                    SetBackColor(3, row, digestionCutoff, 1, Palette.Slime);
                    SetColor(3, row, digestionCutoff, 1, TextTilePalette.Represent(target).Color);
                    SetBackColor(3 + digestionCutoff, row, _nameWidth - digestionCutoff, 1, Palette.RootOrganelle);
                    SetColor(3 + digestionCutoff, row, _nameWidth - digestionCutoff, 1, TextTilePalette.Represent(target).Color);
                    if (dig.Overfill > 0)
                    {
                        int overfullCutoff = (int)Math.Floor(_nameWidth * ((float)dig.Overfill / (float)(dig.MaxHP)));
                        SetBackColor(3, row, overfullCutoff, 1, Palette.Overfill);
                        SetColor(3, row, overfullCutoff, 1, TextTilePalette.Represent(target).Color);
                    }
                }
                else if (target is Upgradable up && up.CurrentPath != null)
                {
                    int upgradeCutoff = (int)Math.Floor(_nameWidth * ((float)up.Progress / (float)up.CurrentPath.AmountRequired));
                    RLColor barBG = Palette.RootOrganelle;
                    RLColor bar = Palette.Slime;
                    RLColor text = Palette.Militia;
                    RLColor barText = Palette.RootOrganelle;
                    if (up.CurrentPath.TypeRequired == CraftingMaterial.Resource.CALCIUM)
                    {
                        barBG = Palette.RestingTank;
                        bar = Palette.Calcium;
                        text = Palette.TextHeading;
                        barText = Palette.TextHeading;
                    }
                    else if (up.CurrentPath.TypeRequired == CraftingMaterial.Resource.ELECTRONICS)
                    {
                        barBG = Palette.ReticleBackground;
                        bar = Palette.Electronics;
                        text = Palette.TextHeading;
                        barText = Palette.TextHeading;
                    }

                    if (!(up is Chloroplast))
                    {
                        // Chloroplasts have a progress bar we don't want to overwrite.
                        SetBackColor(3 + upgradeCutoff, row, _nameWidth - upgradeCutoff, 1, barBG);
                        SetColor(3 + upgradeCutoff, row, _nameWidth - upgradeCutoff, 1, text);
                    }
                    SetBackColor(3, row, upgradeCutoff, 1, bar);
                    SetColor(3, row, upgradeCutoff, 1, barText);

                }

                if (target is Nucleus n)
                {
                    List<Actor> nuclei = content.DMap.PlayerMass.Where(a => a is Nucleus).ToList();
                    if (nuclei.Count >= 2)
                    {
                        int curIdx = nuclei.IndexOf(content.ActivePlayer);
                        int listedIdx = nuclei.IndexOf(n);
                        int diff = listedIdx - curIdx;
                        if (diff == 0)
                            Print(Width - 1, row, "@", Palette.SuperBright, Palette.Slime);
                        else if (diff == -1 || curIdx == 0 && listedIdx == nuclei.Count - 1)
                            Print(Width - 1, row, "A", Palette.SuperBright, Palette.Slime);
                        else if (diff == 1 || curIdx == nuclei.Count - 1 && listedIdx == 0)
                            Print(Width - 1, row, "D", Palette.SuperBright, Palette.Slime);
                    }
                }
            }

            if (loggable.Count > _maxLines)
            {
                Print(0, _maxLines + 3, $"Q{(char)30}", Palette.TextHeading, Palette.Slime);
                Print(Width - 2, _maxLines + 3, $"E{(char)31}", Palette.TextHeading, Palette.Slime);
            }
        }
    }

}
