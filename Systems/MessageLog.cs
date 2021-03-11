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

        // Add a line to the MessageLog queue
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
                string nextChunk = message.Substring(0, maxLen);
                string remainder = message.Substring(maxLen, message.Length - maxLen);
                Add(nextChunk);
                Add(remainder);
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
            Actor toDraw = Game.OrganelleLog.GetLoggable()[Game.OrganelleLog.idx];
            if (toDraw is IDescribable d)
                Describe(console, d);
            else
                Console.Clear();
        }

        public void DrawExamine(RLConsole console)
        {
            if(Game.cursor != null)
            {
                IDescribable toDraw = Game.cursor.Under();
                if (toDraw != null)
                    Describe(console, toDraw);
                else
                    Console.Clear();
            }
            else
                Console.Clear();
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
                nameColor = Palette.Membrane;
            console.Print(1, 1, toDescribe.Name, nameColor);
            int maxLen = InfoConsole.INFO_WIDTH - 2;
            string desc = toDescribe.GetDescription();
            int row = 3;
            while(desc.Length > 0)
            {
                string nextChunk = desc.Substring(0, Math.Min(maxLen,desc.Length));
                if (desc.Length >= maxLen)
                    desc = desc.Substring(maxLen, desc.Length - maxLen);
                else
                    desc = "";
                console.Print(1, row++, nextChunk, Palette.TextHeading);
            }
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

    }
}
