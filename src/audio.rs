//! Cues in, sound out. The audio half of the frontend.
//!
//! This is the counterpart to [`render`](crate::render): [`amoeba_rl::sim`]
//! says *what* happened and everything here decides what that sounds like. The
//! waveforms come from [`amoeba_rl::synth`], which stays macroquad-free so it
//! can be unit-tested; this file is the part that needs a live audio context.
//!
//! ## The autoplay problem
//!
//! Browsers start every page's audio context suspended and only resume it
//! inside a real user gesture. quad-snd's `audio.js` hooks mousedown, keydown
//! and touch to do the resuming, so a one-shot fired by the very key that
//! caused it already works. Nothing in this game loops, so there is no drone
//! to start mid-note either — but the info bar still says `press any key`
//! until a press has happened, because a game that is silent and a game that
//! is broken look identical.
//!
//! ## The machine-gun problem
//!
//! A held arrow key takes a turn every ninety milliseconds, and one turn can
//! report a dozen different cues. Two rules keep that from turning into a
//! blender. Every cue has a minimum gap it refuses to retrigger inside, which
//! is what stops a walk from becoming a buzz. And only [`MAX_VOICES`] sounds
//! may be in the air at once, with the loudest news getting first refusal, so
//! a busy turn drops the texture rather than the headline.
//!
//! Both rules are wall-clock and both live out here. The sim stays a pure
//! function of its inputs and never learns that any of this happened.

use amoeba_rl::sim::Cue;
use amoeba_rl::synth;
use macroquad::audio::{self, PlaySoundParams, Sound};
use macroquad::time::get_time;

use crate::input::Frame;

/// How many sounds may be in the air at once.
///
/// Four is enough for a turn where you eat something, lose an organelle, and
/// hear a gate wind up, and few enough that the mixer never turns a busy turn
/// into mud.
const MAX_VOICES: usize = 4;

/// The run ended. Exempt from [`MAX_VOICES`], because a fanfare you cannot
/// hear over four leftover footsteps is a bug.
const ENDING: u8 = 3;

/// Something happened that changes what you should do next.
const MAJOR: u8 = 2;

/// Something happened.
const MINOR: u8 = 1;

/// Something is still happening. The first thing dropped when a turn is busy.
const TEXTURE: u8 = 0;

/// Every cue, in the order [`slot`] indexes them and [`Audio::load`] bakes
/// them.
const CUES: [Cue; 20] = [
    Cue::Step,
    Cue::Bump,
    Cue::Swap,
    Cue::Eat,
    Cue::Ingest,
    Cue::Engulf,
    Cue::Digest,
    Cue::OrganelleLost,
    Cue::NucleusLost,
    Cue::Upgrade,
    Cue::Produce,
    Cue::Aim,
    Cue::Shot,
    Cue::Terrify,
    Cue::Depart,
    Cue::WaveSpawned,
    Cue::GateCountdown,
    Cue::CityDestroyed,
    Cue::Win,
    Cue::Lose,
];

/// Where a cue's voice sits in the table.
///
/// Exhaustive on purpose: a new [`Cue`] variant in the sim does not compile
/// until somebody here has decided what it sounds like.
const fn slot(cue: Cue) -> usize {
    match cue {
        Cue::Step => 0,
        Cue::Bump => 1,
        Cue::Swap => 2,
        Cue::Eat => 3,
        Cue::Ingest => 4,
        Cue::Engulf => 5,
        Cue::Digest => 6,
        Cue::OrganelleLost => 7,
        Cue::NucleusLost => 8,
        Cue::Upgrade => 9,
        Cue::Produce => 10,
        Cue::Aim => 11,
        Cue::Shot => 12,
        Cue::Terrify => 13,
        Cue::Depart => 14,
        Cue::WaveSpawned => 15,
        Cue::GateCountdown => 16,
        Cue::CityDestroyed => 17,
        Cue::Win => 18,
        Cue::Lose => 19,
    }
}

