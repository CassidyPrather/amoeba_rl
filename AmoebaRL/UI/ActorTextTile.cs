using AmoebaRL.Core;
using AmoebaRL.Core.Enemies;
using AmoebaRL.Core.Organelles;
using AmoebaRL.Interfaces;
using RLNET;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.UI
{
    public class ActorTextTile : TextTile
    {
        /// <summary>
        /// <see cref="TextTile.Represents"/> as an <see cref="Actor"/>.
        /// </summary>
        public virtual Actor ActorRepresents => Represents as Actor;

        /// <summary>
        /// The <see cref="Entity"/> this is an <see cref="IGraphic"/> for.
        /// Can only be <see cref="Actor"/>.
        /// </summary>
        public override Entity Represents
        {
            get => base.Represents;
            set
            {
                if (value is Actor a)
                    base.Represents = a;
                else
                    throw new InvalidCastException($"{nameof(ActorTextTile)} can only represent {nameof(Actor)}s");
            }
        }

        /// <inheritdoc/>
        public ActorTextTile(Entity represents) : base(represents)
        {
            DetermineBackup(represents);
        }

        /// <summary>
        /// Generate a <see cref="TextTile"/> to show <see cref="Represents"/>.
        /// </summary>
        /// <param name="e">The <see cref="Entity"/> this <see cref="TextTile"/> <see cref="Represents"/>.</param>
        /// <param name="symbol">The glyph which shows <paramref name="represents"/>.</param>
        /// <param name="color">The foreground color which shows <paramref name="represents"/>.</param>
        /// <param name="visibility">The condition under which <paramref name="represents"/> should be drawn. When this is not met, no drawing action occurs.</param>
        public ActorTextTile(Entity e, char symbol, RLColor color, VisibilityCondition visibility) : base(e)
        {
            Symbol = symbol;
            Color = color;
            Visibility = visibility;
            DetermineBackup(e);
        }

        /// <summary>
        /// Sets <see cref="Backup"/> to whatever <see cref="Represents"/> is covering. May be <c>null</c>.
        /// </summary>
        private void DetermineBackup(Entity source)
        {
            Entity under = source.Map.GetItemAt(source.X, source.Y);
            if(under != null)
                Backup = TextTilePalette.Represent(under);
            else
                Backup = null;
        }

        /// <summary>
        /// Determines the appropriate <see cref="TextTile.BackgroundColor"/>
        /// based on the state of the <see cref="ActorRepresents"/> <see cref="Actor"/>.
        /// </summary>
        public override RLColor BackgroundColor
        {
            get
            {
                switch(ActorRepresents.Slime)
                {
                    case 0:
                        return Palette.FloorBackgroundFov;
                    case 1:
                        return Palette.BodySlime;
                    case 2:
                        return Palette.PathSlime;
                }
                return Palette.FloorBackground;
            }
        }

        // TODO override draw to show the item underneath if invisible
    }

    public class NucleusTextTile : ActorTextTile
    {
        /// <summary>
        /// <see cref="ActorTextTile.ActorRepresents"/> as a <see cref="Nucleus"/>.
        /// </summary>
        public Nucleus NucleusRepresents => Represents as Nucleus;

        /// <summary>
        /// The <see cref="Actor"/> this is an <see cref="IGraphic"/> for.
        /// Can only be <see cref="Nucleus"/>.
        /// </summary>
        public override Entity Represents
        {
            get => base.Represents;
            set
            {
                if (value is Nucleus a)
                    base.Represents = a;
                else
                    throw new InvalidCastException($"{nameof(NucleusTextTile)} can only represent {nameof(Nucleus)}s");
            }
        }

        /// <summary>
        /// The color of the glyph when it is under the control of the user.
        /// </summary>
        public virtual RLColor ActiveColor { get; set; } = Palette.Wall;

        /// <summary>
        /// The color of the glyph when it is not under the control of the user.
        /// </summary>
        public virtual RLColor InactiveColor { get; set; } = Palette.Wall;

        /// <inheritdoc/>
        public override RLColor Color
        {
            get
            {
                if (Represents.Map.Context.ActivePlayer == Represents)
                    return ActiveColor;
                else
                    return InactiveColor;
            }
        }

        /// <summary>
        /// Generate a <see cref="TextTile"/> to show <see cref="Represents"/>.
        /// </summary>
        /// <param name="e">The <see cref="Entity"/> this <see cref="TextTile"/> <see cref="Represents"/>.</param>
        /// <param name="symbol">The glyph which shows <paramref name="represents"/>.</param>
        /// <param name="activeColor">The foreground color which shows <paramref name="represents"/> when this <see cref="Actor"/> is active.</param>
        /// <param name="inactiveColor">The foreground color which shows <paramref name="represents"/> when this <see cref="Actor"/> is inactive.</param>
        /// <param name="visibility">The condition under which <paramref name="represents"/> should be drawn. When this is not met, no drawing action occurs.</param>
        public NucleusTextTile(Nucleus e, char symbol, RLColor activeColor, RLColor inactiveColor, VisibilityCondition visibility) : base(e)
        {
            Symbol = symbol;
            ActiveColor = activeColor;
            InactiveColor = inactiveColor;
            Visibility = visibility;
        }
    }

    /// <summary>
    /// A <see cref="TextTile"/> for something which acts so infrequently that the user should be graphically notified 
    /// when it is soon (i.e. <see cref="ActiveThreshhold"/> to act.
    /// </summary>
    public class SlowTextTile : ActorTextTile
    {
        /// <summary>
        /// The remaining number of time units until this is activated in <see cref="Systems.SchedulingSystem"/> under or equalling which this should be considered <see cref="ActiveColor"/>.
        /// </summary>
        public virtual float ActiveThreshhold { get; set; } = 16;

        /// <summary>
        /// The color of the glyph when it is under the control of the user.
        /// </summary>
        public virtual RLColor ActiveColor { get; set; } = Palette.Wall;

        /// <summary>
        /// The color of the glyph when it is not under the control of the user.
        /// </summary>
        public virtual RLColor InactiveColor { get; set; } = Palette.Wall;

        /// <inheritdoc/>
        public override RLColor Color
        {
            get
            {
                int? goesAt = Represents.Map.Context.SchedulingSystem.ScheduledFor(ActorRepresents);
                int now = Represents.Map.Context.SchedulingSystem.GetTime();
                if (goesAt.HasValue && goesAt - now <= ActiveThreshhold)
                    return ActiveColor;
                else
                    return InactiveColor;
            }
        }

        /// <summary>
        /// Generate a <see cref="TextTile"/> to show <see cref="Represents"/>.
        /// </summary>
        /// <param name="e">The <see cref="Entity"/> this <see cref="TextTile"/> <see cref="Represents"/>.</param>
        /// <param name="symbol">The glyph which shows <paramref name="represents"/>.</param>
        /// <param name="activeColor">The foreground color which shows <paramref name="represents"/> when this <see cref="Actor"/> is active.</param>
        /// <param name="inactiveColor">The foreground color which shows <paramref name="represents"/> when this <see cref="Actor"/> is inactive.</param>
        /// <param name="visibility">The condition under which <paramref name="represents"/> should be drawn. When this is not met, no drawing action occurs.</param>
        public SlowTextTile(Actor e, char symbol, RLColor activeColor, RLColor inactiveColor, VisibilityCondition visibility) : base(e)
        {
            Symbol = symbol;
            ActiveColor = activeColor;
            InactiveColor = inactiveColor;
            Visibility = visibility;
        }
    }

    public class RangedTextTile : ActorTextTile
    {

        /// <summary>
        /// <see cref="ActorTextTile.ActorRepresents"/> as a <see cref="Hunter"/>.
        /// </summary>
        public Hunter RangedRepresents => Represents as Hunter;

        /// <summary>
        /// The <see cref="Actor"/> this is an <see cref="IGraphic"/> for.
        /// Can only be <see cref="Hunter"/>.
        /// </summary>
        public override Entity Represents
        {
            get => base.Represents;
            set
            {
                if (value is Hunter a)
                    base.Represents = a;
                else
                    throw new InvalidCastException($"{nameof(ActorTextTile)} can only represent {nameof(Actor)}s");
            }
        }

        public char BaseSymbol { get; set; }

        protected bool FiringBlink { get; set; } = false;

        public override char Symbol
        { 
            get
            {
                if (RangedRepresents.Firing < RangedRepresents.FiringTime && FiringBlink)
                {
                    if (RangedRepresents.FiringDirection.X > 0)
                        return (char)16;
                    else if (RangedRepresents.FiringDirection.X < 0)
                        return (char)17;
                    else if (RangedRepresents.FiringDirection.Y > 0)
                        return (char)31;
                    else if (RangedRepresents.FiringDirection.Y < 0)
                        return (char)30;
                    return '?'; // Should never happen.
                }
                else
                    return BaseSymbol;
            }
            set => BaseSymbol = value; 
        }

        public override void SetFrame(int idx)
        {
            FiringBlink = idx == 0;
        }

        /// <summary>
        /// Generate a <see cref="TextTile"/> to show <see cref="Represents"/>.
        /// </summary>
        /// <param name="e">The <see cref="Entity"/> this <see cref="TextTile"/> <see cref="Represents"/>.</param>
        /// <param name="symbol">The glyph which shows <paramref name="represents"/>.</param>
        /// <param name="color">The foreground color which shows <paramref name="represents"/>.</param>
        /// <param name="visibility">The condition under which <paramref name="represents"/> should be drawn. When this is not met, no drawing action occurs.</param>
        public RangedTextTile(Entity e, char symbol, RLColor color, VisibilityCondition visibility) : base(e)
        {
            BaseSymbol = symbol;
            Color = color;
            Visibility = visibility;
            // Animation:
            Frames = 2;
            Speed = 3;
        }
    }

    public class CityTextTile : ActorTextTile
    {
        /// <summary>
        /// <see cref="ActorTextTile.ActorRepresents"/> as a <see cref="City"/>.
        /// </summary>
        public City CityRepresents => Represents as City;

        /// <summary>
        /// The <see cref="Actor"/> this is an <see cref="IGraphic"/> for.
        /// Can only be <see cref="Hunter"/>.
        /// </summary>
        public override Entity Represents
        {
            get => base.Represents;
            set
            {
                if (value is City a)
                    base.Represents = a;
                else
                    throw new InvalidCastException($"{nameof(CityTextTile)} can only represent {nameof(City)}s");
            }
        }

        public RLColor TimerPrimary { get; set; }

        public RLColor TimerSecondary { get; set; }

        public RLColor TimerTetriary { get; set; }

        public bool ShowCounter { get; protected set; } = false;

        public override char Symbol
        { 
            get
            {
                if (!ShowCounter)
                    return base.Symbol;
                if (CityRepresents.SpawnQueue.Count > 0)
                {
                    if (CityRepresents.SpawnQueue.Count > 9)
                        return '*';
                    else
                        return $"{CityRepresents.SpawnQueue.Count}"[0];
                }
                else if (CityRepresents.TurnsToNextWave < 10)
                    return $"{CityRepresents.TurnsToNextWave}"[0];
                else
                    return base.Symbol;
            }
            set => base.Symbol = value;
        }

        public override RLColor Color
        {
            get
            {
                if (!ShowCounter)
                    return base.Color;
                if (CityRepresents.SpawnQueue.Count > 0)
                    return TimerPrimary;
                else if (CityRepresents.TurnsToNextWave < 10)
                    return TimerPrimary;
                else
                    return base.Color;
            }
            set => base.Color = value;
        }

        public override RLColor BackgroundColor
        {
            get
            {
                if (!ShowCounter)
                    return base.BackgroundColor;
                if (CityRepresents.SpawnQueue.Count > 0)
                    return TimerSecondary;
                else if (CityRepresents.TurnsToNextWave < 10)
                    return TimerTetriary;
                else
                    return base.Color;
            }
            set => base.Color = value;
        }

        public CityTextTile(Entity e, char symbol, RLColor color, RLColor timerPrimary, RLColor timerSecondary, RLColor timerTetriary, VisibilityCondition visibility) : base(e)
        {
            base.Symbol = symbol;
            base.Color = color;
            TimerPrimary = timerPrimary;
            TimerSecondary = timerSecondary;
            TimerTetriary = timerTetriary;
            Visibility = visibility;

            // Animation
            Speed = 3;
            Frames = 2;
        }

        public override void SetFrame(int idx)
        {
            ShowCounter = idx != 0 && (CityRepresents.SpawnQueue.Count > 0 || CityRepresents.TurnsToNextWave < 10);
        }
    }
}
