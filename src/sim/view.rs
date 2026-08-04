//! Turning the world into something drawable.
//!
//! [`Sim::view`] resolves every visibility rule here rather than in the
//! frontend: what a cell looks like, whether you can see it, whether you only
//! remember it, which colour a nucleus is because it happens to be the one you
//! are steering. What comes out is a grid of glyphs and colours plus the panel
//! contents, and a renderer can draw it without knowing that maws or gates
//! exist.
//!
//! Animation is a pure function of the frame counter the caller passes in, so
//! two sims fed the same commands and the same frame produce byte-identical
//! views. Nothing here reads a clock either.

use super::actors::{self, ActorId, Extra, IntentKind, ItemId, Kind, Resource, Rgb, palette};
use super::grid::Coord;
use super::log::VISIBLE_LINES;
use super::{Phase, Sim, UiMode};

/// Animation frames per blink step. Blinking things are on for three frames
/// and off for three, at the four-frames-per-second the original ticked at.
const BLINK_SPEED: u32 = 3;

/// Steps in a blink cycle.
const BLINK_FRAMES: u32 = 2;

/// Code page 437 glyph 16: aiming to the right.
const ARROW_RIGHT: char = '\u{25BA}';
/// Code page 437 glyph 17: aiming to the left.
const ARROW_LEFT: char = '\u{25C4}';
/// Code page 437 glyph 30: aiming up.
const ARROW_UP: char = '\u{25B2}';
/// Code page 437 glyph 31: aiming down.
const ARROW_DOWN: char = '\u{25BC}';

/// One drawn map cell.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CellView {
    /// The character to draw. Space means nothing is there.
    pub glyph: char,
    /// Foreground colour.
    pub fg: Rgb,
    /// Background colour.
    pub bg: Rgb,
}

/// A progress bar under a sidebar entry.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Bar {
    /// How full, in `0..=1`.
    pub fraction: f32,
    /// Colour of the filled part.
    pub fill: Rgb,
    /// Colour of the empty part.
    pub empty: Rgb,
    /// A second, banked fraction drawn over the top: a dissolving human that
    /// has already earned a free product.
    pub overfill: Option<f32>,
}

/// One row of the organelle sidebar.
#[derive(Clone, PartialEq, Debug)]
pub struct OrganelleEntry {
    /// Display name.
    pub name: String,
    /// Flavour text, for whichever panel is showing descriptions.
    pub description: String,
    /// The colour this organelle draws in. The gravity core's dark green is
    /// remapped to ordinary slime here so the row stays readable.
    pub fg: Rgb,
    /// Where it is, so a frontend can point at it.
    pub pos: Coord,
    /// The organelle browser's cursor is on this row.
    pub selected: bool,
    /// The examine cursor is over this organelle.
    pub examined: bool,
    /// Nucleus navigation hint: `@` for the active one, `A` and `D` for the
    /// ones the cycle keys reach. Only set when there is more than one.
    pub nucleus_hint: Option<char>,
    /// Production, digestion or crafting progress.
    pub bar: Option<Bar>,
}

/// One human's committed next move, ready to be drawn over its head.
///
/// This is the visible half of the promise the sim makes in
/// [`Intent`](super::actors::Intent): what is drawn here is not a guess about
/// what a human might do, it is the thing it has already committed to and will
/// do whatever the player does in between.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Telegraph {
    /// The cell the human is standing on.
    pub from: Coord,
    /// The cell it has committed to.
    pub to: Coord,
    /// What it means to do there.
    pub kind: IntentKind,
}

/// Something the examine cursor can report on.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Described {
    /// Display name.
    pub name: String,
    /// Flavour text.
    pub description: String,
    /// Extra lines: upgrade hints, and what it is standing on.
    pub detail: Vec<String>,
    /// The colour the name should be drawn in.
    pub color: Rgb,
}

/// The examine cursor and whatever it is over.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExamineView {
    /// Where the cursor is.
    pub pos: Coord,
    /// What it can see there, if anything.
    pub target: Option<Described>,
}

