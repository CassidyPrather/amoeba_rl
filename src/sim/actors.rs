//! Every kind of thing that can stand on a cell, and the tables that say what
//! it is.
//!
//! The original was a deep class hierarchy and most of its rules were `is`
//! checks against it — `monster is Tank` was true for Mechs and Caravans,
//! `victim is ReinforcedMembrane` was true for Force Fields and Phase
//! Membranes. A flat enum loses that, so the subtype relationships are spelled
//! out as the predicate functions at the bottom of this file, each one naming
//! the C# type it stands in for. Rewrite an `is` check against those, never
//! against a bare `==`.
//!
//! Storage is a slot arena. Slots are never reused: an [`ActorId`] handed out
//! during a run stays valid or stays dead, and can never silently come back as
//! a different actor. A run leaks a few tens of kilobytes of tombstones, which
//! is cheaper than a generation counter on every handle.

use std::collections::VecDeque;
use std::ops::{Index, IndexMut};

use super::grid::Coord;

/// An 8-bit-per-channel colour, the way the original's palette stored them.
pub type Rgb = [u8; 3];

/// The exact palette the original drew with.
///
/// The derived entries were computed in normalised float space and are given
/// here already resolved: `BODY_SLIME` is `PATH_SLIME * 0.75` rounded, and
/// `CALCIUM` is deep water scaled per channel. Four entries of the original
/// palette were never referenced by anything and are not repeated here.
pub mod palette {
    use super::Rgb;

    /// Background of an explored but unlit floor cell.
    pub const FLOOR_BACKGROUND: Rgb = [0, 0, 0];
    /// Foreground of an explored but unlit floor cell.
    pub const FLOOR: Rgb = [71, 62, 45];
    /// Background of a lit floor cell, and of anything with no slime on it.
    pub const FLOOR_BACKGROUND_FOV: Rgb = [20, 12, 28];
    /// Foreground of a lit floor cell.
    pub const FLOOR_FOV: Rgb = [129, 121, 107];
    /// Background of an explored but unlit wall.
    pub const WALL_BACKGROUND: Rgb = [31, 38, 47];
    /// Foreground of an explored but unlit wall.
    pub const WALL: Rgb = [72, 77, 85];
    /// Background of a lit wall.
    pub const WALL_BACKGROUND_FOV: Rgb = [51, 56, 64];
    /// Foreground of a lit wall.
    pub const WALL_FOV: Rgb = [93, 97, 105];
    /// Headings and other bright interface text.
    pub const TEXT_HEADING: Rgb = [222, 238, 214];
    /// Ordinary interface text.
    pub const TEXT_BODY: Rgb = [210, 125, 44];
    /// The brightest thing on screen.
    pub const SUPER_BRIGHT: Rgb = [222, 238, 214];
    /// A nucleus that is not the one under your control.
    pub const PLAYER_INACTIVE: Rgb = [88, 56, 72];
    /// Cytoplasm and everything grown from a nutrient.
    pub const SLIME: Rgb = [109, 170, 44];
    /// The gravity core, which needs to read as heavy.
    pub const DARK_SLIME: Rgb = [52, 101, 36];
    /// Background of an organelle that will move when you do.
    pub const PATH_SLIME: Rgb = [132, 190, 56];
    /// Background of an organelle that is merely part of you.
    pub const BODY_SLIME: Rgb = [99, 143, 42];
    /// A city gate you can see.
    pub const CITY: Rgb = [133, 149, 161];
    /// A human that is about to act.
    pub const MILITIA: Rgb = [210, 125, 44];
    /// A human that is still recovering from acting.
    pub const RESTING_MILITIA: Rgb = [133, 76, 48];
    /// Bone, and everything built out of it.
    pub const CALCIUM: Rgb = [107, 113, 247];
    /// An armoured human that is still recovering from acting.
    pub const RESTING_TANK: Rgb = [48, 52, 109];
    /// Circuitry, and everything built out of it.
    pub const ELECTRONICS: Rgb = [218, 212, 94];
    /// A hunter's targeting mark.
    pub const RETICLE_FOREGROUND: Rgb = [208, 70, 72];
    /// Behind a hunter's targeting mark.
    pub const RETICLE_BACKGROUND: Rgb = [68, 36, 52];
    /// The first thing of each organelle line, before any upgrade.
    pub const ROOT_ORGANELLE: Rgb = [208, 70, 72];
    /// Raw human-made materials lying on the floor.
    pub const ORGANELLE_INACTIVE: Rgb = [133, 149, 161];
    /// The examine cursor.
    pub const CURSOR: Rgb = [255, 127, 255];
    /// Behind the examine cursor.
    pub const DARK_CURSOR: Rgb = [255, 0, 255];
    /// A digestion bar that has banked a free product.
    pub const OVERFILL: Rgb = [109, 194, 202];
    /// Behind the organelle sidebar.
    pub const ORGANELLE_CONSOLE_BG: Rgb = [6, 26, 0];
    /// A remembered city gate, seen once and now out of sight.
    pub const DB_STONE: Rgb = [117, 113, 97];
}

/// The two things organelles can be upgraded with.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Resource {
    /// Worked bone. Reinforces.
    Calcium,
    /// Stolen circuitry. Accelerates.
    Electronics,
}

impl Resource {
    /// The display name, as the upgrade hint lines print it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Calcium => "Calcium",
            Self::Electronics => "Electronics",
        }
    }

    /// The colour the sidebar tints this resource's progress bar.
    #[must_use]
    pub const fn color(self) -> Rgb {
        match self {
            Self::Calcium => palette::CALCIUM,
            Self::Electronics => palette::ELECTRONICS,
        }
    }
}

/// One way an organelle can become a different organelle.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct UpgradePath {
    /// How many of the resource it takes.
    pub amount: i32,
    /// Which resource.
    pub resource: Resource,
    /// What it becomes.
    pub result: Kind,
}

/// An item lying on the floor. Every one of them is edible.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ItemKind {
    /// Food. Eating one grows the mass by a cell.
    Nutrient,
    /// Bone dust. Becomes a Calcium organelle.
    CalciumDust,
    /// Circuit dust. Becomes an Electronics organelle.
    SiliconDust,
    /// Human fortification. Becomes a Membrane.
    BarbedWire,
    /// Greenery. Becomes a Chloroplast.
    Plant,
    /// Becomes a Nucleus.
    Dna,
}

impl ItemKind {
    /// Display name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Nutrient => "Nutrient",
            Self::CalciumDust => "Calcium Dust",
            Self::SiliconDust => "Silicon Dust",
            Self::BarbedWire => "Barbed Wire",
            Self::Plant => "Plant",
            Self::Dna => "DNA",
        }
    }

    /// Flavour text shown when examined.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Nutrient => "This precious meal is the foundation of growth.",
            Self::CalciumDust => "This precious powder could be used to build strong bones.",
            Self::SiliconDust => {
                "These rocks contain the magic of humanity, and could be used to accelerate evolution."
            }
            Self::BarbedWire => {
                "Humans set these up to protect their cities. You can probably put it to better use."
            }
            Self::Plant => {
                "A cute green plant. Its ability to use the sun to produce food is fascinating and could be exploited."
            }
            Self::Dna => {
                "Short for Deoxyribonucleic Acid. It would be possible to fasion a new nucleus out of this."
            }
        }
    }

    /// Map glyph.
    #[must_use]
    pub const fn glyph(self) -> char {
        match self {
            Self::Nutrient | Self::CalciumDust | Self::SiliconDust => '%',
            Self::BarbedWire => '*',
            Self::Plant => '?',
            Self::Dna => '&',
        }
    }

    /// Map colour.
    #[must_use]
    pub const fn color(self) -> Rgb {
        match self {
            Self::Nutrient => palette::SLIME,
            Self::CalciumDust => palette::CALCIUM,
            Self::SiliconDust => palette::ELECTRONICS,
            Self::BarbedWire | Self::Plant | Self::Dna => palette::ORGANELLE_INACTIVE,
        }
    }

    /// What eating this turns it into.
    #[must_use]
    pub const fn new_organelle(self) -> Kind {
        match self {
            Self::Nutrient => Kind::Cytoplasm,
            Self::CalciumDust => Kind::Calcium,
            Self::SiliconDust => Kind::Electronics,
            Self::BarbedWire => Kind::Membrane,
            Self::Plant => Kind::Chloroplast,
            Self::Dna => Kind::Nucleus,
        }
    }
}

