using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using AmoebaRL.Core;
using AmoebaRL.Core.Organelles;
using AmoebaRL.Interfaces;
using AmoebaRL.UI;
using RLNET;

namespace AmoebaRL.Systems
{
    public class MessageLog
    {
        public enum Mode
        {
            MESSAGE,
            ORGANELLE,
            EXAMINE
        }

        public Mode Showing { get; private set; } = Mode.MESSAGE;

        // Define the maximum number of lines to store
        private static readonly int _maxLines = InfoConsole.INFO_HEIGHT - 2;

        // Use a Queue to keep track of the lines of text
        // The first line added to the log will also be the first removed
        private readonly Queue<string> _lines;

        public void Toggle()
        {
            if (Showing == Mode.MESSAGE)
                Showing = Mode.ORGANELLE;
            else if (Showing == Mode.ORGANELLE)
                Showing = Mode.MESSAGE;
            else if (Showing == Mode.EXAMINE)
                Showing = Mode.MESSAGE;
        }

        public void ExamineMode()
        {
            Showing = Mode.EXAMINE;
        }

        public MessageLog()
        {
            _lines = new Queue<string>();
        }

        public void Add(string message)
        {
            int maxLen = InfoConsole.INFO_WIDTH - 2;
            if (message.Length <= maxLen)
            {
                _lines.Enqueue(message);

                // When exceeding the maximum number of lines remove the oldest one.
                if (_lines.Count > _maxLines)
                {
                    _lines.Dequeue();
                }
            }
            else
            {
                List<string> wrapped = WrapText(message, maxLen);
                foreach (string s in wrapped)
                    Add(s);
            }
            
        }

        // Draw each line of the MessageLog queue to the console
        public void Draw(RLConsole console)
        {
            switch (Showing)
            {
                case Mode.MESSAGE:
                    DrawMessage(console);
                    break;
                case Mode.ORGANELLE:
                    DrawOrganelle(console);
                    break;
                case Mode.EXAMINE:
                    DrawExamine(console);
                    break;
            }
        }

        public void DrawMessage(RLConsole console)
        {
            console.Clear();
            string[] lines = _lines.ToArray();
            for (int i = 0; i < lines.Length; i++)
            {
                console.Print(1, i + 1, lines[i], RLColor.White);
            }
        }
        public void DrawOrganelle(RLConsole console)
        {
            List<Actor> toDrawSet = Game.OrganelleLog.GetLoggable();
            if (toDrawSet.Count == 0)
                return;
            if (Game.OrganelleLog.idx >= toDrawSet.Count)
                Game.OrganelleLog.idx = 0;
            Actor toDraw = Game.OrganelleLog.GetLoggable()[Game.OrganelleLog.idx];
            if (toDraw is IDescribable d)
                Describe(console, d);
            else
                console.Clear();
        }

        public void DrawExamine(RLConsole console)
        {
            if(Game.ExamineCursor != null)
            {
                IDescribable toDraw = Game.ExamineCursor.Under();
                if (toDraw != null)
                    Describe(console, toDraw);
                else
                    console.Clear();
            }
            else
                console.Clear();
        }

        public void Describe(RLConsole console, IDescribable toDescribe)
        {
            console.Clear();
            RLColor nameColor = Palette.TextHeading;
            if (toDescribe is Organelle)
                nameColor = Palette.Slime;
            else if (toDescribe is Militia || toDescribe is City)
                nameColor = Palette.Militia;
            else if (toDescribe is Item)
                nameColor = Palette.RootOrganelle;
            console.Print(1, 1, toDescribe.Name, nameColor);
            int maxLen = InfoConsole.INFO_WIDTH - 2;
            string desc = toDescribe.GetDescription();
            int row = 3;
            List<string> wrapped = WrapText(desc, maxLen);
            foreach (string s in wrapped)
                console.Print(1, row++, s, Palette.TextHeading);
            if(toDescribe is Upgradable u)
            {
                Upgradable.UpgradePath status = u.CurrentPath;
                if (status == null)
                {
                    foreach(Upgradable.UpgradePath p in u.PossiblePaths)
                    {
                        string mat = CraftingMaterial.ResourceName(p.TypeRequired);
                        console.Print(1, row++, $"It can be upgraded with {p.AmountRequired} {mat}.", Palette.TextHeading);
                        console.SetColor(27, row - 1, mat.Length, 1, ResourceColor(p.TypeRequired));
                    }
                }
                else
                {
                    string mat = CraftingMaterial.ResourceName(status.TypeRequired);
                    int remaining = status.AmountRequired - u.Progress;
                    console.Print(1, row++, $"It needs {remaining} more {mat}.", Palette.TextHeading);
                    console.SetColor(17, row - 1, mat.Length, 1, ResourceColor(status.TypeRequired));
                }
            }
            if(toDescribe is Actor a)
            {
                Item on = Game.DMap.GetItemAt(a.X, a.Y);
                if(on != null)
                {    
                    console.Print(1, row++, $"It is standing on a {on.Name}.", Palette.TextHeading);
                    console.SetColor(21, row - 1, on.Name.Length, 1, on.Color);
                }
            }
        }

        public static RLColor ResourceColor(CraftingMaterial.Resource toColor)
        {
            if (toColor == CraftingMaterial.Resource.CALCIUM)
                return Palette.Calcium;
            else if (toColor == CraftingMaterial.Resource.ELECTRONICS)
                return Palette.Hunter;
            else
                return Palette.TextHeading;
        }

        /// <summary>
        /// 
        /// <seealso cref="https://stackoverflow.com/questions/3961278/word-wrap-a-string-in-multiple-lines"/>
        /// </summary>
        /// <param name="text"></param>
        /// <param name="lineWidth"></param>
        /// <returns></returns>
        public static List<string> WrapText(string text, int lineWidth)
        {
            const string space = " ";
            string[] words = text.Split(new string[] { space }, StringSplitOptions.None);
            int spaceLeft = lineWidth;
            List<string> output = new List<string>();
            StringBuilder buffer = new StringBuilder();

            foreach (string word in words)
            {
                int wordWidth = word.Length;
                if (wordWidth + 1 > spaceLeft)
                {
                    output.Add(buffer.ToString());
                    buffer.Clear();
                    spaceLeft = lineWidth - wordWidth;
                }
                else
                {
                    spaceLeft -= (wordWidth + 1);
                }
                buffer.Append(word + space);
            }
            if (!(buffer.Length == 0))
                output.Add(buffer.ToString());

            return output;
        }
    }
}