/// The numbers along the top of the sidebar.
#[derive(Clone, PartialEq, Debug)]
pub struct Status {
    /// Turns elapsed. Never decreases, even though winning rewinds the clock.
    pub turn: f32,
    /// Organelles in the mass. This is also the score and the gate threshold.
    pub mass: usize,
    /// Gates still standing.
    pub cities_remaining: usize,
    /// Gates that must come down for this difficulty.
    pub cities_required: i32,
    /// Mass needed to break the next gate. Goes up with every gate that falls.
    pub city_armor: i32,
    /// The nucleus under your control.
    pub active_nucleus: Option<(String, Coord)>,
}

/// Everything a frontend needs to draw one frame.
#[derive(Clone, PartialEq, Debug)]
pub struct RenderView {
    /// Map columns.
    pub width: i32,
    /// Map rows.
    pub height: i32,
    /// Row-major map cells.
    pub cells: Vec<CellView>,
    /// The last few message-log lines, oldest first.
    pub messages: Vec<String>,
    /// The organelle sidebar.
    pub organelles: Vec<OrganelleEntry>,
    /// Header numbers.
    pub status: Status,
    /// The examine cursor, when examine mode is open.
    pub examine: Option<ExamineView>,
    /// Cells that will move when you do. Already reflected in the cell
    /// backgrounds; here as well so a frontend can do something else with it.
    pub highlight: Vec<Coord>,
    /// Cells a hunter or scout is aiming at. Already blinking in the cells;
    /// here as well so a frontend can warn about them some other way.
    pub reticles: Vec<Coord>,
    /// What every human in sight has committed to doing next.
    pub telegraphs: Vec<Telegraph>,
    /// Where the run is.
    pub phase: Phase,
    /// Which panel is open.
    pub mode: UiMode,
}

impl RenderView {
    /// The cell at `(x, y)`, if it is on the map.
    #[must_use]
    pub fn cell(&self, x: i32, y: i32) -> Option<CellView> {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return None;
        }
        #[allow(clippy::cast_sign_loss)] // Guarded above.
        self.cells.get((y * self.width + x) as usize).copied()
    }
}

impl Sim {
    /// Resolve the world into something drawable.
    ///
    /// `anim_frame` drives every blink and every arrow; pass a counter that
    /// ticks about four times a second.
    #[must_use]
    pub fn view(&self, anim_frame: u32) -> RenderView {
        let blink_on = (anim_frame / BLINK_SPEED) % BLINK_FRAMES == 0;
        let (w, h) = (self.grid.width(), self.grid.height());
        let mut cells = Vec::with_capacity((w * h).unsigned_abs() as usize);
        for y in 0..h {
            for x in 0..w {
                cells.push(self.cell_view(Coord::new(x, y), blink_on));
            }
        }
        if let Some(cursor) = self.cursor
            && blink_on
            && let Some(i) = self.cell_index(cursor)
        {
            // The cursor is always visible, even on cells you have never seen.
            cells[i] = CellView {
                glyph: 'X',
                fg: palette::CURSOR,
                bg: palette::DARK_CURSOR,
            };
        }
        RenderView {
            width: w,
            height: h,
            cells,
            messages: self.messages.tail(VISIBLE_LINES).to_vec(),
            organelles: self.organelle_entries(),
            status: self.status(),
            examine: self.cursor.map(|pos| ExamineView {
                pos,
                target: self.describe_under(pos),
            }),
            highlight: self
                .player_mass
                .iter()
                .filter_map(|id| self.actors.get(*id))
                .filter(|a| a.slime == 2)
                .map(|a| a.pos)
                .collect(),
            reticles: self.reticles.iter().map(|r| r.pos).collect(),
            telegraphs: self.telegraphs(),
            phase: self.phase,
            mode: self.mode,
        }
    }

    /// What every human you can see has committed to doing next.
    ///
    /// Only the ones in sight. Reading somebody's intentions is a thing you do
    /// by looking at them, and an arrow drawn over unexplored black would say
    /// where a human is as plainly as drawing the human would.
    fn telegraphs(&self) -> Vec<Telegraph> {
        self.actors
            .iter()
            .filter(|(_, actor)| actors::is_npc(actor.kind) && self.grid.in_fov(actor.pos))
            .filter_map(|(_, actor)| {
                actor.intent.map(|intent| Telegraph {
                    from: actor.pos,
                    to: intent.at,
                    kind: intent.kind,
                })
            })
            .collect()
    }