/// Every kind of actor in the game.
///
/// Ordering here follows the spec's sections: nuclei, cytoplasm and crafting
/// materials, the membrane line, the chloroplast line, live humans, and the
/// dissolving forms they become once eaten.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Kind {
    /// The organelle you steer. Only one may act per turn.
    Nucleus,
    /// A nucleus that sees much further.
    EyeCore,
    /// A nucleus that slips through its own body cheaply.
    SmartCore,
    /// A nucleus that cuts through armour.
    LaserCore,
    /// A nucleus that costs adjacent humans their turn.
    TerrorCore,
    /// A nucleus that drags nearby organelles along.
    GravityCore,
    /// A nucleus that acts twice per turn.
    QuantumCore,
    /// Undifferentiated mass, and the host cell every catalyst needs.
    Cytoplasm,
    /// A crafting material that upgrades a neighbour with bone.
    Calcium,
    /// A crafting material that upgrades a neighbour with circuitry.
    Electronics,
    /// Kills unarmoured attackers.
    Membrane,
    /// Kills armoured attackers too.
    ReinforcedMembrane,
    /// A membrane that hunts food on its own.
    Maw,
    /// Protects every ally near it.
    ForceField,
    /// Steps in front of a doomed ally and hardens.
    PhaseMembrane,
    /// A maw that bites through armour.
    ReinforcedMaw,
    /// A maw that acts eight times per turn.
    Tentacle,
    /// Produces cytoplasm on a timer.
    Chloroplast,
    /// A chloroplast that starts producing sooner.
    Bioreactor,
    /// Heals adjacent dissolving humans and overfills them.
    Cultivator,
    /// Produces crafting materials out of nothing.
    BiometalForge,
    /// Produces whole organelles out of nothing.
    PrimordialSoup,
    /// A cultivator that hauls dissolving humans toward itself.
    Extractor,
    /// Every dissolving human that finishes yields one extra product.
    Butcher,
    /// The basic human.
    Militia,
    /// A human that fires down a long line.
    Hunter,
    /// A human that fires down a short line and sees further.
    Scout,
    /// An armoured, very slow human.
    Tank,
    /// An armoured human that keeps up.
    Mech,
    /// An armoured human that runs away and drops good loot.
    Caravan,
    /// A gate. Destroying enough of them wins the game.
    City,
    /// A militia coming apart inside the mass.
    DissolvingMilitia,
    /// A hunter coming apart inside the mass.
    DissolvingHunter,
    /// A scout coming apart inside the mass.
    DissolvingScout,
    /// A tank coming apart inside the mass.
    DissolvingTank,
    /// A mech coming apart inside the mass.
    DissolvingMech,
    /// A caravan coming apart inside the mass.
    DissolvingCaravan,
}

/// The fixed stat block for a [`Kind`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Stats {
    /// Display name.
    pub name: &'static str,
    /// Flavour text shown when examined or browsed.
    pub description: &'static str,
    /// Map glyph.
    pub glyph: char,
    /// Map colour when active, or the only colour if there is one.
    pub fg: Rgb,
    /// Map colour when resting: an inactive nucleus, or an armoured human that
    /// will not act within a turn.
    pub fg_resting: Option<Rgb>,
    /// Taxicab sight radius. Also the field-of-view radius it contributes.
    pub awareness: i32,
    /// Time units between this thing's actions.
    pub delay: i32,
    /// Background index: 0 none, 1 body, 2 on the drag path.
    pub slime: u8,
    /// Nonzero means walking into it does nothing without a laser core or a
    /// reinforced maw.
    pub armor: i32,
    /// Digestion length for a dissolving form; zero for everything else.
    pub max_hp: i32,
}

