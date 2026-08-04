//! The whole simulation: pure, deterministic, and macroquad-free.
//!
//! You are a set of organelles rather than a single body. There is no "player
//! entity" anywhere in here — the mass is a list, your score is its length,
//! and the gate you are trying to break needs a certain number of you leaning
//! on it. Everything else follows from that.
//!
//! The frontend's only channel in is a [`Command`]; its only channels out are
//! [`Sim::view`], which hands back a fully resolved grid of glyphs and
//! colours, and [`Sim::cues`], which says what just happened so a sound can be
//! chosen for it. Nothing in this module knows what a pixel or a sample is,
//! and nothing in it reads a clock: the same seed and the same commands always
//! produce the same run.
//!
//! Time is discrete and event-driven rather than fixed-step. Everything that
//! acts sits in the [`schedule`], sixteen time units make a turn, and the
//! world runs itself forward until a nucleus comes up — at which point it
//! stops and waits for you.

pub mod actors;
pub mod ai;
pub mod combat;
pub mod effect;
pub mod grid;
pub mod log;
pub mod mapgen;
pub mod organelles;
pub mod player;
pub mod schedule;
pub mod view;

use fastrand::Rng;

use actors::{Actor, ActorId, ActorStore, Item, ItemId, ItemKind, ItemStore, Kind, Reticle};
use effect::{Effect, EffectKind};
use grid::{Coord, Dir, Grid};
use log::{MessageLog, OrganelleLog};
use schedule::Schedule;

pub use view::RenderView;

/// Effects one [`Sim::advance`] will report before it stops recording.
///
/// This is a runaway backstop and not a display budget, which is why it is set
/// so far above what any frontend will draw. A real turn is bounded by the map:
/// every organelle in the mass can shuffle a cell and every human can take a
/// step, so a full cavern tops out around twice its cell count. Setting the cap
/// below that would silently drop the *last* interactions of a busy turn —
/// which are as likely to be the nucleus dying as another footstep.
const MAX_EFFECTS: usize = 4096;

/// How hard the humans push back, and how big the cavern is.
///
/// The original picked this from the command line. There is no command line on
/// the web, so it is a title-screen choice instead.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub enum Difficulty {
    /// Twelve gates, eight of which you must break.
    #[default]
    Normal,
    /// Fewer gates and slower waves.
    Easy,
    /// A wider cavern, sixteen gates, and every one of them must fall.
    Gj,
}

impl Difficulty {
    /// The constants this difficulty runs on.
    #[must_use]
    pub const fn rules(self) -> Rules {
        match self {
            Self::Normal => Rules {
                map_width: 48,
                map_height: 48,
                spawn_rate: 50,
                evolution_rate: 6,
                max_budget: 5,
                city_armor_step: 20,
                num_cities: 12,
                grace_cities: 4,
            },
            Self::Easy => Rules {
                map_width: 48,
                map_height: 48,
                spawn_rate: 75,
                evolution_rate: 7,
                max_budget: 5,
                city_armor_step: 20,
                num_cities: 10,
                grace_cities: 4,
            },
            Self::Gj => Rules {
                map_width: 64,
                map_height: 48,
                spawn_rate: 50,
                evolution_rate: 5,
                max_budget: 6,
                city_armor_step: 32,
                num_cities: 16,
                grace_cities: 0,
            },
        }
    }

    /// The name to show on a title screen.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Easy => "Easy",
            Self::Gj => "GJ",
        }
    }
}

/// Everything a [`Difficulty`] decides.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rules {
    /// Map columns. The original's console could not show more than 64.
    pub map_width: i32,
    /// Map rows. The original's console could not show more than 48.
    pub map_height: i32,
    /// Turns between a gate's waves, after the first.
    pub spawn_rate: i32,
    /// Waves per step up in wave budget.
    pub evolution_rate: i32,
    /// Ceiling on a wave's budget.
    pub max_budget: i32,
    /// Mass the first gate costs to break, and what every gate after it adds.
    ///
    /// DELIBERATE CHANGE from C# (§1), where every gate cost a flat
    /// `CityArmor` — 100 on Normal and Easy, 160 on GJ. That was the whole
    /// early game spent growing to a threshold nothing else needed. The first
    /// gate now costs a fifth of the old price and each one after it costs a
    /// fifth more, so the last gate a Normal run has to break still costs 160.
    pub city_armor_step: i32,
    /// Gates the map is generated with.
    pub num_cities: i32,
    /// Gates you may leave standing and still escape.
    pub grace_cities: i32,
}

impl Rules {
    /// Gates you must actually destroy.
    #[must_use]
    pub const fn cities_required(&self) -> i32 {
        self.num_cities - self.grace_cities
    }

    /// Mass needed to break the next gate once `destroyed` of them have fallen.
    #[must_use]
    pub const fn city_armor(&self, destroyed: i32) -> i32 {
        (destroyed + 1) * self.city_armor_step
    }
}

/// Where the run is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    /// No map yet; waiting for a difficulty to be chosen.
    Title,
    /// A run in progress.
    Playing,
    /// The run is over and the map is frozen behind the final messages.
    GameOver {
        /// Whether you escaped rather than died.
        won: bool,
    },
}

/// How the player is asking for things, which is all the sim's own prose needs
/// to know about the frontend.
///
/// The sim writes a handful of lines that name controls — the help the run
/// opens with, and the offer of another run once this one is over — and a phone
/// has no keys to name. This picks the wording; nothing else in the world
/// changes, so a given seed still resolves identically either way.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum InputStyle {
    /// A keyboard, which the lines can name keys for.
    #[default]
    Keys,
    /// A touchscreen, where the on-screen buttons are the only controls there
    /// are and naming a key would send the player looking for one.
    Touch,
}

/// Which panel the interface is showing, and therefore what the arrow keys do.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum UiMode {
    /// The message log. Arrows move the active nucleus.
    #[default]
    Messages,
    /// The organelle list. Arrows move the selection.
    Organelles,
    /// The examine cursor. Arrows move the cursor.
    Examine,
}

/// Everything the player can ask for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Command {
    /// Leave the title screen and generate a map.
    Start(Difficulty),
    /// Move, swap, eat or break a gate in a direction.
    Move(Dir),
    /// Pass the turn.
    Wait,
    /// Hand control to the next or previous nucleus. Free.
    CycleNucleus {
        /// Forwards through the acquisition order, or backwards.
        forward: bool,
    },
    /// Enter or leave examine mode.
    ToggleExamine,
    /// Move the examine cursor. Ignored outside examine mode.
    MoveCursor(Dir),
    /// Enter or leave the organelle browser.
    ToggleOrganelles,
    /// Move the organelle selection.
    ScrollOrganelles(i32),
    /// Move the organelle list by whole pages.
    PageOrganelles(i32),
    /// Leave whatever panel is open.
    Back,
    /// Print the controls into the message log.
    Help,
    /// Start a fresh run once this one is over. The seed comes from the
    /// frontend, because the sim never reads a clock.
    Restart {
        /// Seed for the new run.
        seed: u64,
    },
}