    /// Floor, wall, or nothing at all where you have never been.
    fn base_tile(&self, c: Coord) -> CellView {
        let cell = self.grid.cell(c);
        // Occupancy clears walkability, so ask whether the cell is rock rather
        // than whether it is walkable; otherwise everything you stand on would
        // remember itself as a wall.
        let rock = self.is_wall(c);
        if cell.in_fov {
            if rock {
                CellView {
                    glyph: '#',
                    fg: palette::WALL_FOV,
                    bg: palette::WALL_BACKGROUND_FOV,
                }
            } else {
                CellView {
                    glyph: '.',
                    fg: palette::FLOOR_FOV,
                    bg: palette::FLOOR_BACKGROUND_FOV,
                }
            }
        } else if cell.explored {
            if rock {
                CellView {
                    glyph: '#',
                    fg: palette::WALL,
                    bg: palette::WALL_BACKGROUND,
                }
            } else {
                CellView {
                    glyph: '.',
                    fg: palette::FLOOR,
                    bg: palette::FLOOR_BACKGROUND,
                }
            }
        } else {
            CellView {
                glyph: ' ',
                fg: palette::FLOOR_BACKGROUND,
                bg: palette::FLOOR_BACKGROUND,
            }
        }
    }

    /// One fully resolved map cell.
    ///
    /// Everything except a gate is line-of-sight only: out of sight it falls
    /// back to remembered floor rather than showing where it went. A gate is
    /// remembered, drawn in dull stone once you have seen it.
    fn cell_view(&self, c: Coord, blink_on: bool) -> CellView {
        let base = self.base_tile(c);
        let cell = self.grid.cell(c);
        // A reticle sits on top of whatever it covers and blinks off every
        // other beat, which is how you see what is about to be shot.
        if blink_on && cell.in_fov && self.reticle_at(c) {
            return CellView {
                glyph: 'X',
                fg: palette::RETICLE_FOREGROUND,
                bg: palette::RETICLE_BACKGROUND,
            };
        }
        if let Some(id) = self.actor_at(c)
            && let Some(actor) = self.actors.get(id)
        {
            if actors::is_city(actor.kind) {
                let (glyph, fg, bg) = self.city_tile(id, blink_on);
                if cell.in_fov {
                    return CellView { glyph, fg, bg };
                } else if cell.explored {
                    return CellView {
                        glyph,
                        fg: palette::DB_STONE,
                        bg,
                    };
                }
                return base;
            }
            if cell.in_fov {
                return CellView {
                    glyph: self.actor_glyph(id, blink_on),
                    fg: self.actor_color(id),
                    bg: slime_background(actor.slime),
                };
            }
            return base;
        }
        if let Some(id) = self.item_at(c)
            && cell.in_fov
            && let Some(item) = self.items.get(id)
        {
            return CellView {
                glyph: item.kind.glyph(),
                fg: item.kind.color(),
                bg: palette::FLOOR_BACKGROUND_FOV,
            };
        }
        base
    }

    /// A ranged human replaces its glyph with an arrow while it is aiming.
    fn actor_glyph(&self, id: ActorId, blink_on: bool) -> char {
        let actor = &self.actors[id];
        if let Extra::Ranged(ranged) = actor.extra
            && ranged.firing < ranged.firing_time
            && blink_on
        {
            return match (ranged.direction.x, ranged.direction.y) {
                (x, _) if x > 0 => ARROW_RIGHT,
                (x, _) if x < 0 => ARROW_LEFT,
                (_, y) if y > 0 => ARROW_DOWN,
                _ => ARROW_UP,
            };
        }
        actor.kind.stats().glyph
    }