impl Kind {
    /// This kind's fixed stats.
    #[allow(clippy::too_many_lines)] // One arm per entity kind; a table is the point.
    #[must_use]
    pub const fn stats(self) -> Stats {
        const fn s(
            name: &'static str,
            description: &'static str,
            glyph: char,
            fg: Rgb,
            awareness: i32,
            delay: i32,
            slime: u8,
        ) -> Stats {
            Stats {
                name,
                description,
                glyph,
                fg,
                fg_resting: None,
                awareness,
                delay,
                slime,
                armor: 0,
                max_hp: 0,
            }
        }
        const fn resting(mut stats: Stats, fg_resting: Rgb) -> Stats {
            stats.fg_resting = Some(fg_resting);
            stats
        }
        const fn armored(mut stats: Stats) -> Stats {
            stats.armor = 1;
            stats
        }
        const fn digesting(mut stats: Stats, max_hp: i32) -> Stats {
            stats.max_hp = max_hp;
            stats
        }
        match self {
            Self::Nucleus => resting(
                s(
                    "Nucleus",
                    "The seat of your will. Only one nucleus may act each turn; the rest are dragged along behind it.",
                    '@',
                    palette::ROOT_ORGANELLE,
                    3,
                    16,
                    1,
                ),
                palette::PLAYER_INACTIVE,
            ),
            // DELIBERATE CHANGE from C# (§5.2): Awareness was 6, exactly twice a
            // plain nucleus. The eye line was the weaker half of the tree, so it
            // gets one more cell of sight, and the two cores grown from it keep
            // it.
            Self::EyeCore => resting(
                s(
                    "Eye Core",
                    "A nucleus grown around a lens of bone. It sees more than twice as far into the dark.",
                    '@',
                    palette::CALCIUM,
                    7,
                    16,
                    1,
                ),
                palette::RESTING_TANK,
            ),
            // DELIBERATE CHANGE from C# (§5.2): Delay was 8, which made the
            // smart core faster at everything and left the quantum core with
            // nothing to upgrade but a swap discount stacked on top. The haste
            // now only applies to slipping through your own mass; the quantum
            // core is what turns it into speed everywhere. See `attack_move`.
            Self::SmartCore => resting(
                s(
                    "Smart Core",
                    "A nucleus threaded with stolen circuitry. Slipping through your own body costs it half the usual effort.",
                    '@',
                    palette::ELECTRONICS,
                    3,
                    16,
                    1,
                ),
                palette::RESTING_TANK,
            ),
            Self::LaserCore => resting(
                s(
                    "Laser Core",
                    "A nucleus that focuses light into a cutting beam. Armour means nothing to it.",
                    '@',
                    palette::SUPER_BRIGHT,
                    7,
                    16,
                    1,
                ),
                palette::RESTING_TANK,
            ),
            Self::TerrorCore => resting(
                s(
                    "Terror Core",
                    "A nucleus that radiates dread. Humans beside it forget how to take their turn.",
                    '@',
                    palette::ORGANELLE_INACTIVE,
                    7,
                    16,
                    1,
                ),
                palette::PLAYER_INACTIVE,
            ),
            // DELIBERATE CHANGE from C# (§5.2): Delay was 8, inherited from the
            // smart core. It inherits the smart core's swap discount instead.
            Self::GravityCore => resting(
                s(
                    "Gravity Core",
                    "A nucleus dense enough to bend the mass around it. Where it goes, nearby organelles follow.",
                    '@',
                    palette::DARK_SLIME,
                    3,
                    16,
                    1,
                ),
                palette::PLAYER_INACTIVE,
            ),
            // DELIBERATE CHANGE from C# (§5.2): the original stacked a half-cost
            // swap on top of Delay 8. The swap discount is the smart core's now,
            // and this is the upgrade that drops the restriction: two actions a
            // turn, whatever they are.
            Self::QuantumCore => resting(
                s(
                    "Quantum Core",
                    "A nucleus that is never quite where it was. It takes two steps in the time anything else takes one.",
                    '@',
                    palette::CURSOR,
                    3,
                    8,
                    1,
                ),
                palette::PLAYER_INACTIVE,
            ),
            Self::Cytoplasm => s(
                "Cytoplasm",
                "Undifferentiated mass. It is what everything else is made of, and what every catalyst needs to grow in.",
                ' ',
                palette::SLIME,
                0,
                16,
                1,
            ),
            Self::Calcium => s(
                "Calcium",
                "A bead of worked bone looking for something to reinforce. Walk a nucleus into it to spend it.",
                '$',
                palette::CALCIUM,
                0,
                // DELIBERATE CHANGE from C#: Delay was 1, so a crafting material
                // ticked sixteen times per turn and dominated the frame budget.
                // One tick per turn, like everything else.
                16,
                1,
            ),
            Self::Electronics => s(
                "Electronics",
                "A knot of stolen circuitry looking for something to accelerate. Walk a nucleus into it to spend it.",
                '$',
                palette::ELECTRONICS,
                0,
                // DELIBERATE CHANGE from C#: see Calcium.
                16,
                1,
            ),
            Self::Membrane => s(
                "Membrane",
                "A wall of barbed protein. Anything unarmoured that strikes it dies for the trouble.",
                'B',
                palette::ROOT_ORGANELLE,
                0,
                16,
                1,
            ),
            Self::ReinforcedMembrane => s(
                "Tough Membrane",
                "Barbed protein grown over a lattice of bone. Even armour splits on it.",
                'B',
                palette::CALCIUM,
                0,
                16,
                1,
            ),
            // DELIBERATE CHANGE from C# (§5.4): Delay was 16. The whole maw line
            // gains one step of speed — the maws crawl twice a turn and the
            // tentacle keeps its fourfold lead over them.
            Self::Maw => s(
                "Maw",
                "A mouth on the outside of your body. It crawls toward food twice a turn without being told.",
                'W',
                palette::ELECTRONICS,
                1,
                8,
                1,
            ),
            Self::ForceField => s(
                "Force Field",
                "A standing field of charged protein. Everything near it is shielded, armour or no armour.",
                'F',
                palette::CALCIUM,
                0,
                16,
                1,
            ),
            Self::PhaseMembrane => s(
                "Phase Membrane",
                "Liquid until struck. It steps in front of a doomed neighbour and hardens around the attacker.",
                'P',
                palette::ELECTRONICS,
                0,
                16,
                1,
            ),
            // DELIBERATE CHANGE from C# (§5.4): Delay was 16. See `Maw`.
            Self::ReinforcedMaw => s(
                "Reinforced Maw",
                "A mouth lined with bone teeth. It bites through armour that would turn anything else away.",
                'W',
                palette::CALCIUM,
                1,
                8,
                1,
            ),
            // DELIBERATE CHANGE from C# (§5.4): Delay was 4. See `Maw`.
            Self::Tentacle => s(
                "Tentacle",
                "A whip of muscle that lashes out eight times in the span others take to move once. It gives tanks a wide berth.",
                'T',
                palette::ELECTRONICS,
                3,
                2,
                1,
            ),
            Self::Chloroplast => s(
                "Chloroplast",
                "Stolen greenery. It turns light into food, slowly and forever.",
                'H',
                palette::ROOT_ORGANELLE,
                0,
                16,
                1,
            ),
            Self::Bioreactor => s(
                "Bioreactor",
                "A chloroplast walled in bone. It comes online sooner and then keeps the same steady pace.",
                'R',
                palette::CALCIUM,
                0,
                16,
                1,
            ),
            Self::Cultivator => s(
                "Cultivator",
                "A chloroplast taught to tend meat. Dissolving humans beside it hold together and give up more.",
                'U',
                palette::ELECTRONICS,
                0,
                16,
                1,
            ),
            Self::BiometalForge => s(
                "Biometal Forge",
                "It grows bone and circuitry out of light alone, one or the other, never both at once.",
                'G',
                palette::CALCIUM,
                0,
                16,
                1,
            ),
            Self::PrimordialSoup => s(
                "Primordial Soup",
                "The oldest chemistry, running again. Whole organelles condense out of it, and sometimes a nucleus.",
                'S',
                palette::ELECTRONICS,
                0,
                16,
                1,
            ),
            Self::Extractor => s(
                "Extractor",
                "A cultivator with reach. It hauls dissolving humans in toward itself so they finish under its care.",
                'V',
                palette::CALCIUM,
                0,
                16,
                1,
            ),
            Self::Butcher => s(
                "Butcher",
                "It wastes nothing. Every human that finishes dissolving anywhere in the mass yields one more product.",
                'K',
                palette::ELECTRONICS,
                0,
                16,
                1,
            ),
            Self::Militia => s(
                "Militia",
                "A human with a stick and a grudge. Alone it is food; in numbers it is a problem.",
                'm',
                palette::MILITIA,
                3,
                16,
                0,
            ),
            Self::Hunter => s(
                "Hunter",
                "A human with a long gun. It paints a line across the cavern and fires down it two turns later.",
                'h',
                palette::ELECTRONICS,
                3,
                16,
                0,
            ),
            Self::Scout => s(
                "Scout",
                "A human with a short gun and sharp eyes. It sees you before the others do.",
                's',
                palette::ELECTRONICS,
                4,
                16,
                0,
            ),
            Self::Tank => armored(resting(
                s(
                    "Tank",
                    "A human in plate. Slow, and proof against anything not sharpened into bone.",
                    't',
                    palette::CALCIUM,
                    3,
                    48,
                    0,
                ),
                palette::RESTING_TANK,
            )),
            Self::Mech => armored(resting(
                s(
                    "Mech",
                    "A walking machine. Armoured like a tank and half again as quick.",
                    'c',
                    palette::CALCIUM,
                    3,
                    32,
                    0,
                ),
                palette::RESTING_TANK,
            )),
            Self::Caravan => armored(resting(
                s(
                    "Caravan",
                    "A supply wagon. It runs from you, and it is worth the chase.",
                    'v',
                    palette::MILITIA,
                    3,
                    32,
                    0,
                ),
                palette::RESTING_MILITIA,
            )),
            Self::City => s(
                "City Gate",
                "A gate in the rock. Humans pour out of it forever, unless enough of you leans on it at once.",
                'C',
                palette::CITY,
                0,
                16,
                0,
            ),
            Self::DissolvingMilitia => digesting(
                s(
                    "Dissolving Militia",
                    "A militiaman coming apart inside you. Left alone it becomes cytoplasm.",
                    'm',
                    palette::MILITIA,
                    0,
                    16,
                    1,
                ),
                8,
            ),
            Self::DissolvingHunter => digesting(
                s(
                    "Dissolving Hunter",
                    "A hunter coming apart inside you. Left alone its gun becomes electronics.",
                    'h',
                    palette::ELECTRONICS,
                    0,
                    16,
                    1,
                ),
                16,
            ),
            Self::DissolvingScout => digesting(
                s(
                    "Dissolving Scout",
                    "A scout coming apart inside you. Left alone its gun becomes electronics.",
                    's',
                    palette::ELECTRONICS,
                    0,
                    16,
                    1,
                ),
                16,
            ),
            Self::DissolvingTank => digesting(
                s(
                    "Dissolving Tank",
                    "A tank coming apart inside you. Left alone its plate becomes calcium.",
                    't',
                    palette::CALCIUM,
                    0,
                    16,
                    1,
                ),
                24,
            ),
            Self::DissolvingMech => digesting(
                s(
                    "Dissolving Mech",
                    "A mech coming apart inside you. Left alone its frame becomes calcium.",
                    'c',
                    palette::CALCIUM,
                    0,
                    16,
                    1,
                ),
                24,
            ),
            Self::DissolvingCaravan => digesting(
                s(
                    "Dissolving Caravan",
                    "A caravan coming apart inside you. Its cargo could be calcium or electronics.",
                    'v',
                    palette::MILITIA,
                    0,
                    16,
                    1,
                ),
                24,
            ),
        }
    }

