using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using AmoebaRL.UI;
using RLNET;

namespace AmoebaRL.Systems
{
    public class MessageLog
    {
        // Define the maximum number of lines to store
        private static readonly int _maxLines = InfoConsole.INFO_HEIGHT - 2;

        // Use a Queue to keep track of the lines of text
        // The first line added to the log will also be the first removed
        private readonly Queue<string> _lines;

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
            console.Clear();
            string[] lines = _lines.ToArray();
            for (int i = 0; i < lines.Length; i++)
            {
                console.Print(1, i + 1, lines[i], RLColor.White);
            }
        }
    }
}
