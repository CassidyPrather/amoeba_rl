using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using AmoebaRL.Core;
using AmoebaRL.Core.Enemies;
using AmoebaRL.Core.Organelles;
using AmoebaRL.Interfaces;
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
        public Queue<string> Lines { get; protected set; }

        public MessageLog()
        {
            Lines = new Queue<string>();
        }

        public void Add(string message)
        {
            int maxLen = InfoConsole.INFO_WIDTH - 2;
            if (message.Length <= maxLen)
            {
                Lines.Enqueue(message);

                // When exceeding the maximum number of lines remove the oldest one.
                if (Lines.Count > _maxLines)
                {
                    Lines.Dequeue();
                }
            }
            else
            {
                List<string> wrapped = WrapText(message, maxLen);
                foreach (string s in wrapped)
                    Add(s);
            }
            
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
