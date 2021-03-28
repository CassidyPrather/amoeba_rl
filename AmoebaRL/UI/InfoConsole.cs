using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using RogueSharp;
using RLNET;
using AmoebaRL.Systems;
using AmoebaRL.Core;
using AmoebaRL.Interfaces;
using AmoebaRL.Core.Organelles;
using AmoebaRL.Core.Enemies;

namespace AmoebaRL.UI
{
    class InfoConsole : RLConsole
    {
        public static readonly int INFO_WIDTH = MapConsole.MAP_WIDTH;
        public static readonly int INFO_HEIGHT = 11;

        public InfoConsole() : base(INFO_WIDTH, INFO_HEIGHT)
        {
            SetBackColor(0, 0, Width, Height, Palette.PrimaryDarker);
            Print(1, 1, "Log", Palette.TextHeading);
        }

        public void OnUpdate(object sender, UpdateEventArgs e)
        {
            
        }

        public void DrawContent(Game context)
        {
            switch (context.Showing)
            {
                case Game.Mode.MESSAGE:
                    DrawMessage(context);
                    break;
                case Game.Mode.ORGANELLE:
                    DrawOrganelle(context);
                    break;
                case Game.Mode.EXAMINE:
                    DrawExamine(context);
                    break;
            }
        }

        public void DrawMessage(Game context)
        {
            Clear();
            string[] lines = context.MessageLog.Lines.ToArray();
            for (int i = 0; i < lines.Length; i++)
            {
                Print(1, i + 1, lines[i], RLColor.White);
            }
        }
        public void DrawOrganelle(Game context)
        {
            OrganelleLog activeLog = context.OrganelleLog;
            List<Actor> toDrawSet = activeLog.GetLoggable();
            if (toDrawSet.Count == 0)
                return;
            if (activeLog.idx >= toDrawSet.Count)
                activeLog.idx = 0;
            Actor toDraw = activeLog.GetLoggable()[context.OrganelleLog.idx];
            if (toDraw is IDescribable d)
                Describe(d);
            else
                Clear();
        }

        public void DrawExamine(Game context)
        {
            if (context.ExamineCursor != null)
            {
                IDescribable toDraw = context.ExamineCursor.Under();
                if (toDraw != null)
                    Describe(toDraw);
                else
                    Clear();
            }
            else
                Clear(); // Should never happen
        }

        /// <summary>
        /// Describes an <see cref="IDescribable"/> in this canvas.
        /// </summary>
        /// <param name="toDescribe">The <see cref="IDescribable"/> to display information about.</param>
        /// <remarks>Will need to update to include a better color system when attributes come around.</remarks>
        public void Describe(IDescribable toDescribe)
        {
            Clear();
            RLColor nameColor = Palette.TextHeading;
            if (toDescribe is Organelle)
                nameColor = Palette.Slime;
            else if (toDescribe is Militia || toDescribe is City)
                nameColor = Palette.Militia;
            else if (toDescribe is Item)
                nameColor = Palette.RootOrganelle;
            Print(1, 1, toDescribe.Name, nameColor);
            int maxLen = InfoConsole.INFO_WIDTH - 2;
            string desc = toDescribe.Description;
            int row = 3;
            List<string> wrapped = MessageLog.WrapText(desc, maxLen);
            foreach (string s in wrapped)
                Print(1, row++, s, Palette.TextHeading);
            if (toDescribe is Upgradable u)
            {
                Upgradable.UpgradePath status = u.CurrentPath;
                if (status == null)
                {
                    foreach (Upgradable.UpgradePath p in u.PossiblePaths)
                    {
                        string mat = CraftingMaterial.ResourceName(p.TypeRequired);
                        Print(1, row++, $"It can be upgraded with {p.AmountRequired} {mat}.", Palette.TextHeading);
                        SetColor(27, row - 1, mat.Length, 1, ResourceColor(p.TypeRequired));
                    }
                }
                else
                {
                    string mat = CraftingMaterial.ResourceName(status.TypeRequired);
                    int remaining = status.AmountRequired - u.Progress;
                    Print(1, row++, $"It needs {remaining} more {mat}.", Palette.TextHeading);
                    SetColor(17, row - 1, mat.Length, 1, ResourceColor(status.TypeRequired));
                }
            }
            if (toDescribe is Actor a)
            {
                Item on = a.Map.GetItemAt(a.X, a.Y);
                if (on != null)
                {
                    Print(1, row++, $"It is standing on a {on.Name}.", Palette.TextHeading);
                    SetColor(21, row - 1, on.Name.Length, 1, TextTilePalette.Represent(on).Color);
                }
            }
        }

        public static RLColor ResourceColor(CraftingMaterial.Resource toColor)
        {
            if (toColor == CraftingMaterial.Resource.CALCIUM)
                return Palette.Calcium;
            else if (toColor == CraftingMaterial.Resource.ELECTRONICS)
                return Palette.Electronics;
            else
                return Palette.TextHeading;
        }
    }
}