/// Something that just happened, for the frontend to make a noise about.
///
/// A cue says what happened, never what it should sound like, and lives for
/// exactly one [`Sim::advance`]. The list is a *set*: an event that happened
/// several times in one advance — twenty chloroplasts all producing on the same
/// turn — is reported once, because a frontend wants one sound for it and not
/// twenty.
///
/// [`Effect`] is the same news in the shape the eyes need it: ordered, one
/// entry per occurrence, and with the cells attached. The two channels stay
/// separate because those two shapes genuinely disagree — see
/// [`effect`](self::effect).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cue {
    /// The mass flowed one cell.
    Step,
    /// An action was refused: a wall, armour, or too little mass.
    Bump,
    /// An organelle traded places with another.
    Swap,
    /// A human was consumed by contact.
    Eat,
    /// A catalyst was absorbed.
    Ingest,
    /// A sealed-in group of humans was captured at once.
    Engulf,
    /// A dissolving human finished and became something useful.
    Digest,
    /// Something of yours other than a nucleus was destroyed or shot away.
    OrganelleLost,
    /// A nucleus died.
    NucleusLost,
    /// An organelle finished an upgrade and became something else.
    Upgrade,
    /// A producer put something new into the world.
    Produce,
    /// A ranged human painted a line and is now aiming down it.
    Aim,
    /// A ranged human fired.
    Shot,
    /// A terror core took an adjacent human's turn away.
    Terrify,
    /// A gate queued a fresh wave.
    WaveSpawned,
    /// A gate is fewer than ten turns from its next wave.
    GateCountdown,
    /// A caravan reached a gate and left the map with its cargo.
    Depart,
    /// A gate came down.
    CityDestroyed,
    /// The run ended in escape.
    Win,
    /// The run ended in death.
    Lose,
}

/// Somewhere a human has heard the amoeba is.
///
/// A rumor is how a human that cannot see you still knows which way to walk.
/// One is raised whenever somebody lays eyes on the mass or dies to it, and it
/// stops being worth walking to after a while — see [`Sim::raise_rumor`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rumor {
    /// Where the amoeba was said to be.
    pub pos: Coord,
    /// The instant this stops being news.
    pub until: u64,
}

/// One human a terror core has lifted off the schedule, and the speed it had
/// before it was frightened.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Terrified {
    /// The core holding it.
    pub core: ActorId,
    /// The human that is missing its turn.
    pub victim: ActorId,
    /// What to set its delay back to.
    pub delay: i32,
}

/// The world.
pub struct Sim {
    seed: u64,
    rng: Rng,
    difficulty: Difficulty,
    rules: Rules,
    phase: Phase,
    mode: UiMode,
    grid: Grid,
    actors: ActorStore,
    items: ItemStore,
    /// One slot per cell: the actor standing there, if any.
    actor_at: Vec<Option<ActorId>>,
    /// One slot per cell: the item lying there, if any.
    item_at: Vec<Option<ItemId>>,
    /// The mass, in acquisition order. Nucleus cycling and the sidebar both
    /// depend on that order.
    player_mass: Vec<ActorId>,
    cities: Vec<ActorId>,
    /// Gates broken so far. Every gate costs one step more than the last, so
    /// this is what the next one is priced from.
    cities_destroyed: i32,
    schedule: Schedule,
    active: Option<ActorId>,
    player_turn: bool,
    messages: MessageLog,
    organelles: OrganelleLog,
    cues: Vec<Cue>,
    /// What happened this advance and where, in the order it happened.
    effects: Vec<Effect>,
    cursor: Option<Coord>,
    /// Every cell a hunter or scout is currently aiming at. Not actors and not
    /// items: the original kept these in a separate effects layer that nothing
    /// could stand on or walk into.
    reticles: Vec<Reticle>,
    /// Humans a terror core is currently holding off the schedule.
    terrified: Vec<Terrified>,
    /// Where the humans think you are, newest last.
    rumors: Vec<Rumor>,
    /// The middle of every chamber the generator carved, in the order it
    /// carved them. These are the corners the humans patrol between.
    junctions: Vec<Coord>,
    /// One entry per cell: steps of open floor to the nearest gate doorway, or
    /// [`i32::MAX`] for rock and for anywhere a gate cannot reach. Fixed by
    /// [`mapgen`] and read by everything that cares where the deep is.
    depth: Vec<i32>,
    /// Bumped by every world mutation, so the drag-path cache can tell whether
    /// it is still describing the world it was computed from.
    version: u64,
    drag_cache: Option<player::SlimeTree>,
    /// Which way the frontend takes orders, for the lines that mention it.
    input_style: InputStyle,
}

impl Sim {
    /// A sim sitting on the title screen.
    ///
    /// Nothing is generated until [`Command::Start`] arrives, so the frontend
    /// can offer the difficulty choice the original took on the command line.
    #[must_use]
    pub fn new(seed: u64, difficulty: Difficulty) -> Self {
        let rules = difficulty.rules();
        let grid = Grid::new(rules.map_width, rules.map_height);
        let cells = (rules.map_width * rules.map_height).unsigned_abs() as usize;
        Self {
            seed,
            rng: Rng::with_seed(seed),
            difficulty,
            rules,
            phase: Phase::Title,
            mode: UiMode::Messages,
            grid,
            actors: ActorStore::new(),
            items: ItemStore::new(),
            actor_at: vec![None; cells],
            item_at: vec![None; cells],
            player_mass: Vec::new(),
            cities: Vec::new(),
            cities_destroyed: 0,
            schedule: Schedule::new(),
            active: None,
            player_turn: false,
            messages: MessageLog::new(),
            organelles: OrganelleLog::new(),
            cues: Vec::new(),
            effects: Vec::new(),
            cursor: None,
            reticles: Vec::new(),
            terrified: Vec::new(),
            rumors: Vec::new(),
            junctions: Vec::new(),
            depth: vec![i32::MAX; cells],
            version: 0,
            drag_cache: None,
            input_style: InputStyle::Keys,
        }
    }

    /// Tell the sim what the player is holding, so the lines that name controls
    /// name the right ones. Cheap and idempotent — the frontend sets it every
    /// frame, because a browser window can gain a touchscreen halfway through a
    /// run and a phone can gain a keyboard.
    pub const fn set_input_style(&mut self, style: InputStyle) {
        self.input_style = style;
    }

    /// The seed this run was generated from.
    #[must_use]
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    /// The difficulty in force.
    #[must_use]
    pub const fn difficulty(&self) -> Difficulty {
        self.difficulty
    }

    /// The constants in force.
    #[must_use]
    pub const fn rules(&self) -> Rules {
        self.rules
    }

    /// Where the run is.
    #[must_use]
    pub const fn phase(&self) -> Phase {
        self.phase
    }

    /// Which panel is open.
    #[must_use]
    pub const fn mode(&self) -> UiMode {
        self.mode
    }

    /// What happened during the last [`Sim::advance`].
    #[must_use]
    pub fn cues(&self) -> &[Cue] {
        &self.cues
    }

    /// The same news with cells attached, in the order it happened.
    #[must_use]
    pub fn effects(&self) -> &[Effect] {
        &self.effects
    }

    /// Whether the world is waiting on the player.
    #[must_use]
    pub const fn is_player_turn(&self) -> bool {
        self.player_turn
    }

    /// The current mass, which is also the score and the gate-breaking
    /// threshold.
    #[must_use]
    pub fn mass(&self) -> usize {
        self.player_mass.len()
    }

    /// The message log.
    #[must_use]
    pub const fn messages(&self) -> &MessageLog {
        &self.messages
    }

    /// Feed one command, then run the world forward until it needs you again.
    ///
    /// A command that does not end the turn (cycling nuclei, opening a panel)
    /// leaves the world exactly where it was. One that does ends up here with
    /// the whole non-player phase already resolved, so a caller can treat this
    /// as "one call, one turn".
    pub fn advance(&mut self, command: Option<Command>) {
        self.cues.clear();
        self.effects.clear();
        match (self.phase, command) {
            (Phase::Title, Some(Command::Start(difficulty))) => self.start(difficulty),
            (Phase::GameOver { .. }, Some(Command::Restart { seed })) => {
                let difficulty = self.difficulty;
                // The world starts over; what the player is holding does not.
                let style = self.input_style;
                *self = Self::new(seed, difficulty);
                self.input_style = style;
                self.start(difficulty);
            }
            (Phase::Playing, Some(command)) => self.handle(command),
            _ => {}
        }
        self.pump();
    }

