﻿using AmoebaRL.Behaviors;
using AmoebaRL.Interfaces;
using AmoebaRL.Systems;
using AmoebaRL.UI;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Core
{
    public class City : TutorialMonster
    {
        private readonly IBehavior turn;

        public City()
        {
            Awareness = 0;
            Symbol = 'C';
            Name = "City";
            Color = Palette.City;
            Speed = 16;
            turn = new SpawnGrunt();
        }

        public override void PerformAction(CommandSystem commandSystem)
        {
            turn.Act(this, commandSystem);
        }
    }
}
