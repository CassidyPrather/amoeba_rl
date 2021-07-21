using System;
using System.Linq;

namespace AmoebaRL
{
    public static class Program
    {
        public static bool PlayAgain = false;

        static void Main(string[] args)
        {
            // if (args.Length >= 1 && args[0].Equals("-gj"))
            //    Console.WriteLine("GJ mode enabled."); // hello world
            Game.GameConfigurationSchema options = null;
            if (args.Any(x => x.ToLower().Equals("--gj")))
            {
                options = new()
                {
                    MapHeight = 48,
                    NumCities = 16,
                    MapWidth = 64,
                    DefaultSpawnRate = 50,
                    EvolutionRate = 5,
                    CityArmor = 160,
                    MaxBudget = 6,
                    GraceCities = 0
                };
                Console.WriteLine("GJ mode enabled.");
            }
            else if(args.Any(x => x.ToLower().Equals("--easy")))
            {
                options = new()
                {
                    MapHeight = 48,
                    NumCities = 10,
                    MapWidth = 48,
                    DefaultSpawnRate = 75,
                    EvolutionRate = 7,
                    CityArmor = 100,
                    MaxBudget = 5,
                    GraceCities = 4
                };
                Console.WriteLine("Easy mode enabled.");
            }
            do
            {
                PlayAgain = false;
                Game g = new(options);
                g.Clean();
            } while (PlayAgain);
        }
    }
}