    /// Generate a map and hand control to the first nucleus.
    fn start(&mut self, difficulty: Difficulty) {
        self.difficulty = difficulty;
        self.rules = difficulty.rules();
        self.phase = Phase::Playing;
        self.mode = UiMode::Messages;
        self.generate();
        self.write_help();
        // Building a cavern places a few hundred actors, every one of which
        // reports itself. None of that is news: the map did not *happen*, it
        // was always there.
        self.effects.clear();
    }

    /// Run the scheduler until a nucleus comes up or the run ends.
    fn pump(&mut self) {
        // Every inert organelle costs one scheduler round trip per turn, so a
        // large mass makes this loop long but never unbounded. The guard is
        // only here so a future bug cannot hang a browser tab.
        let mut guard = 0_u32;
        while self.phase == Phase::Playing && !self.player_turn && !self.schedule.is_empty() {
            self.advance_turn_step();
            guard += 1;
            if guard > 1_000_000 {
                break;
            }
        }
        self.organelles.observe_time(self.schedule.time());
    }

    /// One trip through the original's `AdvanceTurn` loop body.
    fn advance_turn_step(&mut self) {
        self.forget_stale_rumors();
        let Some(id) = self.schedule.pop() else {
            return;
        };
        let Some(actor) = self.actors.get(id) else {
            return;
        };
        if actors::is_nucleus_family(actor.kind) {
            // The only place the field of view is recomputed: doing it on
            // every actor move was the original's worst performance sink.
            self.update_player_fov();
            self.player_turn = true;
            self.set_active_nucleus(id);
        } else {
            self.act(id);
            if let Some(actor) = self.actors.get(id) {
                let delay = actor.delay;
                self.schedule.add(id, delay);
            }
        }
        // The one thing that happens whichever branch was taken: a terror core
        // coming off the schedule hands back everybody it frightened.
        if self
            .actors
            .get(id)
            .is_some_and(|a| a.kind == Kind::TerrorCore)
        {
            self.terror_post_schedule(id);
        }
    }

    /// What a non-nucleus actor does with its turn.
    ///
    /// Anything not named here is inert and simply re-queues itself — most of
    /// your body, in other words, along with plain membranes, force fields and
    /// butchers, all of which work by standing still.
    fn act(&mut self, id: ActorId) {
        let Some(actor) = self.actors.get(id) else {
            return;
        };
        let kind = actor.kind;
        if actors::is_dissolving(kind) {
            self.digest(id);
            return;
        }
        if actors::is_crafting_material(kind) {
            self.craft(id);
            return;
        }
        match kind {
            Kind::City => self.city_act(id),
            Kind::Militia
            | Kind::Tank
            | Kind::Mech
            | Kind::Hunter
            | Kind::Scout
            | Kind::Caravan => self.human_act(id),
            Kind::Maw | Kind::ReinforcedMaw => self.maw_act(id),
            Kind::Tentacle => self.tentacle_act(id),
            Kind::Chloroplast | Kind::Bioreactor | Kind::BiometalForge | Kind::PrimordialSoup => {
                self.produce_act(id);
            }
            Kind::Cultivator => self.cultivate(id),
            Kind::Extractor => {
                self.extract(id);
                self.cultivate(id);
            }
            _ => {}
        }
    }

    /// Route a command taken during play.
    fn handle(&mut self, command: Command) {
        let ends_turn = match command {
            Command::Move(dir) => match self.mode {
                UiMode::Examine => {
                    self.move_cursor(dir);
                    false
                }
                UiMode::Organelles => {
                    self.scroll_organelles(if matches!(dir, Dir::Up | Dir::Left) {
                        -1
                    } else {
                        1
                    });
                    false
                }
                UiMode::Messages => self.attack_move_player(dir),
            },
            Command::MoveCursor(dir) => {
                self.move_cursor(dir);
                false
            }
            Command::Wait => {
                self.messages.add("You wait.");
                true
            }
            Command::CycleNucleus { forward } => {
                self.next_nucleus(if forward { 1 } else { -1 });
                false
            }
            Command::ToggleExamine => {
                self.toggle_examine();
                false
            }
            Command::ToggleOrganelles => {
                self.mode = if self.mode == UiMode::Organelles {
                    UiMode::Messages
                } else {
                    self.cursor = None;
                    UiMode::Organelles
                };
                false
            }
            Command::ScrollOrganelles(by) => {
                self.scroll_organelles(by);
                false
            }
            Command::PageOrganelles(by) => {
                self.organelles.turn_page(by);
                false
            }
            Command::Back => {
                self.cursor = None;
                self.mode = UiMode::Messages;
                false
            }
            Command::Help => {
                self.write_help();
                false
            }
            Command::Start(_) | Command::Restart { .. } => false,
        };
        if ends_turn {
            self.player_turn = false;
        }
    }

    fn scroll_organelles(&mut self, by: i32) {
        let count = self.loggable().count();
        self.organelles.scroll(by, count);
    }

    fn toggle_examine(&mut self) {
        if self.cursor.is_some() {
            self.cursor = None;
            self.mode = UiMode::Messages;
            return;
        }
        // The cursor starts wherever attention already is: the highlighted
        // organelle if the list is open, otherwise the active nucleus.
        let start = if self.mode == UiMode::Organelles {
            let count = self.loggable().count();
            let idx = self.organelles.selected(count);
            self.loggable().nth(idx).map(|id| self.actors[id].pos)
        } else {
            self.active.map(|id| self.actors[id].pos)
        };
        self.cursor = Some(
            start.unwrap_or_else(|| Coord::new(self.grid.width() / 2, self.grid.height() / 2)),
        );
        self.mode = UiMode::Examine;
    }

    fn move_cursor(&mut self, dir: Dir) {
        if let Some(cursor) = self.cursor {
            let next = cursor + dir.offset();
            self.cursor = Some(Coord::new(
                next.x.clamp(0, self.grid.width() - 1),
                next.y.clamp(0, self.grid.height() - 1),
            ));
        }
    }

    fn write_help(&mut self) {
        // A phone is told what to press, not which key to find: the buttons on
        // screen are labelled with these words, so the two halves of the
        // interface say the same thing.
        let lines: &[&str] = match self.input_style {
            InputStyle::Keys => &[
                "Arrow keys: Move / Select",
                "Space: Wait",
                "X: Toggle examine mode",
                "Z: Toggle organelle browsing mode",
                "ESC: Back to player mode",
                "A, D: Cycle active nucleus",
            ],
            // Short enough to survive a phone's panel, which is half as wide
            // as the sixty-two columns this log wraps at, and ordered with the
            // essentials last: the panel fills upwards from the newest line,
            // so the last of these are the ones a player sees without asking.
            InputStyle::Touch => &[
                "List is what you are made of.",
                "Prev and next change nucleus.",
                "Look examines whatever you tap.",
                "Wait passes the turn.",
                "The pad moves you, and so does tapping a tile beside you.",
            ],
        };
        for line in lines {
            self.messages.add(line);
        }
        let required = self.rules.cities_required();
        self.messages
            .add(&format!("Destroy {required} cities to win"));
    }

    // -- world queries ------------------------------------------------------

    #[allow(clippy::cast_sign_loss)] // Guarded by the bounds check above it.
    fn cell_index(&self, c: Coord) -> Option<usize> {
        self.grid
            .in_bounds(c)
            .then(|| (c.y * self.grid.width() + c.x) as usize)
    }