    /// Display name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.stats().name
    }

    /// What this organelle drops when it dies.
    ///
    /// A melee kill drops only the first entry; a hunter's shot drops the lot.
    /// An in-progress upgrade adds its own materials on top of this list.
    #[must_use]
    pub const fn components(self) -> &'static [ItemKind] {
        use ItemKind::{BarbedWire, CalciumDust, Dna, Nutrient, Plant, SiliconDust};
        match self {
            Self::Nucleus => &[Dna, Nutrient],
            Self::EyeCore => &[Dna, Nutrient, CalciumDust],
            Self::SmartCore => &[Dna, Nutrient, SiliconDust],
            Self::LaserCore => &[Dna, Nutrient, CalciumDust, CalciumDust, CalciumDust],
            Self::TerrorCore => &[Dna, Nutrient, CalciumDust, SiliconDust, SiliconDust],
            Self::GravityCore => &[Dna, Nutrient, SiliconDust, CalciumDust, CalciumDust],
            Self::QuantumCore => &[
                Dna,
                Nutrient,
                SiliconDust,
                SiliconDust,
                SiliconDust,
                SiliconDust,
            ],
            Self::Cytoplasm => &[Nutrient],
            Self::Calcium => &[CalciumDust, Nutrient],
            Self::Electronics => &[SiliconDust, Nutrient],
            Self::Membrane => &[BarbedWire, Nutrient],
            Self::ReinforcedMembrane => &[BarbedWire, Nutrient, CalciumDust],
            Self::Maw => &[BarbedWire, Nutrient, SiliconDust],
            Self::ForceField => &[
                BarbedWire,
                Nutrient,
                CalciumDust,
                CalciumDust,
                CalciumDust,
                CalciumDust,
            ],
            Self::PhaseMembrane => &[BarbedWire, Nutrient, CalciumDust, SiliconDust],
            Self::ReinforcedMaw => &[
                BarbedWire,
                Nutrient,
                SiliconDust,
                CalciumDust,
                CalciumDust,
                CalciumDust,
            ],
            Self::Tentacle => &[
                BarbedWire,
                Nutrient,
                SiliconDust,
                SiliconDust,
                SiliconDust,
                SiliconDust,
            ],
            Self::Chloroplast => &[Plant, Nutrient],
            Self::Bioreactor => &[Plant, Nutrient, CalciumDust],
            Self::Cultivator => &[Plant, Nutrient, SiliconDust],
            Self::BiometalForge => &[
                Plant,
                Nutrient,
                CalciumDust,
                CalciumDust,
                CalciumDust,
                CalciumDust,
            ],
            Self::PrimordialSoup => &[Plant, Nutrient, CalciumDust, SiliconDust, SiliconDust],
            Self::Extractor => &[Plant, Nutrient, SiliconDust, CalciumDust, CalciumDust],
            Self::Butcher => &[Plant, Nutrient, SiliconDust, SiliconDust, SiliconDust],
            _ => &[],
        }
    }

    /// Every upgrade this organelle can still take. Empty means terminal.
    ///
    /// The tables are spelled out as literals rather than built by a helper
    /// because only literal aggregates get promoted to `'static`.
    #[allow(clippy::too_many_lines)] // One arm per upgradable organelle.
    #[must_use]
    pub const fn upgrade_paths(self) -> &'static [UpgradePath] {
        use Resource::{Calcium, Electronics};
        match self {
            Self::Nucleus => &[
                UpgradePath {
                    amount: 1,
                    resource: Calcium,
                    result: Self::EyeCore,
                },
                UpgradePath {
                    amount: 2,
                    resource: Electronics,
                    result: Self::SmartCore,
                },
            ],
            Self::EyeCore => &[
                UpgradePath {
                    amount: 2,
                    resource: Calcium,
                    result: Self::LaserCore,
                },
                UpgradePath {
                    amount: 2,
                    resource: Electronics,
                    result: Self::TerrorCore,
                },
            ],
            Self::SmartCore => &[
                UpgradePath {
                    amount: 2,
                    resource: Calcium,
                    result: Self::GravityCore,
                },
                UpgradePath {
                    amount: 3,
                    resource: Electronics,
                    result: Self::QuantumCore,
                },
            ],
            Self::Membrane => &[
                UpgradePath {
                    amount: 1,
                    resource: Calcium,
                    result: Self::ReinforcedMembrane,
                },
                UpgradePath {
                    amount: 1,
                    resource: Electronics,
                    result: Self::Maw,
                },
            ],
            Self::ReinforcedMembrane => &[
                UpgradePath {
                    amount: 3,
                    resource: Calcium,
                    result: Self::ForceField,
                },
                UpgradePath {
                    amount: 1,
                    resource: Electronics,
                    result: Self::PhaseMembrane,
                },
            ],
            Self::Maw => &[
                UpgradePath {
                    amount: 3,
                    resource: Calcium,
                    result: Self::ReinforcedMaw,
                },
                UpgradePath {
                    amount: 3,
                    resource: Electronics,
                    result: Self::Tentacle,
                },
            ],
            Self::Chloroplast => &[
                UpgradePath {
                    amount: 1,
                    resource: Calcium,
                    result: Self::Bioreactor,
                },
                UpgradePath {
                    amount: 1,
                    resource: Electronics,
                    result: Self::Cultivator,
                },
            ],
            Self::Bioreactor => &[
                UpgradePath {
                    amount: 3,
                    resource: Calcium,
                    result: Self::BiometalForge,
                },
                UpgradePath {
                    amount: 2,
                    resource: Electronics,
                    result: Self::PrimordialSoup,
                },
            ],
            Self::Cultivator => &[
                UpgradePath {
                    amount: 2,
                    resource: Calcium,
                    result: Self::Extractor,
                },
                UpgradePath {
                    amount: 2,
                    resource: Electronics,
                    result: Self::Butcher,
                },
            ],
            _ => &[],
        }
    }

    /// The dissolving form this human becomes when eaten or engulfed.
    #[must_use]
    pub const fn becomes_on_eaten(self) -> Option<Self> {
        match self {
            Self::Militia => Some(Self::DissolvingMilitia),
            Self::Hunter => Some(Self::DissolvingHunter),
            Self::Scout => Some(Self::DissolvingScout),
            Self::Tank => Some(Self::DissolvingTank),
            Self::Mech => Some(Self::DissolvingMech),
            Self::Caravan => Some(Self::DissolvingCaravan),
            _ => None,
        }
    }

    /// What this human's corpse leaves on the floor when killed outright.
    ///
    /// Two entries mean a coin flip: that is the caravan's cargo.
    #[must_use]
    pub const fn drops_on_die(self) -> &'static [ItemKind] {
        match self {
            Self::Militia => &[ItemKind::Nutrient],
            Self::Hunter | Self::Scout => &[ItemKind::SiliconDust],
            Self::Tank | Self::Mech => &[ItemKind::CalciumDust],
            Self::Caravan => &[ItemKind::CalciumDust, ItemKind::SiliconDust],
            _ => &[],
        }
    }

    /// What a finished dissolving form becomes.
    ///
    /// Two entries mean a coin flip: that is the caravan's cargo again.
    #[must_use]
    pub const fn digests_to(self) -> &'static [Self] {
        match self {
            Self::DissolvingMilitia => &[Self::Cytoplasm],
            Self::DissolvingHunter | Self::DissolvingScout => &[Self::Electronics],
            Self::DissolvingTank | Self::DissolvingMech => &[Self::Calcium],
            Self::DissolvingCaravan => &[Self::Calcium, Self::Electronics],
            _ => &[],
        }
    }

    /// The live human a dissolving form reverts to when something attacks it.
    #[must_use]
    pub const fn rescues_to(self) -> Option<Self> {
        match self {
            Self::DissolvingMilitia => Some(Self::Militia),
            Self::DissolvingHunter => Some(Self::Hunter),
            Self::DissolvingScout => Some(Self::Scout),
            Self::DissolvingTank => Some(Self::Tank),
            Self::DissolvingMech => Some(Self::Mech),
            Self::DissolvingCaravan => Some(Self::Caravan),
            _ => None,
        }
    }

    /// Which resource a crafting material hands out.
    #[must_use]
    pub const fn provides(self) -> Option<Resource> {
        match self {
            Self::Calcium => Some(Resource::Calcium),
            Self::Electronics => Some(Resource::Electronics),
            _ => None,
        }
    }

    /// Turns before this producer's first product. The rest of the line
    /// produces on the same sixteen-turn cycle regardless.
    #[must_use]
    pub const fn next_food_init(self) -> Option<i32> {
        match self {
            Self::Chloroplast => Some(20),
            Self::Bioreactor | Self::BiometalForge | Self::PrimordialSoup => Some(16),
            _ => None,
        }
    }

    /// How far a ranged human's shot travels.
    #[must_use]
    pub const fn range(self) -> i32 {
        match self {
            Self::Hunter => 64,
            Self::Scout => 3,
            _ => 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Family predicates.
//
// Each of these stands in for one `is` check against the original's class
// hierarchy. Rewriting a rule means picking the predicate whose comment names
// the C# type the rule tested for.
// ---------------------------------------------------------------------------

/// `x is NPC` — a live human. Cities are actors but not NPCs.
#[must_use]
pub const fn is_npc(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Militia | Kind::Hunter | Kind::Scout | Kind::Tank | Kind::Mech | Kind::Caravan
    )
}

/// `x is Militia` — true for every live human, because `Tank`, `Hunter` and
/// their subclasses all descend from `Militia`.
#[must_use]
pub const fn is_militia_family(kind: Kind) -> bool {
    is_npc(kind)
}

/// `x is Tank` — true for `Tank`, `Mech` and `Caravan`. This is the check
/// behind "armoured", the force field's long-range exception and the
/// tentacle's fear.
#[must_use]
pub const fn is_tank_family(kind: Kind) -> bool {
    matches!(kind, Kind::Tank | Kind::Mech | Kind::Caravan)
}

/// `x is Hunter` — `Hunter` and `Scout`, the two that shoot.
#[must_use]
pub const fn is_hunter_family(kind: Kind) -> bool {
    matches!(kind, Kind::Hunter | Kind::Scout)
}

/// `x is Caravan` — the one that flees.
#[must_use]
pub const fn is_caravan(kind: Kind) -> bool {
    matches!(kind, Kind::Caravan)
}

/// `x is City`.
#[must_use]
pub const fn is_city(kind: Kind) -> bool {
    matches!(kind, Kind::City)
}

/// `x is Organelle` — everything that lives in the player's mass, including
/// crafting materials and dissolving humans.
#[must_use]
pub const fn is_organelle(kind: Kind) -> bool {
    !is_npc(kind) && !is_city(kind)
}

/// `x is Nucleus` — the whole core line, since every core descends from
/// `Nucleus`.
#[must_use]
pub const fn is_nucleus_family(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Nucleus
            | Kind::EyeCore
            | Kind::SmartCore
            | Kind::LaserCore
            | Kind::TerrorCore
            | Kind::GravityCore
            | Kind::QuantumCore
    )
}

