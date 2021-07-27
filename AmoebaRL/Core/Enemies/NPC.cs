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
    /// Exists within the map and takes basic actions and can be interacted with
    /// through most standard mechanisms.
    /// </summary>
    public abstract class NPC : Actor, IProactive, IEatable, ISlayable, IDescribable, IEngulfable
    {
        /// <summary>
        /// Resistance to basic attack mechanisms. 0 = no resistance, 1 = immune to basic. 2+ reserved.
        /// </summary>
        public virtual int Armor { get; set; } = 0;

        /// <summary>
        /// A string describing this <see cref="IDescribable"/> to be displayed in a viewport like <see cref="Systems.MessageLog"/>.
        /// Composed of <see cref="Flavor"/>, followed by a space and then <see cref="DescBody"/>.
        /// Does not need to contain <see cref="Actor.Name"/>.
        /// </summary>
        /// <remarks>May eventually be updated to a SadConsole-like formatting standard to enable colors.</remarks>
        public virtual string Description => $"{Flavor} {DescBody}";

        /// <summary>
        /// Informative text describing the mechanical function of this <see cref="IDescribable/>.
        /// </summary>
        public virtual string DescBody => "";

        /// <summary>
        /// Decorative text describing this <see cref="IDescribable"/>
        /// </summary>
        public virtual string Flavor => "";

        /// <summary> Default parameterless constructor calls <see cref="Init"/>, which is expected to be overloaded in most cases to set
        /// basic <see cref="Actor"/> and <see cref="NPC"/> attributes.</summary>
        public NPC() => Init();

        /// <summary>
        /// Called once when the <see cref="NPC"/> is constructed via its default, parameterless constructor.
        /// This is expected to be overloaded in most cases to set basic <see cref="Actor"/> and <see cref="NPC"/> attributes.
        /// Otherwise, it will set some placeholder values.
        /// </summary>
        public virtual void Init()
        {
            Awareness = 0;
            Delay = 16;
            Name = "NPC";
        }

        /// <summary>
        /// Removes from the map and replaces with <see cref="BecomesOnDie"/> via the default <see cref="Actor.BecomeItems(IEnumerable{Item})"/>.
        /// </summary>
        public virtual void Die()
        {
            Map.RemoveActor(this);
            BecomeItems(BecomesOnDie);
        }

        /// <summary>
        /// Removes from the map and replaces with <see cref="BecomesOnEaten"/> via the default <see cref="Actor.BecomeActor(Actor)"/>. Adds the resulting
        /// <see cref="Actor"/> to <see cref="Game.PlayerMass"/>.
        /// </summary>
        /// <remarks>
        /// If other clasess become capable of calling <see cref="IEatable.OnEaten"/> that do not belong to <see cref="Game.PlayerMass"/>, this may need to be
        /// updated to include an overload that parameterizes the collection it goes to.
        /// </remarks>
        public virtual void OnEaten()
        {
            Map.RemoveActor(this);
            Actor result = BecomeActor(BecomesOnEaten);
            Map.PlayerMass.Add(result);
        }

        /// <summary>
        /// Checks <see cref="CanEngulf(HashSet{IEngulfable})"/>, and if it is part of a contiguously engulfable group,
        /// calls <see cref="ProcessEngulf"/> on each <see cref="IEngulfable"/> in the set.
        /// </summary>
        /// <returns>True if the target was engulfed, false otherwise.</returns>
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
        /// <summary>
        /// Determines whether this <see cref="IEngulfable"/> is part of a contiguous group of other <see cref="IEngulfable"/>.
        /// </summary>
        /// <param name="engulfing">All of the <see cref="IEngulfable"/> which have previously been 
        /// determined to be part of an engulfable mass, excluding the result of this call. 
        /// If null, it is generated automatically.</param>
        /// <returns>This individual <see cref="IEngulfable"/> is either part of an engulfable mass or would be if all adjacent <see cref="IEngulfable"/>s are.</returns>
        public virtual bool CanEngulf(HashSet<IEngulfable> engulfing = null)
        {
            if (engulfing == null)
                engulfing = new HashSet<IEngulfable>() { this };
            else
                engulfing.Add(this);
            List<ICell> adj = Map.Adjacent(X, Y);
            if (adj.Where(a => Map.IsWalkable(a.X, a.Y)).Count() > 0)
                return false; // An escape route exists.
            List<Actor> adjActors = new List<Actor>();
            foreach (ICell a in adj)
            {
                Actor adjacentActor = Map.GetActorAt(a.X, a.Y);
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

        /// <summary>
        /// Replace this with <see cref="BecomesOnEaten"/> via <see cref="OnEaten"/> when this is found to be engulfed by <see cref="CanEngulf(HashSet{IEngulfable})"/>.
        /// </summary>
        public virtual void ProcessEngulf()
        {
            Map.Context.MessageLog.Add($"The {Name} is engulfed!");
            OnEaten();
        }

        /// <inheritdoc/>
        public abstract void Act();

        /// <summary>Instantiated constituent elements which make up this <see cref="NPC"/> to be distributed on <see cref="Die"/>.</summary>
        public virtual List<Item> BecomesOnDie => new List<Item>();

        /// <summary>The <see cref="Actor"/> to add to the consuming group on <see cref="OnEaten"/>.</summary>
        public virtual Actor BecomesOnEaten => new Cytoplasm();
    }

    /// <summary>
    /// Version of <see cref="NPC"/> which is no longer proactive and instead primarily functions as an <see cref="Organelle"/> with some transformation methods.
    /// </summary>
    public abstract class DissolvingNPC : Organelle, IProactive, IDigestable
    {
        /// <inheritdoc/>
        public virtual int MaxHP { get; set; } = 1;

        /// <inheritdoc/>
        public virtual int HP { get; set; } = 1;

        /// <inheritdoc/>
        public virtual int Overfill { get; set; } = 0;

        /// <summary>Call <see cref="Init"/></summary>
        public DissolvingNPC() => Init();

        /// <summary> Default parameterless constructor calls <see cref="Init"/>, which is expected to be overloaded in most cases to set
        /// basic <see cref="Actor"/> and <see cref="DissolvingNPC"/> attributes.</summary>
        public virtual void Init()
        {
            Awareness = 0;
            Slime = 1;
            Delay = 16;
        }

        /// <summary>
        /// Handle production.
        /// </summary>
        public virtual void Act()
        {
            HP--;
            if (HP <= 0)
            {
                int numButchers = Map.PlayerMass.Where(k => k is Butcher).Count();
                for (int i = 0; i < numButchers; i++)
                {
                    Overfill = MaxHP * 2;
                    ProduceIfOverfull();
                }
                Map.RemoveActor(this);
                Actor transformation = DigestsTo;
                transformation.X = X;
                transformation.Y = Y;
                Map.AddActor(transformation);
                Map.PlayerMass.Add(transformation);
                //Map.UpdatePlayerFieldOfView();
            }
        }

        /// <summary>
        /// Produce an extra product without destroying the instance.
        /// </summary>
        /// <returns></returns>
        public virtual bool ProduceIfOverfull()
        {
            if (Overfill >= MaxHP)
            {
                List<ICell> drops = Map.NearestNoActor(X, Y);
                if (drops.Count > 0)
                {
                    ICell pick = drops[Map.Context.Rand.Next(drops.Count - 1)];
                    Actor bounty = DigestsTo;
                    bounty.X = pick.X;
                    bounty.Y = pick.Y;
                    Map.AddActor(bounty);
                    Map.PlayerMass.Add(bounty);
                    Overfill = 0;
                    //Map.UpdatePlayerFieldOfView();
                }
                return true;
            }
            return false;
        }

        /// <summary>
        /// A new instance of the product this <see cref="NPC"/> produces.
        /// </summary>
        public abstract Actor DigestsTo { get; }

        /// <summary>
        /// A new instance of the actor to be instantiated when defeated.
        /// </summary>
        public abstract Actor RescuesTo { get; }

        /// <summary>
        /// <see cref="Actor.BecomeActor(Actor)"/> a <see cref="RescuesTo"/>.
        /// </summary>
        public override void OnUnslime() => BecomeActor(RescuesTo);

        /// <summary>
        /// <see cref="Actor.BecomeActor(Actor)"/> a <see cref="RescuesTo"/>.
        /// </summary>
        public override void OnDestroy() => BecomeActor(RescuesTo);

        /// <summary>
        /// A description of the fundamental mechanics of <see cref="DissolvingNPC"/>.
        /// </summary>
        public virtual string DissolvingAddendum()
        {
            if (!Map.AdjacentActors(X, Y).Where(a => a is Cultivator).Any())
            {
                return $"After {HP} turns, it will become {NameOfResult}. Be careful, it can still be rescued!";
            }
            else
            {
                return $"Kept alive against its will. It is regenerating up to {MaxHP} HP. After {MaxHP - Overfill} turns," +
                    $" it will produce {NameOfResult}. Be careful, it can still be rescued!";
            }
        }

        /// <summary>
        /// Describes the mechanics behind this type of <see cref="Organelle"/>.
        /// </summary>
        public override string DescBody => DissolvingAddendum();

        /// <summary>Stores <see cref="NameOfResult"/> on first query so that <see cref="DigestsTo"/> doesn't have to be queried repeatedly.</summary>
        private string CachedResultName = null;

        /// <summary><see cref="DigestsTo"/>'s <see cref="Actor.Name"/>.</summary>
        public virtual string NameOfResult
        {
            get
            {
                // Null coalescing not available until C# 8:
                // CachedResultName ??= DigestsTo.Name;
                if (CachedResultName == null) // formerly !=, causing the bug
                    CachedResultName = DigestsTo.Name;
                return CachedResultName;
            }
        }
    }
}