/// What one cue sounds like, how loud it is, and how often it may speak.
struct Design {
    /// The buffers to bake. More than one means alternate between them, which
    /// is the only pitch variation macroquad allows.
    wavs: Vec<Vec<u8>>,
    /// Playback gain, and so the whole mix: every buffer arrives from `synth`
    /// normalised to the same peak, and this is the only thing that changes
    /// how loud one cue is next to another.
    gain: f32,
    /// Shortest gap between two plays of this cue, in seconds.
    gap: f64,
    /// Who wins when a turn reports more than [`MAX_VOICES`] cues.
    rank: u8,
}

impl Design {
    /// The usual case: one buffer.
    fn single(wav: Vec<u8>, gain: f32, gap: f64, rank: u8) -> Self {
        Self {
            wavs: vec![wav],
            gain,
            gap,
            rank,
        }
    }
}

/// The sound design, in one table.
///
/// The gaps are the interesting column. A held arrow key repeats every ninety
/// milliseconds, so a gap above that decimates a cue during a run of moves
/// rather than muting it: `Step` sounds roughly every other pace, and
/// `GateCountdown` — which fires on every turn a gate is inside ten of its
/// wave — becomes an occasional tick under everything else instead of a
/// metronome. Cues that cannot physically repeat that fast get gaps just long
/// enough to swallow a double report.
fn design(cue: Cue) -> Design {
    match cue {
        // Wet, quiet, and decimated: this is the sound of the game moving.
        Cue::Step => Design {
            wavs: (0..synth::STEP_VARIANTS).map(synth::step).collect(),
            gain: 0.13,
            gap: 0.20,
            rank: TEXTURE,
        },
        Cue::Bump => Design::single(synth::bump(), 0.30, 0.18, MINOR),
        Cue::Swap => Design::single(synth::swap(), 0.22, 0.16, MINOR),
        Cue::Eat => Design::single(synth::eat(), 0.55, 0.10, MAJOR),
        Cue::Ingest => Design::single(synth::ingest(), 0.50, 0.10, MAJOR),
        Cue::Engulf => Design::single(synth::engulf(), 0.70, 0.30, MAJOR),
        Cue::Digest => Design::single(synth::digest(), 0.30, 0.12, MINOR),
        Cue::OrganelleLost => Design::single(synth::organelle_lost(), 0.40, 0.12, MINOR),
        Cue::NucleusLost => Design::single(synth::nucleus_lost(), 0.80, 0.35, MAJOR),
        Cue::Upgrade => Design::single(synth::upgrade(), 0.55, 0.25, MAJOR),
        // A large mass produces something every few turns; kept small and
        // rationed so a farm does not become the loudest thing on the map.
        Cue::Produce => Design::single(synth::produce(), 0.26, 0.30, MINOR),
        Cue::Aim => Design::single(synth::aim(), 0.22, 0.14, MINOR),
        Cue::Shot => Design::single(synth::shot(), 0.45, 0.09, MAJOR),
        Cue::Terrify => Design::single(synth::terrify(), 0.38, 0.30, MINOR),
        Cue::Depart => Design::single(synth::depart(), 0.35, 0.40, MINOR),
        Cue::WaveSpawned => Design::single(synth::wave_spawned(), 0.45, 0.60, MAJOR),
        Cue::GateCountdown => Design::single(synth::gate_countdown(), 0.10, 0.55, TEXTURE),
        Cue::CityDestroyed => Design::single(synth::city_destroyed(), 0.90, 1.00, MAJOR),
        Cue::Win => Design::single(synth::win(), 0.85, 2.00, ENDING),
        Cue::Lose => Design::single(synth::lose(), 0.85, 2.00, ENDING),
    }
}