/// The cores that slip through their own mass for half the usual cost: the
/// smart core and the gravity core grown from it.
///
/// DELIBERATE CHANGE from C# (§5.2), where this was the quantum core's discount
/// on top of its own doubled speed. The quantum core spends the same haste on
/// everything it does instead, so it is deliberately not in this list.
#[must_use]
pub const fn swaps_for_half_cost(kind: Kind) -> bool {
    matches!(kind, Kind::SmartCore | Kind::GravityCore)
}

/// `x.Name == "Nucleus"` — the literal name comparison the primordial soup's
/// nucleus cap uses, which upgraded cores deliberately slip past.
#[must_use]
pub const fn is_unupgraded_nucleus(kind: Kind) -> bool {
    matches!(kind, Kind::Nucleus)
}

/// `x is CraftingMaterial` — `Calcium` and `Electronics`. Hidden from the
/// organelle sidebar.
#[must_use]
pub const fn is_crafting_material(kind: Kind) -> bool {
    matches!(kind, Kind::Calcium | Kind::Electronics)
}

/// `x is Membrane` — the whole membrane line including maws and tentacles.
/// These kill unarmoured attackers.
#[must_use]
pub const fn is_membrane_family(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Membrane
            | Kind::ReinforcedMembrane
            | Kind::Maw
            | Kind::ForceField
            | Kind::PhaseMembrane
            | Kind::ReinforcedMaw
            | Kind::Tentacle
    )
}

/// `x is ReinforcedMembrane` — the tough membrane and the two things built
/// from it. Note the reinforced maw is *not* one of these: the original spells
/// it out as a separate `|| victim is ReinforcedMaw`.
#[must_use]
pub const fn is_reinforced_membrane_family(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::ReinforcedMembrane | Kind::ForceField | Kind::PhaseMembrane
    )
}

/// `victim is ReinforcedMembrane || victim is ReinforcedMaw` — the membranes
/// that kill an armoured attacker instead of dying to it.
#[must_use]
pub const fn kills_armored_attackers(kind: Kind) -> bool {
    is_reinforced_membrane_family(kind) || matches!(kind, Kind::ReinforcedMaw)
}

/// `x is Maw` — the maw line, which hunts on its own.
#[must_use]
pub const fn is_maw_family(kind: Kind) -> bool {
    matches!(kind, Kind::Maw | Kind::ReinforcedMaw | Kind::Tentacle)
}

/// `x is Chloroplast` — the whole plant line.
#[must_use]
pub const fn is_chloroplast_family(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Chloroplast
            | Kind::Bioreactor
            | Kind::Cultivator
            | Kind::BiometalForge
            | Kind::PrimordialSoup
            | Kind::Extractor
            | Kind::Butcher
    )
}