    /// The actor standing at `c`, in constant time.
    #[must_use]
    pub fn actor_at(&self, c: Coord) -> Option<ActorId> {
        self.actor_at[self.cell_index(c)?]
    }

    /// The item lying at `c`, in constant time.
    #[must_use]
    pub fn item_at(&self, c: Coord) -> Option<ItemId> {
        self.item_at[self.cell_index(c)?]
    }

    /// The map.
    #[must_use]
    pub const fn grid(&self) -> &Grid {
        &self.grid
    }

    /// The actor arena.
    #[must_use]
    pub const fn actors(&self) -> &ActorStore {
        &self.actors
    }

    /// The item arena.
    #[must_use]
    pub const fn items(&self) -> &ItemStore {
        &self.items
    }

    /// The mass, in acquisition order.
    #[must_use]
    pub fn player_mass(&self) -> &[ActorId] {
        &self.player_mass
    }

    /// Gates still standing.
    #[must_use]
    pub fn cities(&self) -> &[ActorId] {
        &self.cities
    }

    /// Mass needed to break the next gate. Every gate standing is worth this
    /// much, and the price goes up by one step each time one falls.
    #[must_use]
    pub const fn city_armor(&self) -> i32 {
        self.rules.city_armor(self.cities_destroyed)
    }

    /// The nucleus under your control.
    #[must_use]
    pub const fn active_nucleus(&self) -> Option<ActorId> {
        self.active
    }

    /// The examine cursor, if examine mode is open.
    #[must_use]
    pub const fn cursor(&self) -> Option<Coord> {
        self.cursor
    }

    /// Every cell a hunter or scout is currently aiming at.
    #[must_use]
    pub fn reticles(&self) -> &[Reticle] {
        &self.reticles
    }

    /// Whether somebody is aiming at this cell.
    #[must_use]
    pub fn reticle_at(&self, c: Coord) -> bool {
        self.reticles.iter().any(|r| r.pos == c)
    }

    /// The middle of every chamber the generator carved.
    #[must_use]
    pub fn junctions(&self) -> &[Coord] {
        &self.junctions
    }

    /// How far this cell is from the nearest gate, in steps of open floor.
    ///
    /// Solid rock, and anywhere no gate can reach, answers [`i32::MAX`]. This
    /// is a property of the map and not of the moment: it is measured once,
    /// before anything is standing anywhere.
    #[must_use]
    pub fn depth_at(&self, c: Coord) -> i32 {
        self.cell_index(c).map_or(i32::MAX, |i| self.depth[i])
    }

    /// Where the humans currently think you are.
    #[must_use]
    pub fn rumors(&self) -> &[Rumor] {
        &self.rumors
    }

    /// Report that something happened, once per advance however often it did.
    pub(crate) fn cue(&mut self, cue: Cue) {
        if !self.cues.contains(&cue) {
            self.cues.push(cue);
        }
    }

    /// Report where something happened, every time it does.
    ///
    /// Whether the player could see it is settled here, while the field of view
    /// is still the one that was in force when it happened — a turn resolves in
    /// one go and the view is only recomputed when a nucleus comes up, so
    /// asking later would be asking about a different moment.
    pub(crate) fn effect(&mut self, kind: EffectKind, from: Coord, to: Coord) {
        if self.effects.len() < MAX_EFFECTS {
            let effect = Effect::new(kind, from, to);
            let seen = self.grid.in_fov(from) || self.grid.in_fov(to);
            self.effects
                .push(if seen { effect } else { effect.unseen() });
        }
    }

    /// Report something that happened in one cell.
    pub(crate) fn effect_at(&mut self, kind: EffectKind, at: Coord) {
        self.effect(kind, at, at);
    }

    /// The turn queue.
    #[must_use]
    pub const fn schedule(&self) -> &Schedule {
        &self.schedule
    }

    /// Sidebar state.
    #[must_use]
    pub const fn organelle_log(&self) -> &OrganelleLog {
        &self.organelles
    }

    /// Solid rock: unwalkable and holding nothing.
    ///
    /// An actor-occupied cell is deliberately *not* a wall. That is what lets
    /// bullets pass through bodies and loot-drop searches route around them.
    #[must_use]
    pub fn is_wall(&self, c: Coord) -> bool {
        !self.grid.walkable(c) && self.actor_at(c).is_none() && self.item_at(c).is_none()
    }

    /// Nothing standing and nothing lying here.
    #[must_use]
    pub fn is_empty_cell(&self, c: Coord) -> bool {
        self.actor_at(c).is_none() && self.item_at(c).is_none()
    }