/// One cue's loaded buffers and the state that rations them.
struct Voice {
    sounds: Vec<Sound>,
    gain: f32,
    gap: f64,
    rank: u8,
    /// How long one of these runs for, so the polyphony count knows when its
    /// place in the mix comes free.
    secs: f64,
    /// Which buffer plays next, for the cues that baked more than one.
    next: usize,
    /// When this last spoke, on the frontend's clock.
    last: f64,
}

impl Voice {
    /// Hand one design's buffers to the audio backend.
    async fn bake(design: Design) -> Self {
        let secs = design
            .wavs
            .first()
            .map_or(0.0, |wav| f64::from(synth::duration_secs(wav)));
        let mut sounds = Vec::with_capacity(design.wavs.len());
        for wav in &design.wavs {
            sounds.push(
                audio::load_sound_from_bytes(wav)
                    .await
                    .expect("synth output is a WAV the backend just accepted"),
            );
        }
        Self {
            sounds,
            gain: design.gain,
            gap: design.gap,
            rank: design.rank,
            secs,
            next: 0,
            // Everything is allowed to speak the first time it is asked to.
            last: f64::NEG_INFINITY,
        }
    }
}

/// The loaded sound bank plus the state that shapes playback.
pub struct Audio {
    /// One voice per cue, indexed by [`slot`].
    voices: Vec<Voice>,
    /// When each sound currently sounding is due to finish.
    airborne: Vec<f64>,
    muted: bool,
    /// Whether the player has pressed anything yet, which is what a browser
    /// waits for before it will make a noise at all.
    awake: bool,
}

impl Audio {
    /// Synthesise and decode the whole bank.
    ///
    /// Async because on the web each buffer goes to the browser to decode and
    /// macroquad waits a frame for each result. That is roughly a third of a
    /// second of startup for ten seconds of samples, and it happens once.
    pub async fn load() -> Self {
        let mut voices = Vec::with_capacity(CUES.len());
        for cue in CUES {
            voices.push(Voice::bake(design(cue)).await);
        }
        Self {
            voices,
            airborne: Vec::with_capacity(MAX_VOICES),
            muted: false,
            awake: false,
        }
    }

    /// One frame: handle the mute key, then play what the turn reported.
    pub fn update(&mut self, cues: &[Cue], frame: &Frame) {
        if frame.mute {
            self.muted = !self.muted;
        }
        // Only presses count. A browser will not resume its audio context for
        // anything less, and neither will we; every command in this game comes
        // from a key, a click or a touch, so a command is a gesture.
        self.awake |= frame.mute || frame.command.is_some();

        let now = get_time();
        self.airborne.retain(|ends| *ends > now);

        // Loudest band first, so a turn with more in it than the mix can carry
        // drops the footsteps rather than the news. It also means the run's
        // last turn keeps its cause: the gate comes down and the fanfare
        // starts over the top of the rubble, rather than replacing it.
        for rank in (TEXTURE..=ENDING).rev() {
            for &cue in cues {
                if self.voices[slot(cue)].rank == rank {
                    self.play(cue, now);
                }
            }
        }
    }

    /// Whether audio is still waiting on a first press, which is the one state
    /// worth nagging the player about.
    #[must_use]
    pub const fn needs_gesture(&self) -> bool {
        !self.awake
    }

    /// Whether sound is off. This is the only copy of that fact — the settings
    /// panel shows it and asks for it to be flipped rather than keeping a
    /// second one that could disagree.
    #[must_use]
    pub const fn muted(&self) -> bool {
        self.muted
    }