/// `x is Cultivator` — the branch that tends dissolving humans.
#[must_use]
pub const fn is_cultivator_family(kind: Kind) -> bool {
    matches!(kind, Kind::Cultivator | Kind::Extractor | Kind::Butcher)
}

/// `x is IDigestable` — a dissolving human, the only thing with hit points.
#[must_use]
pub const fn is_dissolving(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::DissolvingMilitia
            | Kind::DissolvingHunter
            | Kind::DissolvingScout
            | Kind::DissolvingTank
            | Kind::DissolvingMech
            | Kind::DissolvingCaravan
    )
}

/// `x is IEatable` for actors — walking into one consumes it. Cities are not.
#[must_use]
pub const fn is_eatable(kind: Kind) -> bool {
    is_npc(kind)
}

/// `x is Upgradable` — has somewhere left to go.
#[must_use]
pub const fn is_upgradable(kind: Kind) -> bool {
    !kind.upgrade_paths().is_empty()
}

/// What a fresh human of this kind is for, before a gate briefs it.
///
/// A gate overwrites [`Orders::home`] and hands out escort duty as it lets
/// people out; everything else about a human's job comes from what it is.
#[must_use]
pub const fn default_duty(kind: Kind) -> Duty {
    match kind {
        Kind::Caravan => Duty::Convoy,
        Kind::Hunter => Duty::Investigate,
        Kind::Scout => Duty::Patrol,
        Kind::Militia | Kind::Tank | Kind::Mech => Duty::Garrison,
        _ => Duty::None,
    }
}

// ---------------------------------------------------------------------------
// Mutable per-actor state.
// ---------------------------------------------------------------------------

/// Crafting progress toward one locked-in upgrade path.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct UpgradeState {
    /// Index into the kind's [`Kind::upgrade_paths`], locked in by the first
    /// material absorbed.
    pub path: Option<usize>,
    /// Materials absorbed so far.
    pub progress: i32,
}

/// A ranged human's aim.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RangedState {
    /// Counts down from `firing_time`; zero means the shot goes off.
    pub firing: i32,
    /// Turns between painting a line and firing down it.
    pub firing_time: i32,
    /// Unit vector the painted line runs along.
    pub direction: Coord,
    /// Cells the painted line covers.
    pub range: i32,
}

/// A gate's wave timer and its queue of humans waiting to step outside.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CityState {
    /// Turns until the next wave is queued. Starts at fifty for every
    /// difficulty, then follows the difficulty's spawn rate.
    pub turns_to_next_wave: i32,
    /// Waves released so far.
    pub wave_number: i32,
    /// Drives the wave budget.
    pub level: i32,
    /// Humans waiting to be let out, one per turn.
    pub queue: VecDeque<Kind>,
    /// The caravan this gate has just sent out, if it still has one on the
    /// road.
    pub convoy: Option<ActorId>,
    /// Soldiers still owed to that caravan. Every one released while this is
    /// positive comes out as its escort.
    pub escorts: i32,
}

/// What a human does with a turn nobody is in sight for.
///
/// DELIBERATE CHANGE from C# (§11.5): the original had one answer — take a
/// random step, or stand still, with standing still as likely as any one
/// direction — so a cavern full of humans with nothing to do looked exactly
/// like a cavern full of humans with nothing to do. These are the answers
/// instead, and every one of them is a reason to be somewhere: a caravan is
/// going to the far gate, its escort goes wherever the caravan goes, a scout
/// walks the deep, a hunter walks down whatever was last reported, and a
/// garrison holds the ground outside its own gate.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Duty {
    /// Nothing in particular. Organelles, gates, and anybody nobody briefed.
    #[default]
    None,
    /// Walk a beat between the gate this human came out of and the nearest
    /// chamber.
    Garrison,
    /// Carry cargo to a distant gate and leave the map through it.
    Convoy,
    /// Keep station on a caravan.
    Escort(ActorId),
    /// Walk between the chambers the gates are furthest from.
    Patrol,
    /// Walk down whatever the humans last reported.
    Investigate,
}

/// A human's standing orders.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Orders {
    /// What it does when it cannot see you.
    pub duty: Duty,
    /// The doorway it came out of, and one end of a garrison's beat.
    pub home: Coord,
    /// Where the duty is taking it right now.
    pub goal: Option<Coord>,
}

/// One thing a human has committed to doing on its next turn.
///
/// Committing is the point. A human decides at the end of its turn, the
/// decision is on screen for the whole of yours, and then it does that exact
/// thing — so a blow aimed at a cell lands on that cell whatever is standing
/// there by the time it arrives, and stepping out of the way works.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Intent {
    /// What it means to do.
    pub kind: IntentKind,
    /// The cell it means to do it to.
    pub at: Coord,
}

/// The kinds of thing a human can commit to.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum IntentKind {
    /// Walk onto that cell.
    Step,
    /// Strike whatever of yours is standing there.
    Strike,
    /// Paint a line as far as that cell and start the countdown to a shot.
    Aim,
    /// Fire down the line already painted, as far as that cell.
    Fire,
    /// Stand still.
    Hold,
    /// Step into the gate there and be gone, cargo and all.
    Depart,
}

/// Whatever mutable state a kind needs beyond the common stat fields.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub enum Extra {
    /// Nothing beyond the common fields.
    #[default]
    None,
    /// An organelle with somewhere left to upgrade to.
    Upgrade(UpgradeState),
    /// A producer: a countdown, plus whatever upgrade it is partway through.
    Chloroplast {
        /// Turns until the next product.
        next_food: i32,
        /// Crafting progress, for the members of the line that have paths.
        upgrade: UpgradeState,
    },
    /// A dissolving human's banked extra product.
    Dissolving {
        /// Reaching the kind's max hit points frees one extra product.
        overfill: i32,
    },
    /// A ranged human's aim.
    Ranged(RangedState),
    /// A gate's wave state.
    City(CityState),
}

impl Extra {
    /// The upgrade progress inside this state, for the kinds that have any.
    #[must_use]
    pub const fn upgrade(&self) -> Option<&UpgradeState> {
        match self {
            Self::Upgrade(u) | Self::Chloroplast { upgrade: u, .. } => Some(u),
            _ => None,
        }
    }

    /// Mutable upgrade progress.
    pub const fn upgrade_mut(&mut self) -> Option<&mut UpgradeState> {
        match self {
            Self::Upgrade(u) | Self::Chloroplast { upgrade: u, .. } => Some(u),
            _ => None,
        }
    }
}

/// One thing standing on the map.
///
/// The stat fields start as copies of the kind's [`Stats`] and then drift:
/// upgrades rewrite them, the quantum core halves its own delay, and a
/// dissolving human loses a hit point per turn.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Actor {
    /// What it is.
    pub kind: Kind,
    /// Where it is.
    pub pos: Coord,
    /// Display name. Starts from the kind table; the original allowed it to be
    /// rewritten per instance.
    pub name: &'static str,
    /// Remaining digestion, for dissolving humans.
    pub hp: i32,
    /// Digestion length, for dissolving humans.
    pub max_hp: i32,
    /// Time units between actions.
    pub delay: i32,
    /// Taxicab sight radius, and the radius it lights.
    pub awareness: i32,
    /// Zero means walking into it consumes it.
    pub armor: i32,
    /// Player alignment and background index: 0 none, 1 body, 2 drag path.
    pub slime: u8,
    /// Immovable by the drag: the gravity core sets this while it pulls.
    pub anchor: bool,
    /// What this human has committed to doing on its next turn. `None` for
    /// everything that does not telegraph, and for a human that has only just
    /// stepped out of its gate and has yet to decide anything.
    pub intent: Option<Intent>,
    /// What it does with a turn nobody is in sight for.
    pub orders: Orders,
    /// Per-kind state.
    pub extra: Extra,
}