    /// The mass entries the sidebar lists: everything but cytoplasm and raw
    /// crafting materials, in acquisition order.
    pub(crate) fn loggable(&self) -> impl Iterator<Item = ActorId> + '_ {
        self.player_mass.iter().copied().filter(|id| {
            self.actors
                .get(*id)
                .is_some_and(|a| a.kind != Kind::Cytoplasm && !actors::is_crafting_material(a.kind))
        })
    }

    // -- world mutation -----------------------------------------------------

    /// Place a new actor and wire it into every index.
    ///
    /// Adding an organelle next to a human runs that human's engulf check, so
    /// growing into a gap can capture whoever was standing in it.
    pub(crate) fn add_actor(&mut self, kind: Kind, pos: Coord) -> ActorId {
        let mut actor = Actor::new(kind, pos);
        if actors::is_city(kind) {
            actor.armor = self.city_armor();
        }
        let slime = actor.slime;
        let delay = actor.delay;
        let id = self.actors.spawn(actor);
        if let Some(i) = self.cell_index(pos) {
            self.actor_at[i] = Some(id);
        }
        self.grid.set_walkable(pos, false);
        if slime > 0 {
            self.player_mass.push(id);
        }
        if actors::is_city(kind) {
            self.cities.push(id);
        }
        self.schedule.add(id, delay);
        self.version += 1;
        if actors::is_organelle(kind) {
            let neighbours: Vec<Coord> = self.grid.adjacent(pos).collect();
            for c in neighbours {
                if let Some(other) = self.actor_at(c)
                    && self
                        .actors
                        .get(other)
                        .is_some_and(|a| actors::is_npc(a.kind))
                {
                    self.engulf(other);
                }
            }
        }
        id
    }

    /// Take an actor off the map. Its cell becomes walkable again.
    pub(crate) fn remove_actor(&mut self, id: ActorId) -> Option<Actor> {
        let actor = self.actors.remove(id)?;
        if let Some(i) = self.cell_index(actor.pos)
            && self.actor_at[i] == Some(id)
        {
            self.actor_at[i] = None;
        }
        self.grid.set_walkable(actor.pos, true);
        self.player_mass.retain(|&m| m != id);
        self.cities.retain(|&c| c != id);
        self.schedule.remove(id);
        if self.active == Some(id) {
            self.active = None;
        }
        // A hunter that dies stops aiming, whether it died to a membrane, to a
        // shot of its own side's, or by being swallowed whole.
        self.reticles.retain(|r| r.owner != id);
        if !self.terrified.is_empty() {
            self.release_terrified(id);
        }
        self.version += 1;
        Some(actor)
    }

    /// Move an actor, but only onto a cell nothing is standing on.
    ///
    /// Every walk in the game comes through here — yours, the humans', and the
    /// shuffle of every organelle dragged along behind you — so this is the one
    /// place a [`EffectKind::Flow`] has to be reported from for the mass to be
    /// seen flowing.
    pub(crate) fn set_actor_position(&mut self, id: ActorId, dest: Coord) -> bool {
        if !self.grid.walkable(dest) || !self.actors.contains(id) {
            return false;
        }
        let from = self.actors[id].pos;
        self.effect(EffectKind::Flow, from, dest);
        if let Some(i) = self.cell_index(from) {
            self.actor_at[i] = None;
        }
        self.grid.set_walkable(from, true);
        if let Some(i) = self.cell_index(dest) {
            self.actor_at[i] = Some(id);
        }
        self.grid.set_walkable(dest, false);
        self.actors[id].pos = dest;
        self.version += 1;
        true
    }

    /// Exchange two actors' cells. Walkability is untouched, because both
    /// cells stay occupied.
    pub(crate) fn swap_actors(&mut self, a: ActorId, b: ActorId) {
        if a == b || !self.actors.contains(a) || !self.actors.contains(b) {
            return;
        }
        let pa = self.actors[a].pos;
        let pb = self.actors[b].pos;
        if let Some(i) = self.cell_index(pa) {
            self.actor_at[i] = Some(b);
        }
        if let Some(i) = self.cell_index(pb) {
            self.actor_at[i] = Some(a);
        }
        self.actors[a].pos = pb;
        self.actors[b].pos = pa;
        self.version += 1;
    }

    /// Drop an item on a cell that has none.
    pub(crate) fn add_item(&mut self, kind: ItemKind, pos: Coord) -> Option<ItemId> {
        let i = self.cell_index(pos)?;
        if self.item_at[i].is_some() {
            return None;
        }
        let id = self.items.spawn(Item { kind, pos });
        self.item_at[i] = Some(id);
        self.version += 1;
        Some(id)
    }

    /// Take an item off the map.
    pub(crate) fn remove_item(&mut self, id: ItemId) -> Option<Item> {
        let item = self.items.remove(id)?;
        if let Some(i) = self.cell_index(item.pos)
            && self.item_at[i] == Some(id)
        {
            self.item_at[i] = None;
        }
        self.version += 1;
        Some(item)
    }

    /// Recompute what the mass can see: the union of a taxicab diamond around
    /// every organelle in it. A cytoplasm with no awareness still lights its
    /// own cell, which is why your own body is never dark.
    pub(crate) fn update_player_fov(&mut self) {
        self.grid.clear_fov();
        let lights: Vec<(Coord, i32)> = self
            .player_mass
            .iter()
            .filter_map(|id| self.actors.get(*id))
            .filter(|a| a.awareness >= 0)
            .map(|a| (a.pos, a.awareness))
            .collect();
        for (pos, radius) in lights {
            self.grid.append_fov(pos, radius, true);
        }
        for y in 0..self.grid.height() {
            for x in 0..self.grid.width() {
                let c = Coord::new(x, y);
                if self.grid.in_fov(c) {
                    self.grid.set_explored(c, true);
                }
            }
        }
    }

    /// Ring-by-ring search outward from `from` for somewhere to put something.
    ///
    /// The search spreads only through cells that are not solid rock, and
    /// returns the whole first ring that holds at least one legal cell — the
    /// caller picks among them at random. Gates never count as legal.
    pub(crate) fn nearest_drops(
        &self,
        from: Coord,
        legal: impl Fn(&Self, Coord) -> bool,
    ) -> Vec<Coord> {
        let cells = (self.grid.width() * self.grid.height()).unsigned_abs() as usize;
        let mut seen = vec![false; cells];
        if let Some(i) = self.cell_index(from) {
            seen[i] = true;
        }
        let mut ring = vec![from];
        loop {
            let found: Vec<Coord> = ring
                .iter()
                .copied()
                .filter(|c| {
                    !self.is_wall(*c)
                        && !self
                            .actor_at(*c)
                            .is_some_and(|a| self.actors[a].kind == Kind::City)
                        && legal(self, *c)
                })
                .collect();
            if !found.is_empty() {
                return found;
            }
            let mut next = Vec::new();
            for c in ring {
                for n in self.grid.adjacent(c) {
                    if let Some(i) = self.cell_index(n)
                        && !seen[i]
                        && !self.is_wall(n)
                    {
                        seen[i] = true;
                        next.push(n);
                    }
                }
            }
            if next.is_empty() {
                return Vec::new();
            }
            ring = next;
        }
    }

    /// Nearest cells with nothing standing on them.
    pub(crate) fn nearest_no_actor(&self, from: Coord) -> Vec<Coord> {
        self.nearest_drops(from, |sim, c| sim.actor_at(c).is_none())
    }

    /// Nearest cells with no item, and no organelle of yours standing there.
    pub(crate) fn nearest_loot_drops(&self, from: Coord) -> Vec<Coord> {
        self.nearest_drops(from, |sim, c| {
            sim.item_at(c).is_none()
                && sim
                    .actor_at(c)
                    .is_none_or(|a| sim.actors.get(a).is_none_or(|actor| actor.slime == 0))
        })
    }

    /// Pick one of a list uniformly, the way every `Rand.Next(0, n - 1)` in
    /// the original did.
    pub(crate) fn pick<T: Copy>(&mut self, from: &[T]) -> Option<T> {
        grid::rand_index(&mut self.rng, from.len()).map(|i| from[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sim mid-run, ready to be poked.
    pub(super) fn playing(seed: u64) -> Sim {
        let mut sim = Sim::new(seed, Difficulty::Normal);
        sim.advance(Some(Command::Start(Difficulty::Normal)));
        sim
    }

    /// A twenty-by-twenty walled arena with nothing in it, for posing exact
    /// situations that map generation would never hand you.
    pub(super) fn sandbox(seed: u64) -> Sim {
        let mut sim = Sim::new(seed, Difficulty::Normal);
        sim.phase = Phase::Playing;
        sim.grid = Grid::new(20, 20);
        sim.actor_at = vec![None; 400];
        sim.item_at = vec![None; 400];
        sim.depth = vec![0; 400];
        for y in 0..20 {
            for x in 0..20 {
                let solid = x == 0 || y == 0 || x == 19 || y == 19;
                sim.grid.set_props(Coord::new(x, y), !solid, !solid, true);
            }
        }
        sim
    }

    /// A row of actors starting at `start` and stepping by `step`.
    pub(super) fn line(sim: &mut Sim, kinds: &[Kind], start: Coord, step: Coord) -> Vec<ActorId> {
        let mut at = start;
        let mut ids = Vec::new();
        for kind in kinds {
            ids.push(sim.add_actor(*kind, at));
            at = at + step;
        }
        ids
    }

    #[test]
    fn difficulty_tables_match_the_spec() {
        let normal = Difficulty::Normal.rules();
        assert_eq!((normal.map_width, normal.map_height), (48, 48));
        assert_eq!(normal.cities_required(), 8);
        let easy = Difficulty::Easy.rules();
        assert_eq!(easy.spawn_rate, 75);
        assert_eq!(easy.cities_required(), 6);
        let gj = Difficulty::Gj.rules();
        assert_eq!((gj.map_width, gj.map_height), (64, 48));
        assert_eq!(gj.city_armor(0), 32);
        assert_eq!(gj.cities_required(), 16);
    }

    #[test]
    fn every_gate_costs_one_step_more_than_the_last() {
        let normal = Difficulty::Normal.rules();
        assert_eq!(normal.city_armor(0), 20, "the first gate is cheap");
        assert_eq!(normal.city_armor(1), 40);
        // The last gate a Normal run has to break costs what every gate used to.
        assert_eq!(normal.city_armor(normal.cities_required() - 1), 160);
    }

    #[test]
    fn breaking_a_gate_reprices_the_ones_left_standing() {
        let mut sim = playing(28);
        let step = sim.rules.city_armor_step;
        assert!(sim.cities().iter().all(|id| sim.actors[*id].armor == step));
        let doomed = sim.cities()[0];
        sim.destroy_city(doomed);
        assert_eq!(sim.city_armor(), 2 * step);
        assert!(
            sim.cities()
                .iter()
                .all(|id| sim.actors[*id].armor == 2 * step),
            "every gate still standing went up together"
        );
    }

    #[test]
    fn a_new_sim_waits_on_the_title_screen() {
        let sim = Sim::new(1, Difficulty::Normal);
        assert_eq!(sim.phase(), Phase::Title);
        assert_eq!(sim.mass(), 0);
    }

    #[test]
    fn starting_generates_a_map_and_hands_over_control() {
        let sim = playing(42);
        assert_eq!(sim.phase(), Phase::Playing);
        assert!(sim.is_player_turn());
        assert_eq!(sim.mass(), 6);
        assert!(sim.active_nucleus().is_some());
    }

    #[test]
    fn the_scheduler_hands_the_first_turn_to_a_nucleus() {
        let sim = playing(9);
        let active = sim.active_nucleus().expect("a nucleus is in charge");
        assert!(actors::is_nucleus_family(sim.actors[active].kind));
        assert_eq!(sim.schedule().time(), 16);
    }

    #[test]
    fn occupancy_and_walkability_agree() {
        let sim = playing(3);
        for (id, actor) in sim.actors.iter() {
            assert_eq!(sim.actor_at(actor.pos), Some(id));
            assert!(!sim.grid().walkable(actor.pos));
        }
    }

    #[test]
    fn examine_mode_starts_on_the_active_nucleus() {
        let mut sim = playing(11);
        let active = sim.active_nucleus().unwrap();
        sim.advance(Some(Command::ToggleExamine));
        assert_eq!(sim.cursor(), Some(sim.actors[active].pos));
        assert_eq!(sim.mode(), UiMode::Examine);
        sim.advance(Some(Command::ToggleExamine));
        assert_eq!(sim.cursor(), None);
    }

    #[test]
    fn the_examine_cursor_is_clamped_to_the_map() {
        let mut sim = playing(12);
        sim.advance(Some(Command::ToggleExamine));
        for _ in 0..100 {
            sim.advance(Some(Command::MoveCursor(Dir::Up)));
            sim.advance(Some(Command::MoveCursor(Dir::Left)));
        }
        assert_eq!(sim.cursor(), Some(Coord::new(0, 0)));
    }

    #[test]
    fn opening_a_panel_does_not_cost_a_turn() {
        let mut sim = playing(13);
        let before = sim.schedule().time();
        sim.advance(Some(Command::ToggleOrganelles));
        sim.advance(Some(Command::ScrollOrganelles(1)));
        sim.advance(Some(Command::CycleNucleus { forward: true }));
        sim.advance(Some(Command::Help));
        assert_eq!(sim.schedule().time(), before);
        assert!(sim.is_player_turn());
    }

    #[test]
    fn the_help_a_touchscreen_gets_never_names_a_key() {
        // A phone has no arrow keys, no space bar and no escape, so a line
        // that names one sends the player looking for something that is not
        // there. The words it uses instead are the ones on the buttons.
        let mut sim = Sim::new(21, Difficulty::Normal);
        sim.set_input_style(InputStyle::Touch);
        sim.advance(Some(Command::Start(Difficulty::Normal)));
        let log = sim.messages().lines().join(" ").to_lowercase();
        for key in ["arrow key", "space", "esc", "press "] {
            assert!(!log.contains(key), "the touch help still says {key:?}");
        }
        for word in ["wait", "look", "list", "prev", "next"] {
            assert!(log.contains(word), "the touch help never says {word:?}");
        }
    }

    #[test]
    fn a_keyboard_still_gets_told_which_keys() {
        let sim = playing(21);
        let log = sim.messages().lines().join(" ");
        assert!(log.contains("Arrow keys"));
        assert!(log.contains("Space"));
    }

    #[test]
    fn the_style_survives_a_restart() {
        // Restarting builds a whole new world, but the player is still holding
        // the same thing — so the fresh run's help has to come out the same.
        let mut sim = Sim::new(22, Difficulty::Normal);
        sim.set_input_style(InputStyle::Touch);
        sim.advance(Some(Command::Start(Difficulty::Normal)));
        sim.phase = Phase::GameOver { won: false };
        sim.advance(Some(Command::Restart { seed: 23 }));
        let log = sim.messages().lines().join(" ");
        assert!(!log.contains("Arrow keys"), "the restart forgot the thumbs");
    }

    #[test]
    fn waiting_always_costs_a_turn() {
        let mut sim = playing(14);
        let before = sim.schedule().time();
        sim.advance(Some(Command::Wait));
        assert_eq!(sim.schedule().time(), before + 16);
        assert!(sim.is_player_turn());
    }

    /// A deterministic stream of plausible key presses.
    fn scripted_command(rng: &mut Rng) -> Command {
        match rng.u32(0..14) {
            0 | 11 => Command::Move(Dir::Up),
            1 | 12 => Command::Move(Dir::Down),
            2 | 13 => Command::Move(Dir::Left),
            3 => Command::Move(Dir::Right),
            4 => Command::Wait,
            5 => Command::CycleNucleus { forward: true },
            6 => Command::CycleNucleus { forward: false },
            7 => Command::ToggleExamine,
            8 => Command::MoveCursor(Dir::Right),
            9 => Command::ToggleOrganelles,
            10 => Command::ScrollOrganelles(1),
            _ => Command::Back,
        }
    }

    /// Everything that must be true of the world between any two commands.
    fn check_invariants(sim: &Sim, note: &str) {
        for (id, actor) in sim.actors.iter() {
            assert_eq!(
                sim.actor_at(actor.pos),
                Some(id),
                "{note}: the index lost {:?} at {:?}",
                actor.kind,
                actor.pos
            );
            assert!(
                !sim.grid.walkable(actor.pos),
                "{note}: {:?} is standing on a walkable cell",
                actor.kind
            );
        }
        for y in 0..sim.grid.height() {
            for x in 0..sim.grid.width() {
                let c = Coord::new(x, y);
                if sim.grid.walkable(c) {
                    assert!(
                        sim.actor_at(c).is_none(),
                        "{note}: {c:?} is doubly occupied"
                    );
                }
                if let Some(id) = sim.item_at(c) {
                    assert_eq!(sim.items[id].pos, c, "{note}: the item index disagrees");
                }
            }
        }
        for id in &sim.player_mass {
            let actor = sim
                .actors
                .get(*id)
                .unwrap_or_else(|| panic!("{note}: the mass holds a dead handle"));
            assert!(
                actor.slime > 0,
                "{note}: {:?} is in the mass without slime",
                actor.kind
            );
        }
        for (id, actor) in sim.actors.iter() {
            if actor.slime > 0 {
                assert!(
                    sim.player_mass.contains(&id),
                    "{note}: {:?} has slime but is not in the mass",
                    actor.kind
                );
            }
        }
        assert!(
            sim.schedule.duplicate_entry().is_none(),
            "{note}: something holds two schedule entries and will act twice"
        );
        for reticle in &sim.reticles {
            assert!(
                sim.actors.contains(reticle.owner),
                "{note}: a reticle outlived the hunter that painted it"
            );
        }
        for held in &sim.terrified {
            assert!(
                sim.actors.contains(held.core),
                "{note}: a dead terror core is still holding somebody"
            );
        }
    }

    #[test]
    fn the_same_seed_and_commands_produce_the_same_view() {
        let mut a = playing(2024);
        let mut b = playing(2024);
        let mut script = Rng::with_seed(11);
        let commands: Vec<Command> = (0..200).map(|_| scripted_command(&mut script)).collect();
        for command in commands {
            a.advance(Some(command));
            b.advance(Some(command));
        }
        assert_eq!(a.view(3), b.view(3));
        assert_eq!(a.schedule().time(), b.schedule().time());
        assert_eq!(a.mass(), b.mass());
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = playing(1);
        let mut b = playing(2);
        for _ in 0..50 {
            a.advance(Some(Command::Move(Dir::Right)));
            b.advance(Some(Command::Move(Dir::Right)));
        }
        assert_ne!(a.view(0), b.view(0));
    }

    #[test]
    fn a_long_random_playout_keeps_the_world_consistent() {
        let mut ended = 0;
        for seed in 0..12 {
            let mut sim = playing(seed);
            let mut script = Rng::with_seed(seed ^ 0x9E37);
            for step in 0..800 {
                sim.advance(Some(scripted_command(&mut script)));
                if step % 25 == 0 {
                    check_invariants(&sim, &format!("seed {seed} step {step}"));
                }
                if sim.phase() != Phase::Playing {
                    ended += 1;
                    break;
                }
            }
            check_invariants(&sim, &format!("seed {seed} end"));
            assert!(sim.is_player_turn() || sim.phase() != Phase::Playing);
        }
        // Flailing at random with the waves running is not survivable, and the
        // point of saying so here is that the run really did go all the way to
        // a real loss rather than idling: the invariants above were checked
        // against a world with combat, waves and production all live in it.
        assert!(ended > 0, "random play should sometimes end the run");
    }

    #[test]
    fn waves_actually_arrive_and_the_gates_count_down() {
        let mut sim = playing(31);
        for _ in 0..60 {
            sim.advance(Some(Command::Wait));
        }
        let humans = sim
            .actors
            .iter()
            .filter(|(_, a)| actors::is_npc(a.kind))
            .count();
        assert!(humans > 0, "the first wave is out by turn sixty");
        let queued: usize = sim
            .cities
            .iter()
            .filter_map(|id| match &sim.actors[*id].extra {
                actors::Extra::City(state) => Some(state.queue.len()),
                _ => None,
            })
            .sum();
        let waves: i32 = sim
            .cities
            .iter()
            .filter_map(|id| match &sim.actors[*id].extra {
                actors::Extra::City(state) => Some(state.wave_number),
                _ => None,
            })
            .sum();
        assert_eq!(waves, 12, "one wave out of each of the twelve gates");
        assert!(humans + queued >= 12, "and at least one human each");
    }

    /// How close a human has to be before the bot stops collecting and pulls
    /// its head in, and how many of them make a crowd.
    const CROWD_RANGE: i32 = 3;
    /// Humans within [`CROWD_RANGE`] the bot will stand its ground against.
    const CROWD_LIMIT: usize = 1;

    /// A greedy player: eat whatever is beside you, burrow when you are
    /// outnumbered, collect what grows the mass, and once you are big enough go
    /// and lean on a gate.
    ///
    /// It is not clever. It never plans an upgrade, never lays a trap, and
    /// never sets up a membrane wall — it just walks at the nearest useful
    /// thing. That is the point: everything it does is something the sim has to
    /// resolve correctly turn after turn, and the result is a far more
    /// adversarial exercise of the whole machine than any scripted case.
    fn greedy_command(sim: &Sim) -> Command {
        let Some(active) = sim.active_nucleus() else {
            return Command::Wait;
        };
        let Some(from) = sim.actors.get(active).map(|a| a.pos) else {
            return Command::Wait;
        };
        let toward = |step: Coord| match (step.x - from.x, step.y - from.y) {
            (1, _) => Command::Move(Dir::Right),
            (-1, _) => Command::Move(Dir::Left),
            (_, 1) => Command::Move(Dir::Down),
            _ => Command::Move(Dir::Up),
        };
        // Anything unarmoured beside us is a free meal and one fewer attacker.
        for dir in [Dir::Left, Dir::Right, Dir::Up, Dir::Down] {
            let cell = from + dir.offset();
            if let Some(id) = sim.actor_at(cell)
                && let Some(actor) = sim.actors.get(id)
                && actors::is_npc(actor.kind)
                && actor.armor == 0
            {
                return Command::Move(dir);
            }
        }
        let crowd = sim
            .actors
            .iter()
            .filter(|(_, a)| actors::is_npc(a.kind) && a.pos.taxi(from) <= CROWD_RANGE)
            .count();
        if crowd > CROWD_LIMIT {
            // Outnumbered: swap deeper into the body, where nothing can reach
            // the nucleus and there is always somebody to retreat into.
            let mut best = (body_around(sim, from), None);
            for dir in [Dir::Left, Dir::Right, Dir::Up, Dir::Down] {
                let cell = from + dir.offset();
                let ours = sim
                    .actor_at(cell)
                    .and_then(|a| sim.actors.get(a))
                    .is_some_and(|a| a.slime > 0 && !actors::is_nucleus_family(a.kind));
                if ours && body_around(sim, cell) > best.0 {
                    best = (body_around(sim, cell), Some(dir));
                }
            }
            if let Some(dir) = best.1 {
                return Command::Move(dir);
            }
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let mass = sim.player_mass.len() as i32;
        let big_enough = sim
            .cities
            .first()
            .and_then(|id| sim.actors.get(*id))
            .is_some_and(|city| mass >= city.armor);
        let targets: Vec<Coord> = if big_enough {
            sim.cities
                .iter()
                .filter_map(|id| sim.actors.get(*id))
                .map(|a| a.pos)
                .collect()
        } else {
            // Food first, because it is the only catalyst that grows the mass
            // by itself. The rest need a spare cytoplasm to grow inside.
            let spare = sim
                .player_mass
                .iter()
                .filter(|id| sim.actors[**id].kind == Kind::Cytoplasm)
                .count();
            let mut loot: Vec<Coord> = (0..sim.grid.height())
                .flat_map(|y| (0..sim.grid.width()).map(move |x| Coord::new(x, y)))
                .filter(|c| {
                    sim.item_at(*c)
                        .and_then(|i| sim.items.get(i))
                        .is_some_and(|i| i.kind == ItemKind::Nutrient || spare > 2)
                })
                .collect();
            if loot.is_empty() {
                loot.extend(
                    sim.actors
                        .iter()
                        .filter(|(_, a)| actors::is_npc(a.kind) && a.armor == 0)
                        .map(|(_, a)| a.pos),
                );
            }
            loot
        };
        // Route through open floor and through our own body, which is what a
        // player does when the mass is in the way of itself.
        let paths = sim.shortest_paths_to(from, &targets, |s, c| {
            s.grid.walkable(c)
                || s.actor_at(c)
                    .and_then(|a| s.actors.get(a))
                    .is_some_and(|a| a.slime > 0 || (actors::is_npc(a.kind) && a.armor == 0))
        });
        paths
            .first()
            .and_then(|p| p.get(1))
            .map_or(Command::Wait, |step| toward(*step))
    }

    /// How much of the mass surrounds a cell.
    fn body_around(sim: &Sim, at: Coord) -> usize {
        sim.grid
            .adjacent(at)
            .filter(|c| {
                sim.actor_at(*c)
                    .and_then(|a| sim.actors.get(a))
                    .is_some_and(|a| a.slime > 0)
            })
            .count()
    }

    /// Drive the greedy bot until the run ends or the budget runs out.
    ///
    /// A refused action costs nothing, so the clock standing still means the
    /// bot has walked into something it cannot pass; it waits that turn out
    /// rather than spinning on the same wall forever.
    fn play_out(sim: &mut Sim, budget: usize, note: &str) -> usize {
        let mut peak = sim.mass();
        let mut stalled = 0;
        for turn in 0..budget {
            let before = sim.schedule().time();
            let command = greedy_command(sim);
            sim.advance(Some(command));
            peak = peak.max(sim.mass());
            if sim.phase() != Phase::Playing {
                return peak;
            }
            if turn % 200 == 0 {
                check_invariants(sim, &format!("{note} turn {turn}"));
            }
            if sim.schedule().time() == before {
                stalled += 1;
                sim.advance(Some(Command::Wait));
                assert!(stalled < 500, "{note}: the bot is wedged");
            } else {
                stalled = 0;
            }
        }
        peak
    }

    #[test]
    #[ignore = "a whole playthrough; run with --ignored"]
    fn a_greedy_bot_survives_a_whole_easy_run() {
        for seed in 0..6 {
            let mut sim = Sim::new(seed, Difficulty::Easy);
            sim.advance(Some(Command::Start(Difficulty::Easy)));
            let peak = play_out(&mut sim, 4000, &format!("seed {seed}"));
            check_invariants(&sim, &format!("seed {seed} end"));
            assert!(peak > 10, "seed {seed}: the bot never got going ({peak})");
            assert!(
                sim.is_player_turn() || sim.phase() != Phase::Playing,
                "seed {seed}: the run stopped mid-phase"
            );
        }
    }

    /// The win, end to end, in a real playout.
    ///
    /// The gates' wave timers are held back for this one. That is the only
    /// thing changed: map generation, the drag, eating, digestion, crafting,
    /// production, the mass threshold, the grace-city count and the escape all
    /// run exactly as they do in a live run, and the bot really does walk a
    /// hundred-cell amoeba across the cavern and break six gates with it.
    ///
    /// The waves are held back because they have to be. On the original's
    /// numbers ten gates put something like fifty humans on an Easy map by turn
    /// three hundred, and a bot that only walks at the nearest useful thing
    /// loses every organelle it has long before it reaches a hundred mass.
    /// Beating that needs membrane walls, engulf traps and tentacles — real
    /// play. What this proves is that the win path itself is reachable and
    /// correct; that it is *hard* is the game, and `a_greedy_bot_survives_a_
    /// whole_easy_run` covers the same machinery under full pressure.
    #[test]
    #[ignore = "a whole playthrough; run with --ignored"]
    fn a_greedy_bot_can_win_an_unopposed_easy_run() {
        let mut won = 0;
        for seed in 0..6 {
            let mut sim = Sim::new(seed, Difficulty::Easy);
            sim.advance(Some(Command::Start(Difficulty::Easy)));
            for city in sim.cities.clone() {
                if let actors::Extra::City(state) = &mut sim.actors[city].extra {
                    state.turns_to_next_wave = i32::MAX;
                }
            }
            play_out(&mut sim, 4000, &format!("unopposed seed {seed}"));
            check_invariants(&sim, &format!("unopposed seed {seed} end"));
            if sim.phase() == (Phase::GameOver { won: true }) {
                won += 1;
                // The last gate it broke was the dearest one, and it had to be
                // standing in front of it with that much mass.
                let last = sim.rules.city_armor(sim.cities_destroyed - 1);
                assert!(
                    sim.mass() >= last.unsigned_abs() as usize,
                    "seed {seed}: it won without the mass to do it"
                );
                assert!(
                    sim.cities().len() <= sim.rules.grace_cities.unsigned_abs() as usize,
                    "seed {seed}: it won with too many gates standing"
                );
            }
        }
        assert!(won >= 4, "an unopposed easy run is winnable: {won}/6");
    }

    #[test]
    fn nothing_reaches_the_log_without_something_to_look_at() {
        // The reason the effect stream exists: a player who turns the message
        // log off must not have turned the game's explanations off with it. So
        // every advance with something to *say* needs somewhere to point.
        for seed in 0..8 {
            let mut sim = playing(seed);
            let mut script = Rng::with_seed(seed ^ 0x5EED);
            for step in 0..400 {
                let command = scripted_command(&mut script);
                let before = sim.messages().lines().len();
                sim.advance(Some(command));
                let said = sim.messages().lines().len() - before;
                // Waiting narrates the interface rather than the world, and its
                // one line is the only one with nothing on the map behind it.
                let narration = usize::from(command == Command::Wait);
                assert!(
                    said <= narration || !sim.effects().is_empty(),
                    "seed {seed} step {step}: {said} lines and nothing to see"
                );
                if sim.phase() != Phase::Playing {
                    break;
                }
            }
        }
    }

    #[test]
    fn a_fresh_map_is_not_reported_as_news() {
        // Generating a cavern places a few hundred actors. None of it happened;
        // it was always there, and animating it would be a light show.
        let sim = playing(5);
        assert!(sim.effects().is_empty());
        assert!(sim.mass() > 0, "something really was placed");
    }

    #[test]
    fn effects_live_for_exactly_one_advance() {
        let mut sim = playing(6);
        // Moving always reports something: the mass flowed, or it was refused.
        sim.advance(Some(Command::Move(Dir::Right)));
        assert!(!sim.effects().is_empty(), "a step reported nothing");
        sim.advance(Some(Command::ToggleOrganelles));
        assert!(
            sim.effects().is_empty(),
            "opening a panel changes nothing and should report nothing"
        );
    }

    #[test]
    fn what_happens_in_the_dark_is_marked_as_such() {
        // Twelve gates queue a wave every fifty turns, almost always out in the
        // dark. They are still reported — a frontend may want to say something
        // about them — but nothing may draw them onto cells you have not seen.
        let mut sim = sandbox(8);
        let nucleus = sim.add_actor(Kind::Nucleus, Coord::new(2, 2));
        sim.update_player_fov();
        sim.effects.clear();
        sim.effect_at(EffectKind::Wave, sim.actors[nucleus].pos);
        sim.effect_at(EffectKind::Wave, Coord::new(17, 17));
        assert!(sim.effects()[0].in_sight, "its own cell is always lit");
        assert!(!sim.effects()[1].in_sight, "the far corner is not");
    }

    #[test]
    fn the_effect_list_is_bounded() {
        // A hundred-cell amoeba flowing one cell reports every organelle that
        // shuffled, and a late-game turn has waves in it besides. The cap is
        // the only thing standing between that and an unbounded list.
        let mut sim = sandbox(7);
        for i in 0..(MAX_EFFECTS + 50) {
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            let at = Coord::new(i as i32 % 20, i as i32 / 20);
            sim.effect_at(EffectKind::Flow, at);
        }
        assert_eq!(sim.effects().len(), MAX_EFFECTS);
    }

    #[test]
    fn a_real_turn_never_reaches_the_cap() {
        // If it did, the backstop would be silently deciding what the player
        // gets to see. The biggest turn this can produce is the whole mass
        // flowing at once with every human on the map moving too.
        let mut sim = playing(15);
        let mut script = Rng::with_seed(4);
        let mut worst = 0;
        for _ in 0..300 {
            sim.advance(Some(scripted_command(&mut script)));
            worst = worst.max(sim.effects().len());
            if sim.phase() != Phase::Playing {
                break;
            }
        }
        assert!(worst > 0, "nothing was reported at all");
        assert!(worst < MAX_EFFECTS / 2, "a real turn reported {worst}");
    }

    #[test]
    fn views_survive_being_asked_for_at_any_frame() {
        let mut sim = playing(77);
        for frame in 0..24 {
            sim.advance(Some(Command::Wait));
            let view = sim.view(frame);
            assert_eq!(view.cells.len(), 48 * 48);
        }
    }
}
