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
    /// <summary>
    /// A repository of all of the <see cref="TextTile"/>s which correspond to defined entities in the game.
    /// </summary>
    /// <remarks>
    /// Eventually, this shouldn't be hardcoded, but rather visible in the game's "asset" or similar folder;
    /// but since the <see cref="ASCIIGraphics"/> is not intended to be permanent anyway (hence its abstraction
    /// into <see cref="GraphicalSystem"/>), this is fine for now.
    /// 
    /// This is the same issue that the class <see cref="Palette"/> has, so the two problems should be adressed at (roughly) the same time.
    /// </remarks>
    public static class TextTilePalette
    {
        public static TextTile Represent(Entity e)
        {
            string representing = e.GetType().Name;
            switch (representing)
            {
                /* Items */
                case nameof(Nutrient):
                    return new TextTile(e, '%', Palette.Slime, Palette.FloorBackgroundFov, VisibilityCondition.LOS_ONLY);
                case nameof(CalciumDust):
                    return new TextTile(e, '%', Palette.Calcium, Palette.FloorBackgroundFov, VisibilityCondition.LOS_ONLY);
                case nameof(SiliconDust):
                    return new TextTile(e, '%', Palette.Electronics, Palette.FloorBackgroundFov, VisibilityCondition.LOS_ONLY);
                case nameof(BarbedWire):
                    return new TextTile(e, 'b', Palette.OrganelleInactive, Palette.FloorBackgroundFov, VisibilityCondition.LOS_ONLY);
                case nameof(Plant):
                    return new TextTile(e, 'l', Palette.OrganelleInactive, Palette.FloorBackgroundFov, VisibilityCondition.LOS_ONLY);
                case nameof(DNA):
                    return new TextTile(e, 'X', Palette.OrganelleInactive, Palette.FloorBackgroundFov, VisibilityCondition.LOS_ONLY);

                /* Actors */
                /* Organelles */
                case nameof(Cytoplasm):
                    return new ActorTextTile(e, ' ', Palette.Slime, VisibilityCondition.LOS_ONLY);
                case nameof(Electronics):
                    return new ActorTextTile(e, '$', Palette.Electronics, VisibilityCondition.LOS_ONLY);
                case nameof(Calcium):
                    return new ActorTextTile(e, '$', Palette.Calcium, VisibilityCondition.LOS_ONLY);
                // Nuclei
                case nameof(Nucleus):
                    return new NucleusTextTile(e as Nucleus, '@', Palette.RootOrganelle, Palette.PlayerInactive, VisibilityCondition.LOS_ONLY);
                case nameof(EyeCore):
                    return new NucleusTextTile(e as Nucleus, '@', Palette.Calcium, Palette.RestingTank, VisibilityCondition.LOS_ONLY);
                case nameof(SmartCore):
                    return new NucleusTextTile(e as Nucleus, '@', Palette.Electronics, Palette.RestingTank, VisibilityCondition.LOS_ONLY);
                case nameof(LaserCore):
                    return new NucleusTextTile(e as Nucleus, '@', Palette.SuperBright, Palette.RestingTank, VisibilityCondition.LOS_ONLY);
                case nameof(TerrorCore):
                    return new NucleusTextTile(e as Nucleus, '@', Palette.OrganelleInactive, Palette.PlayerInactive, VisibilityCondition.LOS_ONLY);
                case nameof(GravityCore):
                    return new NucleusTextTile(e as Nucleus, '@', Palette.DarkSlime, Palette.PlayerInactive, VisibilityCondition.LOS_ONLY);
                case nameof(QuantumCore):
                    return new NucleusTextTile(e as Nucleus, '@', Palette.Cursor, Palette.PlayerInactive, VisibilityCondition.LOS_ONLY);
                // Membranes:
                case nameof(Membrane):
                    return new ActorTextTile(e, 'B', Palette.RootOrganelle, VisibilityCondition.LOS_ONLY);
                case nameof(ReinforcedMembrane):
                    return new ActorTextTile(e, 'B', Palette.Calcium, VisibilityCondition.LOS_ONLY);
                case nameof(Maw):
                    return new ActorTextTile(e, 'W', Palette.Electronics, VisibilityCondition.LOS_ONLY);
                case nameof(ForceField):
                    return new ActorTextTile(e, 'F', Palette.Calcium, VisibilityCondition.LOS_ONLY);
                case nameof(NonNewtonianMembrane):
                    return new ActorTextTile(e, 'P', Palette.Electronics, VisibilityCondition.LOS_ONLY);
                case nameof(ReinforcedMaw):
                    return new ActorTextTile(e, 'W', Palette.Calcium, VisibilityCondition.LOS_ONLY);
                case nameof(Tentacle):
                    return new ActorTextTile(e, 'T', Palette.Electronics, VisibilityCondition.LOS_ONLY);

                // Chloroplasts:
                case nameof(Chloroplast):
                    return new ActorTextTile(e, 'H', Palette.RootOrganelle, VisibilityCondition.LOS_ONLY);
                case nameof(Bioreactor):
                    return new ActorTextTile(e, 'R', Palette.Calcium, VisibilityCondition.LOS_ONLY);
                case nameof(Cultivator):
                    return new ActorTextTile(e, 'U', Palette.Electronics, VisibilityCondition.LOS_ONLY);
                case nameof(BiometalForge):
                    return new ActorTextTile(e, 'G', Palette.Calcium, VisibilityCondition.LOS_ONLY);
                case nameof(PrimordialSoup):
                    return new ActorTextTile(e, 'S', Palette.Electronics, VisibilityCondition.LOS_ONLY);
                case nameof(Extractor):
                    return new ActorTextTile(e, 'V', Palette.Calcium, VisibilityCondition.LOS_ONLY);
                case nameof(Butcher):
                    return new ActorTextTile(e, 'K', Palette.Electronics, VisibilityCondition.LOS_ONLY);

                /* NPCs */
                case nameof(Militia):
                    return new ActorTextTile(e, 'm', Palette.Militia, VisibilityCondition.LOS_ONLY);
                case nameof(Caravan):
                    return new SlowTextTile(e as Caravan, 'v', Palette.Militia, Palette.RestingMilitia, VisibilityCondition.LOS_ONLY);
                case nameof(Tank):
                    return new SlowTextTile(e as Tank, 't', Palette.Calcium, Palette.RestingTank, VisibilityCondition.LOS_ONLY);
                case nameof(Scout):
                    return new RangedTextTile(e as Scout, 's', Palette.Electronics, VisibilityCondition.LOS_ONLY);
                case nameof(Mech):
                    return new SlowTextTile(e as Mech, 'c', Palette.Calcium, Palette.RestingTank, VisibilityCondition.LOS_ONLY);
                case nameof(Hunter):
                    return new RangedTextTile(e as Hunter, 'h', Palette.Electronics, VisibilityCondition.LOS_ONLY);
                // Remains:
                case nameof(Militia.CapturedMilitia):
                    return new ActorTextTile(e as Militia.CapturedMilitia, 'm', Palette.Militia, VisibilityCondition.LOS_ONLY);
                case nameof(Caravan.CapturedCaravan):
                    return new ActorTextTile(e as Caravan.CapturedCaravan, 'v', Palette.Militia, VisibilityCondition.LOS_ONLY);
                case nameof(Tank.CapturedTank):
                    return new ActorTextTile(e as Tank.CapturedTank, 't', Palette.Calcium, VisibilityCondition.LOS_ONLY);
                case nameof(Scout.CapturedScout):
                    return new ActorTextTile(e as Scout.CapturedScout, 's', Palette.Electronics, VisibilityCondition.LOS_ONLY);
                case nameof(Mech.CapturedMech):
                    return new ActorTextTile(e as Mech.CapturedMech, 'c', Palette.Calcium, VisibilityCondition.LOS_ONLY);
                case nameof(Hunter.CapturedHunter):
                    return new ActorTextTile(e as Hunter.CapturedHunter, 'h', Palette.Electronics, VisibilityCondition.LOS_ONLY);

                /* Other */
                case nameof(City):
                    return new CityTextTile(e as City, 'C', Palette.City, Palette.Electronics, Palette.ReticleForeground, Palette.ReticleBackground, VisibilityCondition.EXPLORED_ONLY);
                case nameof(Reticle):
                    return new ReticleTextTile(e, 'X', Palette.ReticleForeground, Palette.ReticleBackground, VisibilityCondition.EXPLORED_ONLY);
                case nameof(Cursor):
                    return new ReticleTextTile(e, 'X', Palette.Cursor, Palette.DarkCursor, VisibilityCondition.ALWAYS_VISIBLE);
                default:
                    return new TextTile(e, '?', Palette.Cursor, Palette.DarkCursor, VisibilityCondition.LOS_ONLY);
            }
        }
    }
}
