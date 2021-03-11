using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using AmoebaRL.Core.Organelles;
using AmoebaRL.Interfaces;
using AmoebaRL.UI;

namespace AmoebaRL.Core.Organelles
{
    public class Nucleus : Upgradable
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
        public override List<Item> Components() => new List<Item>() { new DNA(), new Nutrient() };


        public void SetAsActiveNucleus()
        {
            Game.Player = this;
            Color = Palette.Player;
            List<Actor> otherNucleus = Game.PlayerMass.Where(a => a is Nucleus && a != this).ToList();
            foreach (Nucleus n in otherNucleus)
            {
                n.Color = Palette.PlayerInactive;
                Game.SchedulingSystem.Remove(n);
                Game.SchedulingSystem.Add(n);
            }
        }

        public override void OnUnslime()
        {
            base.OnUnslime();
            HandleGameOver();
        }

        public override void OnDestroy()
        {
            base.OnDestroy();
            HandleGameOver();
        }

        public void HandleGameOver()
        {
            if (Game.DMap.Actors.Where(a => a is Nucleus).Count() == 0)
            {
                Game.MessageLog.Add($"You lose. Final Score: {Game.PlayerMass.Count}.");
                Game.DMap.AddActor(new PostMortem());
            }
        }

        public Organelle Retreat()
        {
            List<Actor> sacrifices = Game.PlayerMass.Where(a => DungeonMap.TaxiDistance(this, a) == 1 && !(a is Nucleus) && (a is Organelle)).ToList();
            if (sacrifices.Count > 0)
            {
                List<Actor> bestSacrifices = sacrifices.Where(s => !(Game.DMap.GetVFX(s.X,s.Y) is Hunter.Reticle)).ToList();
                if (bestSacrifices.Count == 0)
                    bestSacrifices = sacrifices;
                // Prioritize retreating into organelles?
                int r = Game.Rand.Next(0, bestSacrifices.Count - 1);
                Game.DMap.Swap(this, bestSacrifices[r]);
                return bestSacrifices[r] as Organelle;
            }
            return null;
        }

        public override List<Item> OrganelleComponents()
        {
            throw new NotImplementedException();
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
