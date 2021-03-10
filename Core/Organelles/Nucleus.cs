using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using AmoebaRL.Core.Organelles;
using AmoebaRL.UI;

namespace AmoebaRL.Core.Organelles
{
    public class Nucleus : Organelle
    {
        public Nucleus()
        {
            Awareness = 4;
            Name = "Nucleus";
            Color = Palette.PlayerInactive;
            Symbol = '@';
            X = 10;
            Y = 10;
            Slime = true;
            Speed = 16;
        }

        public void SetAsActiveNucleus()
        {
            Game.Player = this;
            Color = Palette.Player;
            List<Actor> otherNucleus = Game.PlayerMass.Where(a => a is Nucleus && a != this).ToList();
            foreach(Nucleus n in otherNucleus)
            {
                n.Color = Palette.PlayerInactive;
                Game.SchedulingSystem.Remove(n);
                Game.SchedulingSystem.Add(n);
            }
        }

        public override void OnDestroy()
        {
            BecomeItem(new DNA());
            if (Game.DMap.Actors.Where(a => a is Nucleus).Count() == 0)
            {
                Game.MessageLog.Add($"You lose. Final Score: {Game.PlayerMass.Count}.");
                Game.DMap.AddActor(new PostMortem());
            }
        }

        public Actor Retreat()
        {
            List<Actor> sacrifices = Game.PlayerMass.Where(a => DungeonMap.TaxiDistance(this, a) == 1 && !(a is Nucleus)).ToList();
            if (sacrifices.Count > 0)
            {
                int r = Game.Rand.Next(0, sacrifices.Count - 1);
                Game.DMap.Swap(this, sacrifices[r]);
                return sacrifices[r];
            }
            return null;
        }
    }

    public class DNA : Catalyst
    {
        public DNA()
        {
            Name = "DNA";
            Color = Palette.Player;
            Symbol = 'X';
        }

        public override Actor NewOrganelle() => new Nucleus();
    }
}
