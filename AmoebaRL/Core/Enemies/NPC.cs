using AmoebaRL.Core.Organelles;
using AmoebaRL.Interfaces;
using AmoebaRL.UI;
using RogueSharp;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Core.Enemies
{
    /// <summary>
    /// Something which exists within the map and takes basic actions and can be interacted with
    /// through most standard mechanisms.
    /// </summary>
    public abstract class NPC : Actor, IProactive, IEatable, ISlayable, IDescribable, IEngulfable
    {
        public virtual int Armor { get; set; } = 0;

        public virtual string Description => $"{DescBody} {Flavor}";

        public virtual string DescBody => "";

        public virtual string Flavor => "";

        public NPC() => Init();

        public virtual void Init()
        {
            Awareness = 0;
            Color = Palette.Cursor;
            Symbol = '?';
            Speed = 16;
            Name = "NPC";
        }

        public virtual void Die()
        {
            Game.DMap.RemoveActor(this);
            BecomeItems(BecomesOnDie);
        }

        public virtual void OnEaten()
        {
            Game.DMap.RemoveActor(this);
            Actor result = BecomeActor(BecomesOnEaten);
            Game.PlayerMass.Add(result);
        }

        public virtual bool Engulf()
        {
            HashSet<IEngulfable> toEngulf = new HashSet<IEngulfable>() { this };
            if (CanEngulf(toEngulf))
            {
                foreach (IEngulfable i in toEngulf)
                    i.ProcessEngulf();
                return true;
            }
            return false;
        }

        // This could be augmented with the floodfill library.
        public virtual bool CanEngulf(HashSet<IEngulfable> engulfing = null)
        {
            if (engulfing == null)
                engulfing = new HashSet<IEngulfable>() { this };
            else
                engulfing.Add(this);
            List<ICell> adj = Game.DMap.Adjacent(X, Y);
            if (adj.Where(a => Game.DMap.IsWalkable(a.X, a.Y)).Count() > 0)
                return false; // An escape route exists.
            List<Actor> adjActors = new List<Actor>();
            foreach (ICell a in adj)
            {
                Actor adjacentActor = Game.DMap.GetActorAt(a.X, a.Y);
                if (adjacentActor != null)
                {
                    if (adjacentActor is City)
                        return false; // Cities will not help to engulf their friends! We can escape!
                    adjActors.Add(adjacentActor);
                }
            }
            // The only remaining places to check are adjacent actors.
            bool hasEscape = false; // Assume each actor locks us in unless it specifically won't.
            foreach (Actor a in adjActors)
            {
                if (a is IEngulfable e && !engulfing.Contains(e))
                {
                    // We cannot ignore e, which could mean we are sealed in
                    if (!e.CanEngulf(engulfing))
                        hasEscape = true; // Since a neighbor was able to escape, we can too.
                }
            }
            return !hasEscape;
        }

        public virtual void ProcessEngulf()
        {
            Game.MessageLog.Add($"The {Name} is engulfed!");
            OnEaten();
        }

        public abstract void Act();

        public virtual List<Item> BecomesOnDie => new List<Item>();

        public virtual Actor BecomesOnEaten => new Cytoplasm();
    }

    public abstract class DissolvingNPC : Organelle, IProactive, IDigestable
    {
        public virtual int MaxHP { get; set; } = 1;

        public virtual int HP { get; set; } = 1;

        public virtual int Overfill { get; set; } = 0;

        public DissolvingNPC() => Init();

        public virtual void Init()
        {
            Awareness = 0;
            Slime = 1;
            Color = Palette.Cursor;
            Symbol = '?';
            Speed = 16;
        }

        public virtual void Act()
        {
            HP--;
            if (HP <= 0)
            {
                int numButchers = Game.PlayerMass.Where(k => k is Butcher).Count();
                for (int i = 0; i < numButchers; i++)
                {
                    Overfill = MaxHP * 2;
                    ProduceIfOverfull();
                }
                Game.DMap.RemoveActor(this);
                Actor transformation = DigestsTo;
                transformation.X = X;
                transformation.Y = Y;
                Game.DMap.AddActor(transformation);
                Game.PlayerMass.Add(transformation);
                Game.DMap.UpdatePlayerFieldOfView();
            }
        }
        public virtual bool ProduceIfOverfull()
        {
            if (Overfill >= MaxHP)
            {
                List<ICell> drops = Game.DMap.NearestNoActor(X, Y);
                if (drops.Count > 0)
                {
                    ICell pick = drops[Game.Rand.Next(drops.Count - 1)];
                    Actor bounty = DigestsTo;
                    bounty.X = pick.X;
                    bounty.Y = pick.Y;
                    Game.DMap.AddActor(bounty);
                    Game.PlayerMass.Add(bounty);
                    Overfill = 0;
                    Game.DMap.UpdatePlayerFieldOfView();
                }
                return true;
            }
            return false;
        }

        public abstract Actor DigestsTo { get; }

        public abstract Actor RescuesTo { get; }


        public override void OnUnslime() => BecomeActor(RescuesTo);

        public override void OnDestroy() => BecomeActor(RescuesTo);

        public virtual string DissolvingAddendum()
        {
            if (!Game.DMap.AdjacentActors(X, Y).Where(a => a is Cultivator).Any())
            {
                return $"After {HP} turns, it will become {NameOfResult}. Be careful, it can still be rescued!";
            }
            else
            {
                return $"Kept alive against its will. It is regenerating up to {MaxHP} HP. After {MaxHP - Overfill} turns," +
                    $" it will produce {NameOfResult}. Be careful, it can still be rescued!";
            }
        }

        public override string DescBody => DissolvingAddendum();

        private string CachedResultName;

        public string NameOfResult
        {
            get
            {
                // Null coalescing not available until C# 8:
                // CachedResultName ??= DigestsTo.Name;
                if (CachedResultName != null)
                    CachedResultName = DigestsTo.Name;
                return CachedResultName;
            }
        }
    }
}