impl Actor {
    /// A fresh actor of `kind` at `pos`, with its kind's stats.
    #[must_use]
    pub fn new(kind: Kind, pos: Coord) -> Self {
        let stats = kind.stats();
        let extra = if is_dissolving(kind) {
            Extra::Dissolving { overfill: 0 }
        } else if is_hunter_family(kind) {
            // DELIBERATE CHANGE from C# (§5.6): `FiringTime` was 2, so a shot
            // cost aim, charge and fire — three turns, with the reticles up for
            // the last two. A gunman now announces that it is *about to* line
            // up a turn before it does, the way every other human announces its
            // move, so dropping the charge turn keeps the tempo the original
            // had while adding a turn of warning ahead of the reticles rather
            // than behind them.
            Extra::Ranged(RangedState {
                firing: 1,
                firing_time: 1,
                direction: Coord::new(0, 0),
                range: kind.range(),
            })
        } else if is_city(kind) {
            Extra::City(CityState {
                // Hard-coded in the original, and not the difficulty's spawn
                // rate: every gate's first wave lands on turn fifty.
                turns_to_next_wave: 50,
                wave_number: 0,
                level: 1,
                queue: VecDeque::new(),
                convoy: None,
                escorts: 0,
            })
        } else if let Some(next_food) = kind.next_food_init() {
            Extra::Chloroplast {
                next_food,
                upgrade: UpgradeState::default(),
            }
        } else if is_upgradable(kind) {
            Extra::Upgrade(UpgradeState::default())
        } else {
            Extra::None
        };
        Self {
            kind,
            pos,
            name: stats.name,
            hp: stats.max_hp,
            max_hp: stats.max_hp,
            delay: stats.delay,
            awareness: stats.awareness,
            armor: stats.armor,
            slime: stats.slime,
            anchor: false,
            intent: None,
            orders: Orders {
                duty: default_duty(kind),
                home: pos,
                goal: None,
            },
            extra,
        }
    }

    /// Whether this actor counts as part of the player's mass.
    #[must_use]
    pub const fn is_player_aligned(&self) -> bool {
        self.slime > 0
    }

    /// Everything this organelle drops when unslimed: its kind's components
    /// plus the materials its in-progress upgrade would have used.
    ///
    /// The original charged the full cost of the locked-in path regardless of
    /// how far along it was, so an interrupted upgrade refunds more than went
    /// in. Kept, because it is load-bearing for how forgiving the game feels.
    #[must_use]
    pub fn components(&self) -> Vec<ItemKind> {
        let mut out = self.kind.components().to_vec();
        if let Some(state) = self.extra.upgrade()
            && let Some(path) = state.path.and_then(|i| self.kind.upgrade_paths().get(i))
        {
            let material = match path.resource {
                Resource::Calcium => ItemKind::CalciumDust,
                Resource::Electronics => ItemKind::SiliconDust,
            };
            for _ in 0..path.amount {
                out.push(material);
            }
        }
        out
    }
}

/// A hunter's painted targeting mark.
///
/// Not an actor and not an item: the original kept these in a separate effects
/// layer, so nothing can stand on one, walk into one, or be blocked by one.
/// They exist to be drawn, to be shot down, and to be avoided by a retreating
/// nucleus.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Reticle {
    /// The cell the mark covers.
    pub pos: Coord,
    /// The hunter or scout that painted it.
    pub owner: ActorId,
}

/// A handle to an actor. Stable for the life of a run.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct ActorId(usize);

impl ActorId {
    /// The slot number behind this handle. Exposed so grids can key on it.
    #[must_use]
    pub const fn raw(self) -> usize {
        self.0
    }

    /// Build a handle from a slot number. For tests and index bookkeeping.
    #[must_use]
    pub const fn from_raw(raw: usize) -> Self {
        Self(raw)
    }
}

/// The actor arena.
///
/// Removing an actor leaves a tombstone; slots are never reused, so a handle
/// held across a removal reads back as absent rather than as somebody else.
#[derive(Clone, Debug, Default)]
pub struct ActorStore {
    slots: Vec<Option<Actor>>,
    live: usize,
}

impl ActorStore {
    /// An empty arena.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an actor and hand back its handle.
    pub(crate) fn spawn(&mut self, actor: Actor) -> ActorId {
        self.slots.push(Some(actor));
        self.live += 1;
        ActorId(self.slots.len() - 1)
    }

    /// Remove an actor, returning it if it was still there.
    pub(crate) fn remove(&mut self, id: ActorId) -> Option<Actor> {
        let gone = self.slots.get_mut(id.0)?.take();
        if gone.is_some() {
            self.live -= 1;
        }
        gone
    }

    /// The actor behind a handle, if it is still alive.
    #[must_use]
    pub fn get(&self, id: ActorId) -> Option<&Actor> {
        self.slots.get(id.0)?.as_ref()
    }

    /// Mutable access to a live actor.
    pub(crate) fn get_mut(&mut self, id: ActorId) -> Option<&mut Actor> {
        self.slots.get_mut(id.0)?.as_mut()
    }

    /// Whether a handle still names a live actor.
    #[must_use]
    pub fn contains(&self, id: ActorId) -> bool {
        self.get(id).is_some()
    }

    /// How many actors are alive.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.live
    }

    /// Whether nothing is alive.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// Slots ever allocated, including tombstones. Sized buffers key on this.
    // Not `const`: `Vec::len` only became callable in a const context after
    // this crate's minimum supported Rust version.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    /// Every live actor, in creation order — the order the original's `Actors`
    /// list kept.
    pub fn iter(&self) -> impl Iterator<Item = (ActorId, &Actor)> + '_ {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(i, slot)| slot.as_ref().map(|a| (ActorId(i), a)))
    }
}

impl Index<ActorId> for ActorStore {
    type Output = Actor;
    fn index(&self, id: ActorId) -> &Actor {
        self.get(id).expect("actor handle outlived its actor")
    }
}

impl IndexMut<ActorId> for ActorStore {
    fn index_mut(&mut self, id: ActorId) -> &mut Actor {
        self.get_mut(id).expect("actor handle outlived its actor")
    }
}

/// A handle to an item.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct ItemId(usize);

impl ItemId {
    /// The slot number behind this handle.
    #[must_use]
    pub const fn raw(self) -> usize {
        self.0
    }
}

/// One item on the floor. Items and actors share a cell freely; only one item
/// may occupy a cell at a time.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Item {
    /// What it is.
    pub kind: ItemKind,
    /// Where it is.
    pub pos: Coord,
}

/// The item arena, with the same never-reuse rule as [`ActorStore`].
#[derive(Clone, Debug, Default)]
pub struct ItemStore {
    slots: Vec<Option<Item>>,
    live: usize,
}

impl ItemStore {
    /// An empty arena.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an item and hand back its handle.
    pub(crate) fn spawn(&mut self, item: Item) -> ItemId {
        self.slots.push(Some(item));
        self.live += 1;
        ItemId(self.slots.len() - 1)
    }

    /// Remove an item, returning it if it was still there.
    pub(crate) fn remove(&mut self, id: ItemId) -> Option<Item> {
        let gone = self.slots.get_mut(id.0)?.take();
        if gone.is_some() {
            self.live -= 1;
        }
        gone
    }