    /// Nuclei show whether they are the one you are steering; armoured humans
    /// show whether they are about to act.
    fn actor_color(&self, id: ActorId) -> Rgb {
        let actor = &self.actors[id];
        let stats = actor.kind.stats();
        let Some(resting) = stats.fg_resting else {
            return stats.fg;
        };
        if actors::is_nucleus_family(actor.kind) {
            return if self.active == Some(id) {
                stats.fg
            } else {
                resting
            };
        }
        // A tank that will act within one turn is shown awake.
        let acting_soon = self
            .schedule
            .scheduled_for(id)
            .is_some_and(|at| at.saturating_sub(self.schedule.time()) <= super::schedule::TURN);
        if acting_soon { stats.fg } else { resting }
    }

    /// A gate flashes its spawn queue or its countdown on the off beat.
    fn city_tile(&self, id: ActorId, blink_on: bool) -> (char, Rgb, Rgb) {
        let plain = (Kind::City.stats().glyph, palette::CITY, slime_background(0));
        let Extra::City(state) = &self.actors[id].extra else {
            return plain;
        };
        let queued = state.queue.len();
        if blink_on || (queued == 0 && state.turns_to_next_wave >= 10) {
            return plain;
        }
        if queued > 0 {
            let glyph = if queued > 9 {
                '*'
            } else {
                char::from_digit(u32::try_from(queued).unwrap_or(9), 10).unwrap_or('*')
            };
            return (glyph, palette::ELECTRONICS, palette::RETICLE_FOREGROUND);
        }
        let glyph = char::from_digit(state.turns_to_next_wave.unsigned_abs(), 10).unwrap_or('*');
        (glyph, palette::ELECTRONICS, palette::RETICLE_BACKGROUND)
    }

    /// The sidebar: everything in the mass except cytoplasm and raw materials.
    fn organelle_entries(&self) -> Vec<OrganelleEntry> {
        let listed: Vec<ActorId> = self.loggable().collect();
        let selected = self.organelles.selected(listed.len());
        let nuclei: Vec<ActorId> = self
            .player_mass
            .iter()
            .copied()
            .filter(|id| {
                self.actors
                    .get(*id)
                    .is_some_and(|a| actors::is_nucleus_family(a.kind))
            })
            .collect();
        let active_at = self
            .active
            .and_then(|a| nuclei.iter().position(|&n| n == a));
        listed
            .iter()
            .enumerate()
            .filter_map(|(row, id)| {
                let actor = self.actors.get(*id)?;
                let stats = actor.kind.stats();
                let fg = if stats.fg == palette::DARK_SLIME {
                    palette::SLIME
                } else {
                    stats.fg
                };
                let hint = active_at.and_then(|at| {
                    if nuclei.len() < 2 {
                        return None;
                    }
                    let here = nuclei.iter().position(|n| n == id)?;
                    let prev = (at + nuclei.len() - 1) % nuclei.len();
                    let next = (at + 1) % nuclei.len();
                    if here == at {
                        Some('@')
                    } else if here == prev {
                        Some('A')
                    } else if here == next {
                        Some('D')
                    } else {
                        None
                    }
                });
                Some(OrganelleEntry {
                    name: actor.name.to_owned(),
                    description: stats.description.to_owned(),
                    fg,
                    pos: actor.pos,
                    selected: self.mode == UiMode::Organelles && row == selected,
                    examined: self.cursor == Some(actor.pos),
                    nucleus_hint: hint,
                    bar: self.entry_bar(*id),
                })
            })
            .collect()
    }

