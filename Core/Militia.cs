using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using AmoebaRL.Behaviors;
using AmoebaRL.Interfaces;
using AmoebaRL.Systems;
using AmoebaRL.UI;

namespace AmoebaRL.Core
{
    public class Militia : Monster, IEatable
    {
        public Militia()
        {
            Awareness = 3;
            Color = Palette.Militia;
            Symbol = 'm';
            Speed = 16;
            Name = "Militia";
        }

        public void OnEaten()
        {
            Game.DMap.RemoveActor(this);
            CapturedMilitia transformation = new CapturedMilitia
            {
                X = X,
                Y = Y
            };
            Game.DMap.AddActor(transformation);
        }

        public class CapturedMilitia : Actor, IProactive
        {
            int HP;

            public CapturedMilitia()
            {
                Awareness = 1;
                Slime = true;
                Color = Palette.Militia;
                Name = "Dissolving Militia";
                Symbol = 'm';
                HP = 8;
                Speed = 16;
                Game.PlayerMass.Add(this);
            }

            public bool Act()
            {
                HP--;
                if(HP <= 0)
                {
                    Game.DMap.RemoveActor(this);
                    Cytoplasm transformation = new Cytoplasm
                    {
                        X = X,
                        Y = Y
                    };
                    Game.DMap.AddActor(transformation);
                    Game.PlayerMass.Add(transformation);
                }
                return true;
            }
        }

        public override void PerformAction(CommandSystem commandSystem)
        {
            IBehavior behavior = new MilitiaPatrolAttack();//StandardMoveAndAttack(); //
            behavior.Act(this, commandSystem);
        }
    }
}