    /// The item behind a handle, if it is still there.
    #[must_use]
    pub fn get(&self, id: ItemId) -> Option<&Item> {
        self.slots.get(id.0)?.as_ref()
    }

    /// How many items are on the floor.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.live
    }

    /// Whether the floor is bare.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.live == 0
    }
}

impl Index<ItemId> for ItemStore {
    type Output = Item;
    fn index(&self, id: ItemId) -> &Item {
        self.get(id).expect("item handle outlived its item")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every kind the game can produce, so table coverage tests stay honest.
    const ALL: [Kind; 37] = [
        Kind::Nucleus,
        Kind::EyeCore,
        Kind::SmartCore,
        Kind::LaserCore,
        Kind::TerrorCore,
        Kind::GravityCore,
        Kind::QuantumCore,
        Kind::Cytoplasm,
        Kind::Calcium,
        Kind::Electronics,
        Kind::Membrane,
        Kind::ReinforcedMembrane,
        Kind::Maw,
        Kind::ForceField,
        Kind::PhaseMembrane,
        Kind::ReinforcedMaw,
        Kind::Tentacle,
        Kind::Chloroplast,
        Kind::Bioreactor,
        Kind::Cultivator,
        Kind::BiometalForge,
        Kind::PrimordialSoup,
        Kind::Extractor,
        Kind::Butcher,
        Kind::Militia,
        Kind::Hunter,
        Kind::Scout,
        Kind::Tank,
        Kind::Mech,
        Kind::Caravan,
        Kind::City,
        Kind::DissolvingMilitia,
        Kind::DissolvingHunter,
        Kind::DissolvingScout,
        Kind::DissolvingTank,
        Kind::DissolvingMech,
        Kind::DissolvingCaravan,
    ];

    #[test]
    fn every_kind_has_a_name_and_a_glyph() {
        for kind in ALL {
            let stats = kind.stats();
            assert!(!stats.name.is_empty(), "{kind:?} has no name");
            assert!(!stats.description.is_empty(), "{kind:?} has no description");
            assert!(stats.delay >= 0);
        }
    }

    #[test]
    fn names_are_unique_except_where_the_original_shared_them() {
        let mut names: Vec<&str> = ALL.iter().map(|k| k.name()).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len(), "duplicate entity names");
    }

    #[test]
    fn armour_marks_exactly_the_tank_family() {
        for kind in ALL {
            assert_eq!(
                kind.stats().armor > 0,
                is_tank_family(kind),
                "{kind:?} armour disagrees with the tank family"
            );
        }
    }

    #[test]
    fn slime_marks_exactly_the_player_side() {
        for kind in ALL {
            assert_eq!(
                kind.stats().slime > 0,
                is_organelle(kind),
                "{kind:?} slime disagrees with organelle membership"
            );
        }
    }

    #[test]
    fn every_dissolving_form_round_trips() {
        for kind in ALL.into_iter().filter(|k| is_npc(*k)) {
            let dissolving = kind.becomes_on_eaten().expect("a human dissolves");
            assert!(is_dissolving(dissolving));
            assert_eq!(dissolving.rescues_to(), Some(kind));
            assert!(dissolving.stats().max_hp > 0);
            assert!(!dissolving.digests_to().is_empty());
            assert!(!kind.drops_on_die().is_empty());
        }
    }

    #[test]
    fn upgrade_paths_lead_somewhere_real() {
        for kind in ALL {
            for path in kind.upgrade_paths() {
                assert!(path.amount > 0);
                assert!(is_organelle(path.result));
                assert_ne!(path.result, kind);
            }
        }
    }

    #[test]
    fn reinforced_maw_is_not_a_reinforced_membrane() {
        // The original spelled this out as a separate `||`; folding it into
        // the family would give force fields a maw's behaviour and vice versa.
        assert!(!is_reinforced_membrane_family(Kind::ReinforcedMaw));
        assert!(kills_armored_attackers(Kind::ReinforcedMaw));
        assert!(!kills_armored_attackers(Kind::Maw));
        assert!(!kills_armored_attackers(Kind::Membrane));
        assert!(!kills_armored_attackers(Kind::Tentacle));
    }

    #[test]
    fn the_tank_family_is_the_armoured_subtree() {
        assert!(is_tank_family(Kind::Mech));
        assert!(is_tank_family(Kind::Caravan));
        assert!(!is_tank_family(Kind::Militia));
        assert!(is_militia_family(Kind::Tank));
        assert!(is_militia_family(Kind::Scout));
        assert!(!is_militia_family(Kind::City));
    }

    #[test]
    fn upgradable_agrees_with_the_path_table() {
        assert!(is_upgradable(Kind::Nucleus));
        assert!(!is_upgradable(Kind::LaserCore));
        assert!(!is_upgradable(Kind::Butcher));
        assert!(!is_upgradable(Kind::Cytoplasm));
    }

    #[test]
    fn components_include_a_locked_in_upgrade() {
        let mut membrane = Actor::new(Kind::Membrane, Coord::new(0, 0));
        assert_eq!(
            membrane.components(),
            vec![ItemKind::BarbedWire, ItemKind::Nutrient]
        );
        // Path 0 is one calcium toward a tough membrane.
        membrane.extra = Extra::Upgrade(UpgradeState {
            path: Some(0),
            progress: 1,
        });
        assert_eq!(
            membrane.components(),
            vec![
                ItemKind::BarbedWire,
                ItemKind::Nutrient,
                ItemKind::CalciumDust
            ]
        );
    }

    #[test]
    fn new_actors_pick_up_their_kind_state() {
        let city = Actor::new(Kind::City, Coord::new(1, 1));
        assert!(matches!(city.extra, Extra::City(_)));
        let corpse = Actor::new(Kind::DissolvingTank, Coord::new(1, 1));
        assert_eq!(corpse.hp, 24);
        assert_eq!(corpse.max_hp, 24);
        let plant = Actor::new(Kind::Chloroplast, Coord::new(1, 1));
        assert!(matches!(
            plant.extra,
            Extra::Chloroplast { next_food: 20, .. }
        ));
        let hunter = Actor::new(Kind::Hunter, Coord::new(1, 1));
        assert!(matches!(hunter.extra, Extra::Ranged(r) if r.range == 64));
    }

    #[test]
    fn handles_are_never_reused() {
        let mut store = ActorStore::new();
        let a = store.spawn(Actor::new(Kind::Cytoplasm, Coord::new(0, 0)));
        store.remove(a);
        let b = store.spawn(Actor::new(Kind::Cytoplasm, Coord::new(1, 0)));
        assert_ne!(a, b);
        assert!(!store.contains(a));
        assert!(store.contains(b));
        assert_eq!(store.len(), 1);
        assert_eq!(store.capacity(), 2);
    }

    #[test]
    fn iteration_is_in_creation_order() {
        let mut store = ActorStore::new();
        let ids: Vec<_> = (0..4)
            .map(|i| store.spawn(Actor::new(Kind::Cytoplasm, Coord::new(i, 0))))
            .collect();
        store.remove(ids[1]);
        let seen: Vec<_> = store.iter().map(|(id, _)| id).collect();
        assert_eq!(seen, vec![ids[0], ids[2], ids[3]]);
    }

    #[test]
    fn items_know_what_they_become() {
        assert_eq!(ItemKind::Nutrient.new_organelle(), Kind::Cytoplasm);
        assert_eq!(ItemKind::Dna.new_organelle(), Kind::Nucleus);
        assert_eq!(ItemKind::BarbedWire.new_organelle(), Kind::Membrane);
        assert_eq!(ItemKind::Plant.new_organelle(), Kind::Chloroplast);
    }
}