    /// HUD text for the current state.
    ///
    /// `touch` swaps the key this names for something a phone can do about it:
    /// there is no `M` to press, and the way to unmute is the settings panel
    /// the pad has a button for.
    #[must_use]
    pub const fn status(&self, touch: bool) -> &'static str {
        match (self.muted, self.awake, touch) {
            (true, _, false) => "audio muted - M",
            (true, _, true) => "audio muted - see menu",
            (false, true, false) => "audio on - M",
            (false, true, true) => "audio on - see menu",
            (false, false, false) => "audio: press any key",
            (false, false, true) => "audio: tap anything",
        }
    }

    /// The same text, but only when it is worth the room. Audio that is on and
    /// working has nothing to say, and the info bar has a game to describe.
    #[must_use]
    pub const fn hint(&self, touch: bool) -> Option<&'static str> {
        if self.muted || self.needs_gesture() {
            Some(self.status(touch))
        } else {
            None
        }
    }

    /// Fire one cue, if the mix has room for it and it has not just spoken.
    fn play(&mut self, cue: Cue, now: f64) {
        if self.muted {
            return;
        }
        let index = slot(cue);
        if self.voices[index].rank < ENDING && self.airborne.len() >= MAX_VOICES {
            return;
        }
        let ends = {
            let voice = &mut self.voices[index];
            if now - voice.last < voice.gap {
                return;
            }
            voice.last = now;
            let buffer = voice.next % voice.sounds.len();
            voice.next = voice.next.wrapping_add(1);
            audio::play_sound(
                &voice.sounds[buffer],
                PlaySoundParams {
                    looped: false,
                    volume: voice.gain,
                },
            );
            now + voice.secs
        };
        self.airborne.push(ends);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_cue_has_exactly_one_slot() {
        // `slot` is exhaustive over `Cue`, so the sim cannot grow a cue without
        // somebody landing in this file. This checks the other direction: that
        // the table `load` walks agrees with the indices `play` looks up.
        for (index, cue) in CUES.into_iter().enumerate() {
            assert_eq!(slot(cue), index, "{cue:?} is in the wrong place");
        }
    }

    #[test]
    fn what_the_sound_says_about_itself_names_no_key_on_a_phone() {
        // There is no `M` on a touchscreen, and "press any key" is an
        // instruction a thumb cannot follow.
        for muted in [false, true] {
            for awake in [false, true] {
                let audio = Audio {
                    voices: Vec::new(),
                    airborne: Vec::new(),
                    muted,
                    awake,
                };
                let line = audio.status(true);
                assert!(!line.contains(" M"), "{line:?} still names the mute key");
                assert!(!line.contains("key"), "{line:?} still names a key");
                // And the keyboard wording is left exactly as it was.
                assert!(audio.status(false).contains('M') || !awake);
            }
        }
    }

    #[test]
    fn the_mix_is_within_bounds() {
        for cue in CUES {
            let design = design(cue);
            assert!(!design.wavs.is_empty(), "{cue:?} has no waveform");
            assert!(
                (0.0..=1.0).contains(&design.gain),
                "{cue:?} plays at {}",
                design.gain
            );
            assert!(design.gap >= 0.0, "{cue:?} has a negative gap");
            for wav in &design.wavs {
                assert!(
                    synth::duration_secs(wav) > 0.0,
                    "{cue:?} has an empty buffer"
                );
            }
        }
    }

    #[test]
    fn the_cues_that_can_fire_every_turn_stay_out_of_the_way() {
        // Step and the gate countdown are the two cues a player can provoke on
        // turn after turn, and a held arrow key takes a turn every 90 ms. Both
        // have to be quiet enough to sit under an event, and rationed slowly
        // enough that a run of moves does not play every one of them.
        let event = design(Cue::Eat).gain;
        for cue in [Cue::Step, Cue::GateCountdown] {
            let quiet = design(cue);
            assert!(
                quiet.gain < event * 0.5,
                "{cue:?} is as loud as something happening"
            );
            assert!(quiet.gap > 0.09, "{cue:?} would sound on every key repeat");
            assert_eq!(quiet.rank, TEXTURE, "{cue:?} outranks the news");
        }
    }

    #[test]
    fn the_run_enders_outrank_everything() {
        // They are the only cues allowed past the polyphony cap, so nothing
        // else may claim the exemption.
        for cue in CUES {
            let ending = matches!(cue, Cue::Win | Cue::Lose);
            assert_eq!(design(cue).rank == ENDING, ending, "{cue:?}");
        }
    }
}
