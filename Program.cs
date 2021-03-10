using System;

namespace AmoebaRL
{
    public static class Program
    {
        static void Main(string[] args)
        {
            if (args.Length >= 1 && args[0].Equals("-gj"))
                Console.WriteLine("GJ mode enabled.");
            Game g = new Game();
            g.Play();
        }
    }
}