    /// Whichever progress bar this organelle wants to show. Production first,
    /// then digestion, then crafting — the same precedence the original drew
    /// them in.
    fn entry_bar(&self, id: ActorId) -> Option<Bar> {
        let actor = self.actors.get(id)?;
        #[allow(clippy::cast_precision_loss)]
        match &actor.extra {
            Extra::Chloroplast { next_food, .. } => Some(Bar {
                fraction: 1.0 - (*next_food as f32 / actor.delay.max(1) as f32).clamp(0.0, 1.0),
                fill: palette::OVERFILL,
                empty: palette::ROOT_ORGANELLE,
                overfill: None,
            }),
            Extra::Dissolving { overfill } => Some(Bar {
                fraction: 1.0 - (actor.hp as f32 / actor.max_hp.max(1) as f32).clamp(0.0, 1.0),
                fill: palette::SLIME,
                empty: palette::ROOT_ORGANELLE,
                overfill: (*overfill > 0)
                    .then(|| (*overfill as f32 / actor.max_hp.max(1) as f32).clamp(0.0, 1.0)),
            }),
            Extra::Upgrade(state) => {
                let path = state.path.and_then(|i| actor.kind.upgrade_paths().get(i))?;
                Some(Bar {
                    fraction: (state.progress as f32 / path.amount.max(1) as f32).clamp(0.0, 1.0),
                    fill: path.resource.color(),
                    empty: match path.resource {
                        Resource::Calcium => palette::RESTING_TANK,
                        Resource::Electronics => palette::RETICLE_BACKGROUND,
                    },
                    overfill: None,
                })
            }
            Extra::None | Extra::Ranged(_) | Extra::City(_) => None,
        }
    }

    fn status(&self) -> Status {
        Status {
            turn: self.organelles.turn(),
            mass: self.player_mass.len(),
            cities_remaining: self.cities.len(),
            cities_required: self.rules.cities_required(),
            city_armor: self.city_armor(),
            active_nucleus: self
                .active
                .and_then(|id| self.actors.get(id).map(|a| (a.name.to_owned(), a.pos))),
        }
    }

    /// What the examine cursor can make out at `pos`.
    ///
    /// A gate you have already seen can be examined from memory; everything
    /// else has to be in sight.
    fn describe_under(&self, pos: Coord) -> Option<Described> {
        let cell = self.grid.cell(pos);
        if let Some(id) = self.actor_at(pos)
            && let Some(actor) = self.actors.get(id)
        {
            let remembered_gate = actors::is_city(actor.kind) && cell.explored;
            if cell.in_fov || remembered_gate {
                return Some(self.describe_actor(id));
            }
            return None;
        }
        if cell.in_fov
            && let Some(id) = self.item_at(pos)
        {
            return Some(self.describe_item(id));
        }
        None
    }

    fn describe_actor(&self, id: ActorId) -> Described {
        let actor = &self.actors[id];
        let stats = actor.kind.stats();
        let mut detail = Vec::new();
        match &actor.extra {
            Extra::City(gate) => {
                detail.push(format!("It takes {} mass to break this gate.", actor.armor));
                detail.push(format!(
                    "{} waiting to come out; wave {} in {} turns.",
                    gate.queue.len(),
                    gate.level,
                    gate.turns_to_next_wave
                ));
            }
            Extra::Dissolving { overfill } => {
                detail.push(format!(
                    "It has {} of {} left to dissolve.",
                    actor.hp, actor.max_hp
                ));
                if *overfill > 0 {
                    detail.push(format!("It is overfull by {overfill}."));
                }
            }
            Extra::Chloroplast { next_food, .. } => {
                detail.push(format!("It produces again in {next_food} turns."));
            }
            Extra::Upgrade(_) | Extra::Ranged(_) | Extra::None => {}
        }
        if actors::is_tank_family(actor.kind)
            && let Some(at) = self.schedule.scheduled_for(id)
        {
            detail.push(format!(
                "It acts in {} turns.",
                at.saturating_sub(self.schedule.time()) / super::schedule::TURN
            ));
        }
        detail.extend(self.upgrade_detail(id));
        if let Some(item) = self.item_at(actor.pos)
            && let Some(item) = self.items.get(item)
        {
            detail.push(format!("It is standing on a {}.", item.kind.name()));
        }
        Described {
            name: actor.name.to_owned(),
            description: stats.description.to_owned(),
            detail,
            color: if actors::is_organelle(actor.kind) {
                palette::SLIME
            } else {
                palette::MILITIA
            },
        }
    }

