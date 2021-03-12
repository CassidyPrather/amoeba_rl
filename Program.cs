using System;

namespace AmoebaRL
{
    public static class Program
    {
        public static bool PlayAgain = false;

        static void Main(string[] args)
        {
            if (args.Length >= 1 && args[0].Equals("-gj"))
                Console.WriteLine("GJ mode enabled."); // hello world
            do
            {
                PlayAgain = false;
                Game g = new Game();
                g.Play();
                Game.Clean();
            } while (PlayAgain);
        }
    }
}