    /// The "it can be upgraded with" lines, or the "it needs N more" line once
    /// a path is locked in.
    fn upgrade_detail(&self, id: ActorId) -> Vec<String> {
        let actor = &self.actors[id];
        let paths = actor.kind.upgrade_paths();
        let Some(state) = actor.extra.upgrade() else {
            return Vec::new();
        };
        state.path.and_then(|i| paths.get(i)).map_or_else(
            || {
                paths
                    .iter()
                    .map(|p| {
                        format!(
                            "It can be upgraded with {} {}.",
                            p.amount,
                            p.resource.name()
                        )
                    })
                    .collect()
            },
            |path| {
                vec![format!(
                    "It needs {} more {}.",
                    (path.amount - state.progress).max(0),
                    path.resource.name()
                )]
            },
        )
    }

    fn describe_item(&self, id: ItemId) -> Described {
        let item = &self.items[id];
        Described {
            name: item.kind.name().to_owned(),
            description: item.kind.description().to_owned(),
            detail: vec![format!(
                "Eating it would grow a {}.",
                item.kind.new_organelle().name()
            )],
            color: palette::ROOT_ORGANELLE,
        }
    }
}

/// Backgrounds are driven by how player-aligned a cell is.
const fn slime_background(slime: u8) -> Rgb {
    match slime {
        0 => palette::FLOOR_BACKGROUND_FOV,
        1 => palette::BODY_SLIME,
        2 => palette::PATH_SLIME,
        _ => palette::FLOOR_BACKGROUND,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::grid::Dir;
    use crate::sim::tests::playing;
    use crate::sim::{Command, Difficulty, Sim};

    #[test]
    fn the_view_covers_the_whole_map() {
        let sim = playing(1);
        let view = sim.view(0);
        assert_eq!(view.width, 48);
        assert_eq!(view.height, 48);
        assert_eq!(view.cells.len(), 48 * 48);
    }

    #[test]
    fn unexplored_cells_are_blank() {
        let sim = playing(2);
        let view = sim.view(0);
        let blank = view
            .cells
            .iter()
            .filter(|c| c.glyph == ' ' && c.bg == palette::FLOOR_BACKGROUND)
            .count();
        assert!(blank > 0, "a fresh map is mostly unexplored");
    }

    #[test]
    fn the_mass_is_drawn_on_slime_backgrounds() {
        let sim = playing(3);
        let view = sim.view(0);
        for id in sim.player_mass() {
            let actor = &sim.actors()[*id];
            let cell = view.cell(actor.pos.x, actor.pos.y).expect("on the map");
            assert!(
                cell.bg == palette::BODY_SLIME || cell.bg == palette::PATH_SLIME,
                "organelle at {:?} drew on {:?}",
                actor.pos,
                cell.bg
            );
        }
    }

    #[test]
    fn the_active_nucleus_is_brighter_than_the_others() {
        let sim = playing(4);
        let active = sim.active_nucleus().unwrap();
        let others: Vec<_> = sim
            .player_mass()
            .iter()
            .copied()
            .filter(|id| *id != active && actors::is_nucleus_family(sim.actors()[*id].kind))
            .collect();
        let view = sim.view(0);
        let at = |id: ActorId| {
            let pos = sim.actors()[id].pos;
            view.cell(pos.x, pos.y).unwrap()
        };
        assert_eq!(at(active).fg, palette::ROOT_ORGANELLE);
        for other in others {
            assert_eq!(at(other).fg, palette::PLAYER_INACTIVE);
        }
    }

    #[test]
    fn a_remembered_gate_is_drawn_in_stone() {
        let mut sim = playing(5);
        // Explore a gate by fiat, then confirm it survives leaving sight.
        let city = sim.cities()[0];
        let pos = sim.actors()[city].pos;
        sim.grid.set_explored(pos, true);
        let view = sim.view(0);
        let cell = view.cell(pos.x, pos.y).unwrap();
        assert_eq!(cell.glyph, 'C');
        assert_eq!(cell.fg, palette::DB_STONE);
    }

    #[test]
    fn the_examine_cursor_blinks() {
        let mut sim = playing(6);
        sim.advance(Some(Command::ToggleExamine));
        let pos = sim.cursor().unwrap();
        let on = sim.view(0).cell(pos.x, pos.y).unwrap();
        let off = sim.view(BLINK_SPEED).cell(pos.x, pos.y).unwrap();
        assert_eq!(on.glyph, 'X');
        assert_eq!(on.fg, palette::CURSOR);
        assert_ne!(off.glyph, 'X');
    }

    #[test]
    fn examining_your_own_nucleus_describes_it() {
        let mut sim = playing(7);
        sim.advance(Some(Command::ToggleExamine));
        let view = sim.view(0);
        let examine = view.examine.expect("examine mode is open");
        let target = examine.target.expect("the cursor starts on a nucleus");
        assert_eq!(target.name, "Nucleus");
        assert!(target.detail.iter().any(|d| d.contains("Calcium")));
    }

    #[test]
    fn examining_the_dark_reports_nothing() {
        let mut sim = playing(8);
        sim.advance(Some(Command::ToggleExamine));
        for _ in 0..60 {
            sim.advance(Some(Command::MoveCursor(Dir::Up)));
        }
        let view = sim.view(0);
        assert!(view.examine.unwrap().target.is_none());
    }

    #[test]
    fn the_sidebar_hides_cytoplasm() {
        let sim = playing(9);
        let view = sim.view(0);
        assert_eq!(
            view.organelles.len(),
            2,
            "two nuclei, four hidden cytoplasm"
        );
        assert!(view.organelles.iter().all(|o| o.name == "Nucleus"));
    }

    #[test]
    fn the_sidebar_marks_the_nucleus_cycle() {
        let sim = playing(10);
        let view = sim.view(0);
        let hints: Vec<Option<char>> = view.organelles.iter().map(|o| o.nucleus_hint).collect();
        assert!(hints.contains(&Some('@')));
        // With exactly two nuclei the other one is both previous and next; the
        // previous marker wins.
        assert!(hints.contains(&Some('A')));
    }

    #[test]
    fn status_reports_the_win_condition() {
        let sim = playing(11);
        let view = sim.view(0);
        assert_eq!(view.status.mass, 6);
        assert_eq!(view.status.cities_remaining, 12);
        assert_eq!(view.status.cities_required, 8);
        assert_eq!(view.status.city_armor, 20, "the first gate is the cheapest");
        assert!((view.status.turn - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn the_highlight_is_the_drag_path() {
        let sim = playing(12);
        let view = sim.view(0);
        assert!(!view.highlight.is_empty());
        for pos in &view.highlight {
            let cell = view.cell(pos.x, pos.y).unwrap();
            assert_eq!(cell.bg, palette::PATH_SLIME);
        }
    }

    #[test]
    fn the_title_screen_renders_an_empty_map() {
        let sim = Sim::new(1, Difficulty::Normal);
        let view = sim.view(0);
        assert_eq!(view.phase, Phase::Title);
        assert!(view.cells.iter().all(|c| c.glyph == ' '));
        assert!(view.organelles.is_empty());
    }

    #[test]
    fn a_reticle_blinks_over_whatever_it_covers() {
        let mut sim = crate::sim::tests::sandbox(20);
        let hunter = sim.add_actor(Kind::Hunter, Coord::new(5, 5));
        sim.add_actor(Kind::Cytoplasm, Coord::new(7, 5));
        sim.add_actor(Kind::Nucleus, Coord::new(15, 15));
        sim.update_player_fov();
        sim.human_act(hunter);
        sim.human_act(hunter);
        let painted = Coord::new(7, 5);
        assert!(sim.reticle_at(painted));
        let on = sim.view(0).cell(painted.x, painted.y).unwrap();
        assert_eq!(on.glyph, 'X');
        assert_eq!(on.fg, palette::RETICLE_FOREGROUND);
        assert_eq!(on.bg, palette::RETICLE_BACKGROUND);
        // On the off beat the organelle underneath shows through again.
        let off = sim.view(BLINK_SPEED).cell(painted.x, painted.y).unwrap();
        assert_eq!(off.bg, palette::BODY_SLIME);
        assert!(sim.view(0).reticles.contains(&painted));
    }

    #[test]
    fn a_hunter_shows_an_arrow_while_it_is_aiming() {
        let mut sim = crate::sim::tests::sandbox(21);
        let hunter = sim.add_actor(Kind::Hunter, Coord::new(5, 5));
        sim.add_actor(Kind::Cytoplasm, Coord::new(3, 5));
        sim.add_actor(Kind::Nucleus, Coord::new(15, 15));
        sim.update_player_fov();
        // Light the hunter's cell by hand: the mass is deliberately far away so
        // that the only thing it can line up on is the cytoplasm to the west.
        sim.grid.append_fov(Coord::new(5, 5), 1, true);
        let at = |sim: &Sim, frame| {
            sim.view(frame)
                .cell(5, 5)
                .expect("the hunter is on the map")
                .glyph
        };
        assert_eq!(at(&sim, 0), 'h', "at rest it is a hunter");
        sim.human_act(hunter);
        sim.human_act(hunter);
        assert_eq!(at(&sim, 0), ARROW_LEFT, "aiming west");
        assert_eq!(at(&sim, BLINK_SPEED), 'h', "and the arrow blinks");
    }

    #[test]
    fn a_gate_flashes_its_countdown_and_then_its_queue() {
        let mut sim = crate::sim::tests::sandbox(22);
        sim.grid.set_props(Coord::new(6, 5), false, false, true);
        let city = sim.add_actor(Kind::City, Coord::new(6, 5));
        sim.update_player_fov();
        let glyph = |sim: &Sim, frame| sim.view(frame).cell(6, 5).expect("the gate").glyph;
        assert_eq!(glyph(&sim, 0), 'C', "far from a wave it is just a gate");
        assert_eq!(glyph(&sim, BLINK_SPEED), 'C');
        if let Extra::City(state) = &mut sim.actors[city].extra {
            state.turns_to_next_wave = 4;
        }
        assert_eq!(glyph(&sim, 0), 'C');
        assert_eq!(glyph(&sim, BLINK_SPEED), '4', "the countdown shows");
        if let Extra::City(state) = &mut sim.actors[city].extra {
            state.queue.push_back(Kind::Militia);
            state.queue.push_back(Kind::Tank);
        }
        assert_eq!(glyph(&sim, BLINK_SPEED), '2', "the queue wins over it");
        let cell = sim.view(BLINK_SPEED).cell(6, 5).unwrap();
        assert_eq!(cell.bg, palette::RETICLE_FOREGROUND);
    }

    #[test]
    fn an_armoured_human_shows_whether_it_is_about_to_act() {
        let mut sim = crate::sim::tests::sandbox(23);
        let tank = sim.add_actor(Kind::Tank, Coord::new(5, 5));
        sim.add_actor(Kind::Nucleus, Coord::new(15, 15));
        sim.update_player_fov();
        sim.grid.append_fov(Coord::new(5, 5), 1, true);
        let fg = |sim: &Sim| sim.view(0).cell(5, 5).expect("the tank").fg;
        // A tank is scheduled forty-eight units out: three turns of resting.
        assert_eq!(fg(&sim), palette::RESTING_TANK);
        sim.schedule.remove(tank);
        sim.schedule.add(tank, 16);
        assert_eq!(fg(&sim), palette::CALCIUM, "one turn out, it wakes up");
    }

    #[test]
    fn a_telegraph_is_only_drawn_for_a_human_you_can_see() {
        let mut sim = crate::sim::tests::sandbox(24);
        let near = sim.add_actor(Kind::Militia, Coord::new(6, 5));
        let far = sim.add_actor(Kind::Militia, Coord::new(17, 17));
        sim.add_actor(Kind::Nucleus, Coord::new(5, 5));
        sim.update_player_fov();
        sim.human_act(near);
        sim.human_act(far);
        let view = sim.view(0);
        assert_eq!(view.telegraphs.len(), 1, "only the one in sight");
        assert_eq!(view.telegraphs[0].from, Coord::new(6, 5));
        assert_eq!(view.telegraphs[0].to, Coord::new(5, 5));
        assert_eq!(view.telegraphs[0].kind, IntentKind::Strike);
    }

    #[test]
    fn views_are_a_pure_function_of_the_world_and_the_frame() {
        let sim = playing(13);
        assert_eq!(sim.view(0), sim.view(0));
        assert_eq!(sim.view(7), sim.view(7));
    }
}
