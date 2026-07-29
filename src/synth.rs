//! Procedural sound effects, as WAV bytes.
//!
//! Every voice is generated at startup, so the game ships no audio assets: the
//! wasm carries the arithmetic below instead of a megabyte of samples, and
//! there is nothing to credit in `CREDITS.md`.
//!
//! The palette is two families that never overlap. Anything the amoeba does is
//! wet: filtered noise whose cutoff sweeps down as something closes, or up as
//! something is drawn in, over a low body. Anything the humans do is dry and
//! sharp: short grains, fast decays, no tail. A player should be able to tell
//! which side of the fight just acted with the screen turned off.
//!
//! Three constraints shape what is here.
//!
//! macroquad's audio API offers volume and looping and nothing else — no pitch
//! control — so anything that should vary by pitch is baked into its own
//! buffer. [`step`] is the only voice that needs it, because it fires on every
//! move and a single buffer repeated eleven times a second reads as a machine.
//!
//! Every buffer leaves here normalised to the same peak, so the mix lives
//! entirely in the gain table over in `audio.rs`. One table to tune beats
//! twenty hand-balanced amplitudes, and it is what lets the amplitude test at
//! the bottom be one assertion for every voice.
//!
//! And on the web these bytes go through the browser's `decodeAudioData`,
//! which never reports failure back in a form macroquad surfaces, so a
//! malformed header hangs the loader on a black screen instead of erroring.
//! That is why the header is tested.

use std::f32::consts::{PI, TAU};

use fastrand::Rng;

/// Sample rate of every buffer.
///
/// quad-snd's native mixer runs at 44.1 kHz and nearest-neighbour resamples
/// anything else on load; matching it means that never happens.
pub const SAMPLE_RATE: u32 = 44_100;

/// How many buffers [`step`] bakes. Playback cycles through them.
pub const STEP_VARIANTS: usize = 3;

/// Peak every buffer is normalised to, leaving headroom for two voices to land
/// on the same sample without the mixer clipping.
const PEAK: f32 = 0.92;

/// Fade applied to the head and tail of every buffer, in seconds. A buffer
/// that starts or stops at a nonzero sample clicks.
const FADE: f32 = 0.004;

/// Fixed seed for the noise voices, so a given build always sounds the same.
const NOISE_SEED: u64 = 0x511E_5EED;

// -- the amoeba ---------------------------------------------------------------

/// Step: the mass flowing one cell.
///
/// A held arrow key produces one of these every ninety milliseconds, so this is
/// the one voice designed to sit just under notice — a grain of noise whose
/// cutoff collapses in a twentieth of a second, with barely any body.
///
/// `variant` picks between buffers that differ only in where that collapse
/// starts. Alternating between them is the only pitch variation macroquad
/// allows, and without it a walk sounds like a printer.
#[must_use]
pub fn step(variant: usize) -> Vec<u8> {
    const START: [f32; STEP_VARIANTS] = [820.0, 690.0, 960.0];
    let variant = variant % STEP_VARIANTS;
    let start = START[variant];
    let mut rng = Rng::with_seed(NOISE_SEED ^ (variant as u64 + 1));
    let mut squish = Svf::default();
    let mut body = Osc::default();
    wav(&one_shot(0.055, |t| {
        let (_, band) = squish.run(noise(&mut rng), start * decay(t, 26.0), 0.5);
        let thud = body.sine(120.0) * decay(t, 90.0) * 0.3;
        band.mul_add(decay(t, 62.0), thud)
    }))
}

/// Bump: an action refused — a wall, armour you cannot bite through, too little
/// mass to break a gate.
///
/// Deliberately the dullest thing in the file. It has no tail and no pitch
/// worth hearing, so it reads as "nothing happened" rather than as an event in
/// its own right.
#[must_use]
pub fn bump() -> Vec<u8> {
    let mut rng = Rng::with_seed(NOISE_SEED ^ 0x11);
    let mut dull = Svf::default();
    let mut body = Osc::default();
    wav(&one_shot(0.13, |t| {
        let low = body.sine(100.0_f32.mul_add(decay(t, 30.0), 76.0)) * decay(t, 26.0);
        let (thud, _) = dull.run(noise(&mut rng), 320.0, 1.4);
        thud.mul_add(decay(t, 70.0) * 0.8, low)
    }))
}

/// Swap: two organelles trading places. Two soft grains, the second higher than
/// the first, one for each thing that moved.
#[must_use]
pub fn swap() -> Vec<u8> {
    const SECOND: f32 = 0.06;
    let mut rng = Rng::with_seed(NOISE_SEED ^ 0x22);
    let mut here = Svf::default();
    let mut there = Svf::default();
    wav(&one_shot(0.16, |t| {
        let first = here.run(noise(&mut rng), 520.0, 0.5).1 * decay(t, 48.0);
        if t < SECOND {
            first
        } else {
            let grain = there.run(noise(&mut rng), 860.0, 0.5).1;
            grain.mul_add(decay(t - SECOND, 48.0), first)
        }
    }))
}

/// Eat: a human consumed by contact.
///
/// The satisfying one. A wet downward sweep — the cutoff falls as the membrane
/// closes — with a fast amplitude flutter over the first third that stands in
/// for teeth, and a low body underneath so it lands with some weight.
#[must_use]
pub fn eat() -> Vec<u8> {
    let mut rng = Rng::with_seed(NOISE_SEED ^ 0x33);
    let mut wet = Svf::default();
    let mut gut = Osc::default();
    wav(&one_shot(0.30, |t| {
        let cutoff = 1400.0_f32.mul_add(decay(t, 14.0), 220.0);
        let (_, band) = wet.run(noise(&mut rng), cutoff, 0.55);
        let chew = 0.35_f32.mul_add((TAU * 34.0 * t).sin() * decay(t, 12.0), 0.65);
        let low = gut.sine(88.0) * decay(t, 10.0) * 0.55;
        band.mul_add(decay(t, 11.0) * chew, low)
    }))
}

/// Ingest: a catalyst absorbed off the floor.
///
/// Sucked in rather than bitten, so the filter climbs where [`eat`]'s falls,
/// and it is capped with a small bright ping. Picking something up should never
/// be mistakable for eating something.
#[must_use]
pub fn ingest() -> Vec<u8> {
    const PING: f32 = 0.16;
    let mut rng = Rng::with_seed(NOISE_SEED ^ 0x44);
    let mut suck = Svf::default();
    let mut bell = Osc::default();
    wav(&one_shot(0.30, |t| {
        let cutoff = 1500.0_f32.mul_add(1.0 - decay(t, 9.0), 300.0);
        let slurp = suck.run(noise(&mut rng), cutoff, 0.6).1 * hit(t, 90.0, 9.0);
        if t < PING {
            slurp
        } else {
            bell.sine(1320.0)
                .mul_add(decay(t - PING, 26.0) * 0.45, slurp)
        }
    }))
}

/// Engulf: a sealed-in group of humans taken all at once.
///
/// The same gesture as [`eat`] made bigger and slower: three gulps over a swell
/// that rises through the whole half second, because the thing that just
/// happened took a whole ring of membrane to do.
#[must_use]
pub fn engulf() -> Vec<u8> {
    const SECS: f32 = 0.55;
    const GULPS: [f32; 3] = [0.0, 0.10, 0.22];
    let mut rng = Rng::with_seed(NOISE_SEED ^ 0x55);
    let mut wet = Svf::default();
    let mut swell = Osc::default();
    wav(&one_shot(SECS, |t| {
        let gulp: f32 = GULPS
            .iter()
            .filter(|start| t >= **start)
            .map(|start| decay(t - start, 13.0))
            .sum();
        let cutoff = 900.0_f32.mul_add(decay(t, 5.0), 200.0);
        let (_, band) = wet.run(noise(&mut rng), cutoff, 0.5);
        let rise = swell.sine(52.0_f32.mul_add(t / SECS, 46.0)) * hit(t, 12.0, 3.5) * 0.7;
        band.mul_add(gulp * 0.55, rise)
    }))
}

/// Digest: a dissolving human finishing and becoming something useful.
///
/// A gurgle. The cutoff wobbles instead of sweeping, which is the whole
/// difference between a bubble and a bite.
#[must_use]
pub fn digest() -> Vec<u8> {
    let mut rng = Rng::with_seed(NOISE_SEED ^ 0x66);
    let mut wet = Svf::default();
    let mut belly = Osc::default();
    wav(&one_shot(0.24, |t| {
        let wobble = 260.0_f32.mul_add((TAU * 17.0 * t).sin(), 480.0);
        let (_, band) = wet.run(noise(&mut rng), wobble, 0.5);
        let bubble = belly.sine(50.0_f32.mul_add((TAU * 11.0 * t).sin(), 190.0));
        band.mul_add(decay(t, 15.0), bubble * decay(t, 13.0) * 0.4)
    }))
}

/// Produce: a chloroplast or cultivator putting something new into the world.
///
/// One short rising bloop, kept small because a large mass produces every few
/// turns and this would otherwise become the sound of the game.
#[must_use]
pub fn produce() -> Vec<u8> {
    let mut fundamental = Osc::default();
    let mut harmonic = Osc::default();
    wav(&one_shot(0.16, |t| {
        let hz = 420.0_f32.mul_add(1.0 - decay(t, 22.0), 520.0);
        let tone = 0.3_f32.mul_add(harmonic.sine(hz * 2.0), fundamental.sine(hz));
        tone * hit(t, 300.0, 17.0)
    }))
}

/// Upgrade: an organelle finishing and becoming something better.
///
/// Three notes up a major triad, which is the smallest number of notes that can
/// say "good, and better than it was".
#[must_use]
pub fn upgrade() -> Vec<u8> {
    // D5, F#5, A5.
    const NOTES: [(f32, f32); 3] = [(587.33, 0.0), (739.99, 0.075), (880.0, 0.15)];
    wav(&one_shot(0.50, |t| {
        NOTES
            .iter()
            .map(|(hz, start)| note(t, *start, *hz, 8.0))
            .sum::<f32>()
            * 0.4
    }))
}

/// Nucleus lost.
///
/// The cue that has to cut through whatever else is happening, so it is the
/// only voice built out of two detuned oscillators: sirens beat against
/// themselves, and so does this, falling most of an octave over a rumble.
#[must_use]
pub fn nucleus_lost() -> Vec<u8> {
    let mut rng = Rng::with_seed(NOISE_SEED ^ 0x77);
    let mut alarm = Osc::default();
    let mut beat = Osc::default();
    let mut floor = Svf::default();
    wav(&one_shot(0.70, |t| {
        let hz = 520.0_f32.mul_add(decay(t, 4.2), 150.0);
        let pair = alarm.sine(hz) + beat.sine(hz * 1.014);
        let (rumble, _) = floor.run(noise(&mut rng), 180.0, 1.2);
        pair.mul_add(hit(t, 200.0, 3.4) * 0.5, rumble * decay(t, 5.0) * 0.6)
    }))
}

/// Organelle lost: something of yours other than a nucleus destroyed or shot
/// away. The same falling gesture as [`nucleus_lost`] at a fifth the size, so
/// the two are obviously related and never confusable.
#[must_use]
pub fn organelle_lost() -> Vec<u8> {
    let mut rng = Rng::with_seed(NOISE_SEED ^ 0x88);
    let mut fall = Osc::default();
    let mut puff = Svf::default();
    wav(&one_shot(0.20, |t| {
        let hz = 260.0_f32.mul_add(decay(t, 16.0), 165.0);
        let (air, _) = puff.run(noise(&mut rng), 400.0, 1.0);
        fall.sine(hz)
            .mul_add(decay(t, 14.0), air * decay(t, 40.0) * 0.5)
    }))
}

// -- the humans ---------------------------------------------------------------

/// Aim: a hunter or scout painting a line it will fire down next turn.
///
/// A dry tick with no tail at all. It is a warning, and a warning that rang
/// would smear over the shot that follows it.
#[must_use]
pub fn aim() -> Vec<u8> {
    let mut rng = Rng::with_seed(NOISE_SEED ^ 0x99);
    let mut tick = Svf::default();
    wav(&one_shot(0.05, |t| {
        tick.run(noise(&mut rng), 2400.0, 0.45).1 * decay(t, 130.0)
    }))
}

/// Shot: a ranged human firing. A crack — a noise burst and a fast downward zap
/// laid over each other, both gone inside a fifth of a second.
#[must_use]
pub fn shot() -> Vec<u8> {
    let mut rng = Rng::with_seed(NOISE_SEED ^ 0xAA);
    let mut crack = Svf::default();
    let mut zap = Osc::default();
    wav(&one_shot(0.20, |t| {
        let cutoff = 1700.0_f32.mul_add(decay(t, 30.0), 500.0);
        let (_, band) = crack.run(noise(&mut rng), cutoff, 0.4);
        let sweep = zap.sine(2200.0_f32.mul_add(decay(t, 60.0), 260.0)) * decay(t, 45.0) * 0.5;
        band.mul_add(decay(t, 26.0), sweep)
    }))
}

/// Terrify: a terror core taking an adjacent human's turn away.
///
/// Two tones a semitone apart — the interval everything unsettling is built
/// from — with a whistle sliding down out of nowhere over the top. This is the
/// amoeba doing something to a human, so it belongs to neither family and
/// sounds like neither.
#[must_use]
pub fn terrify() -> Vec<u8> {
    let mut low = Osc::default();
    let mut clash = Osc::default();
    let mut whistle = Osc::default();
    wav(&one_shot(0.45, |t| {
        // E4 against F4.
        let pair = clash.sine(349.23).mul_add(0.8, low.sine(329.63));
        let slide = whistle.sine(700.0_f32.mul_add(decay(t, 6.0), 620.0)) * decay(t, 9.0) * 0.35;
        pair.mul_add(hit(t, 26.0, 5.5) * 0.45, slide)
    }))
}

/// Wave spawned: a gate has queued another wave of humans.
///
/// A horn heard through rock. The slow attack, the vibrato and the lowpass over
/// the whole thing are all doing the same job — putting it somewhere else on
/// the map, so it reads as news rather than as something adjacent.
#[must_use]
pub fn wave_spawned() -> Vec<u8> {
    // Partials of D3: the stack that makes a stopped horn out of a test tone.
    const PARTIALS: [(f32, f32); 4] = [(1.0, 1.0), (1.5, 0.5), (2.0, 0.28), (3.0, 0.12)];
    let mut horn = [Osc::default(); PARTIALS.len()];
    let mut rock = Svf::default();
    wav(&one_shot(0.85, |t| {
        let hz = (TAU * 5.5 * t).sin().mul_add(3.0, 146.83);
        let stack: f32 = horn
            .iter_mut()
            .zip(PARTIALS)
            .map(|(osc, (ratio, gain))| osc.sine(hz * ratio) * gain)
            .sum();
        rock.run(stack, 620.0, 1.1).0 * hit(t, 9.0, 2.4) * 0.6
    }))
}

/// Gate countdown: a gate is fewer than ten turns from its next wave.
///
/// This fires turn after turn for as long as a countdown is running, so it is
/// the smallest thing in the file — one grain, over in a twentieth of a second,
/// and played quieter than anything else. It is a clock, not an event.
#[must_use]
pub fn gate_countdown() -> Vec<u8> {
    let mut rng = Rng::with_seed(NOISE_SEED ^ 0xBB);
    let mut tick = Svf::default();
    wav(&one_shot(0.045, |t| {
        tick.run(noise(&mut rng), 1500.0, 0.8).1 * decay(t, 150.0)
    }))
}

// -- the run ------------------------------------------------------------------

/// A gate came down. Rubble first, then the fifth that says it was worth it.
#[must_use]
pub fn city_destroyed() -> Vec<u8> {
    let mut rng = Rng::with_seed(NOISE_SEED ^ 0xCC);
    let mut rubble = Svf::default();
    let mut sub = Osc::default();
    wav(&one_shot(1.30, |t| {
        let (debris, _) = rubble.run(noise(&mut rng), 240.0, 1.5);
        let boom = debris.mul_add(hit(t, 60.0, 2.6), sub.sine(46.0) * hit(t, 40.0, 3.2) * 0.9);
        // D4 and its fifth, arriving once the dust is on its way down.
        let cheer = note(t, 0.34, 293.66, 2.2) + note(t, 0.50, 440.0, 2.0);
        cheer.mul_add(0.35, boom * 0.9)
    }))
}

/// Win: you reached the surface. Four notes up a D major triad with the last
/// one held, which is the smallest thing that is unmistakably a fanfare.
#[must_use]
pub fn win() -> Vec<u8> {
    // D4, F#4, A4, D5.
    const NOTES: [(f32, f32, f32); 4] = [
        (293.66, 0.00, 3.0),
        (369.99, 0.14, 3.0),
        (440.00, 0.28, 3.0),
        (587.33, 0.44, 1.4),
    ];
    wav(&one_shot(1.70, |t| {
        NOTES
            .iter()
            .map(|(hz, start, rate)| note(t, *start, *hz, *rate))
            .sum::<f32>()
            * 0.32
    }))
}

/// Lose: the last nucleus is gone. [`win`]'s four notes walked downwards
/// through a minor triad instead, over a drone that outlasts them.
#[must_use]
pub fn lose() -> Vec<u8> {
    // D4, Bb3, F3, D3.
    const NOTES: [(f32, f32, f32); 4] = [
        (293.66, 0.00, 1.8),
        (233.08, 0.38, 1.8),
        (174.61, 0.76, 1.6),
        (146.83, 1.14, 1.0),
    ];
    let mut drone = Osc::default();
    let mut detune = Osc::default();
    wav(&one_shot(2.00, |t| {
        let melody: f32 = NOTES
            .iter()
            .map(|(hz, start, rate)| note(t, *start, *hz, *rate))
            .sum();
        // 73.42 Hz against 73.95: one slow beat across the two seconds, which
        // is just enough to keep the low end from sounding like a test tone.
        let bed = (drone.sine(73.42) + detune.sine(73.95)) * hit(t, 6.0, 1.2);
        melody.mul_add(0.3, bed * 0.28)
    }))
}

// -- voices -------------------------------------------------------------------

/// A phase accumulator.
///
/// Sweeping a sine means integrating frequency: evaluating `sin(TAU * f(t) *
/// t)` with a moving `f` warps the phase and chirps instead of gliding.
#[derive(Clone, Copy, Default)]
struct Osc {
    phase: f32,
}

impl Osc {
    /// Advance one sample at `hz` and read the sine off the front.
    fn sine(&mut self, hz: f32) -> f32 {
        self.phase += TAU * hz / SAMPLE_RATE as f32;
        // Nothing here steps a whole cycle in one sample, so one subtraction
        // always suffices. Wrapping keeps the long buffers from losing angular
        // precision to a phase in the tens of thousands of radians.
        if self.phase >= TAU {
            self.phase -= TAU;
        }
        self.phase.sin()
    }
}

/// A Chamberlin state-variable filter: two multiply-accumulates a sample, and
/// the only thing in here with any resonance to it.
///
/// The resonance is the point. A one-pole lowpass takes the fizz off noise and
/// leaves it sounding like noise; this leaves a formant behind, which is what
/// turns a noise burst into something with a mouth.
#[derive(Clone, Copy, Default)]
struct Svf {
    low: f32,
    band: f32,
}

impl Svf {
    /// Run one sample through at cutoff `hz` and damping `damp`, which is the
    /// reciprocal of Q — smaller is more resonant. Returns the lowpass and
    /// bandpass taps.
    fn run(&mut self, input: f32, hz: f32, damp: f32) -> (f32, f32) {
        // The topology goes unstable as the cutoff approaches a quarter of the
        // sample rate. Nothing here asks for that; the clamp is free insurance
        // against a swept cutoff that someday does.
        let cutoff = hz.clamp(20.0, SAMPLE_RATE as f32 * 0.24);
        let f = 2.0 * (PI * cutoff / SAMPLE_RATE as f32).sin();
        self.low = f.mul_add(self.band, self.low);
        let high = damp.mul_add(-self.band, input - self.low);
        self.band = f.mul_add(high, self.band);
        (self.low, self.band)
    }
}

/// One white-noise sample, in `-1..=1`.
fn noise(rng: &mut Rng) -> f32 {
    rng.f32().mul_add(2.0, -1.0)
}

/// Exponential decay from one, at `rate` nepers a second.
fn decay(t: f32, rate: f32) -> f32 {
    (-t * rate).exp()
}

/// A percussive envelope: a rise fast enough to still read as an attack, then a
/// decay. The rise is what keeps a hard onset from sounding like a click.
fn hit(t: f32, attack: f32, rate: f32) -> f32 {
    let rise = 1.0 - decay(t, attack);
    rise * decay(t, rate)
}

/// One decaying note with a quiet second harmonic, silent until `start`.
///
/// Fixed pitch, so it can be evaluated straight from `t` rather than run
/// through an [`Osc`]; the melodic voices are all built out of these.
fn note(t: f32, start: f32, hz: f32, rate: f32) -> f32 {
    if t < start {
        return 0.0;
    }
    let age = t - start;
    let tone = 0.3_f32.mul_add((TAU * hz * 2.0 * age).sin(), (TAU * hz * age).sin());
    tone * decay(age, rate)
}

// -- buffers ------------------------------------------------------------------

/// Render `secs` of mono audio, normalise it, and fade both ends so it starts
/// and stops silently.
fn one_shot(secs: f32, voice: impl FnMut(f32) -> f32) -> Vec<f32> {
    let mut buffer = render(secs, voice);
    normalize(&mut buffer);
    fade_ends(&mut buffer);
    buffer
}

/// Render `secs` of mono audio verbatim.
fn render(secs: f32, mut voice: impl FnMut(f32) -> f32) -> Vec<f32> {
    (0..sample_count(secs))
        .map(|i| voice(i as f32 / SAMPLE_RATE as f32))
        .collect()
}

/// Scale a buffer so its loudest sample sits at [`PEAK`].
///
/// Every voice therefore leaves here at the same level, and the only thing
/// deciding how loud a step is next to a fanfare is the gain `audio.rs` plays
/// it at.
fn normalize(buffer: &mut [f32]) {
    let peak = buffer
        .iter()
        .fold(0.0_f32, |loudest, s| loudest.max(s.abs()));
    if peak <= f32::EPSILON {
        return;
    }
    let gain = PEAK / peak;
    for sample in buffer {
        *sample *= gain;
    }
}

/// Ramp the head and tail to silence, in place.
fn fade_ends(buffer: &mut [f32]) {
    let fade = sample_count(FADE);
    let len = buffer.len();
    for i in 0..fade.min(len / 2) {
        let gain = i as f32 / fade as f32;
        buffer[i] *= gain;
        buffer[len - 1 - i] *= gain;
    }
}

/// Seconds to a whole number of samples.
// Every duration fed to this is a positive literal in this file.
#[allow(clippy::cast_sign_loss)]
fn sample_count(secs: f32) -> usize {
    (secs * SAMPLE_RATE as f32) as usize
}

/// How long a buffer this module produced runs for, in seconds.
///
/// Read back out of the header rather than tracked alongside it, so a voice
/// whose length changes cannot leave a stale number behind. `audio.rs` uses it
/// to know when a voice has finished and its place in the mix is free again.
#[must_use]
pub fn duration_secs(wav: &[u8]) -> f32 {
    let Some(field) = wav.get(40..44) else {
        return 0.0;
    };
    let bytes = u32::from_le_bytes([field[0], field[1], field[2], field[3]]);
    bytes as f32 / (SAMPLE_RATE * 2) as f32
}

/// Wrap mono `f32` samples as a 16-bit PCM WAV.
///
/// Hand-rolled because the format's uncompressed case is a 44-byte header and
/// pulling a crate in for it would be sillier than writing it out.
fn wav(samples: &[f32]) -> Vec<u8> {
    let data_len = samples.len() * 2;
    let mut out = Vec::with_capacity(44 + data_len);

    // RIFF header: the size counts everything after this field, so the 44-byte
    // header minus the 8 bytes of "RIFF" and the size itself, plus the samples.
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");

    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16_u32.to_le_bytes()); // rest of this chunk
    out.extend_from_slice(&1_u16.to_le_bytes()); // uncompressed PCM
    out.extend_from_slice(&1_u16.to_le_bytes()); // mono
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // bytes per second
    out.extend_from_slice(&2_u16.to_le_bytes()); // bytes per frame
    out.extend_from_slice(&16_u16.to_le_bytes()); // bits per sample

    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data_len as u32).to_le_bytes());
    for sample in samples {
        // Clamp rather than let the cast wrap: a voice that overshoots should
        // sound squashed, not inverted.
        let scaled = sample.clamp(-1.0, 1.0) * f32::from(i16::MAX);
        out.extend_from_slice(&(scaled as i16).to_le_bytes());
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every voice this module can produce, by name.
    ///
    /// Anything added above and forgotten here escapes the header check, and a
    /// header the browser dislikes hangs the loader rather than failing, so
    /// this list is the one thing in the file worth keeping complete by hand.
    fn voices() -> Vec<(&'static str, Vec<u8>)> {
        let mut all = vec![
            ("bump", bump()),
            ("swap", swap()),
            ("eat", eat()),
            ("ingest", ingest()),
            ("engulf", engulf()),
            ("digest", digest()),
            ("produce", produce()),
            ("upgrade", upgrade()),
            ("nucleus lost", nucleus_lost()),
            ("organelle lost", organelle_lost()),
            ("aim", aim()),
            ("shot", shot()),
            ("terrify", terrify()),
            ("wave spawned", wave_spawned()),
            ("gate countdown", gate_countdown()),
            ("city destroyed", city_destroyed()),
            ("win", win()),
            ("lose", lose()),
        ];
        for variant in 0..STEP_VARIANTS {
            all.push(("step", step(variant)));
        }
        all
    }

    /// Pull the samples back out of a WAV the way a decoder would.
    fn decode(bytes: &[u8]) -> Vec<i16> {
        bytes[44..]
            .chunks_exact(2)
            .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
            .collect()
    }

    fn u32_at(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
    }

    #[test]
    fn every_header_is_a_well_formed_wav() {
        // A bad header does not fail loudly on the web -- decodeAudioData just
        // never calls back, and macroquad waits for it forever. Hence a test.
        for (name, bytes) in voices() {
            assert_eq!(&bytes[0..4], b"RIFF", "{name}");
            assert_eq!(&bytes[8..12], b"WAVE", "{name}");
            assert_eq!(&bytes[12..16], b"fmt ", "{name}");
            assert_eq!(&bytes[36..40], b"data", "{name}");

            assert_eq!(u32_at(&bytes, 16), 16, "{name} fmt chunk size");
            assert_eq!(bytes[20..22], 1_u16.to_le_bytes(), "{name} is not PCM");
            assert_eq!(bytes[22..24], 1_u16.to_le_bytes(), "{name} is not mono");
            assert_eq!(u32_at(&bytes, 24), SAMPLE_RATE, "{name} sample rate");
            assert_eq!(u32_at(&bytes, 28), SAMPLE_RATE * 2, "{name} byte rate");
            assert_eq!(bytes[32..34], 2_u16.to_le_bytes(), "{name} block align");
            assert_eq!(bytes[34..36], 16_u16.to_le_bytes(), "{name} bit depth");

            let data_len = u32_at(&bytes, 40) as usize;
            assert_eq!(data_len, bytes.len() - 44, "{name} data size");
            assert_eq!(
                u32_at(&bytes, 4) as usize,
                bytes.len() - 8,
                "{name} riff size"
            );
            assert_eq!(data_len % 2, 0, "{name} truncated 16-bit sample");
        }
    }

    #[test]
    fn every_voice_peaks_where_normalisation_put_it() {
        // The gain table in audio.rs is written against this: every buffer
        // arrives at the same level, so that table alone decides the mix.
        let want = f32::from(i16::MAX) * PEAK;
        for (name, bytes) in voices() {
            let samples = decode(&bytes);
            let peak = samples.iter().map(|s| i32::from(s.abs())).max().unwrap();
            assert!(
                peak as f32 > want * 0.5,
                "{name} peaks at {peak}, far below the {want:.0} it was normalised to"
            );
            assert!(
                peak as f32 <= want + 1.0,
                "{name} peaks at {peak}, above the {want:.0} it was normalised to"
            );
            // Clipping is clamped rather than wrapped, so a voice that ran too
            // hot would show up as a pile of samples pinned at the rail.
            let pinned = samples.iter().filter(|s| s.abs() > 32_700).count();
            assert_eq!(pinned, 0, "{name} clips for {pinned} samples");
        }
    }

    #[test]
    fn every_voice_carries_more_than_a_click() {
        // Peak alone says nothing once everything is normalised -- one stray
        // sample would pass it. Energy across the whole buffer is what says a
        // voice is actually there.
        for (name, bytes) in voices() {
            let samples = decode(&bytes);
            let energy: f64 = samples.iter().map(|s| f64::from(*s).powi(2)).sum();
            let rms = (energy / samples.len() as f64).sqrt();
            assert!(
                rms > 600.0,
                "{name} is nearly silent across its length (rms {rms:.0})"
            );
        }
    }

    #[test]
    fn every_voice_starts_and_ends_silently() {
        // Anything else is an audible click at each end.
        for (name, bytes) in voices() {
            let samples = decode(&bytes);
            assert_eq!(samples.first(), Some(&0), "{name} starts mid-waveform");
            assert_eq!(samples.last(), Some(&0), "{name} ends mid-waveform");
        }
    }

    #[test]
    fn durations_fit_the_pace_of_the_game() {
        for (name, bytes) in voices() {
            let secs = duration_secs(&bytes);
            let samples = (bytes.len() - 44) / 2;
            assert!(
                (secs - samples as f32 / SAMPLE_RATE as f32).abs() < 1e-6,
                "{name} reports {secs}s for {samples} samples"
            );
            // Long enough to be a sound; short enough that the two-second
            // post-mortem stings stay the longest things in the game.
            assert!((0.04..=2.0).contains(&secs), "{name} runs for {secs}s");
        }
    }

    #[test]
    fn the_move_sound_is_short_enough_to_repeat() {
        // A held arrow key repeats every 90 ms. A step that outlasted the gap
        // would pile up into a drone.
        for variant in 0..STEP_VARIANTS {
            let secs = duration_secs(&step(variant));
            assert!(secs < 0.09, "step {variant} runs for {secs}s");
        }
        let tick = duration_secs(&gate_countdown());
        assert!(tick < 0.09, "the countdown tick runs for {tick}s");
    }

    #[test]
    fn the_step_variants_are_actually_different() {
        // They exist only so that walking does not sound mechanical; identical
        // buffers would be three times the work for none of the benefit.
        for variant in 1..STEP_VARIANTS {
            assert_ne!(step(0), step(variant), "step {variant} matches step 0");
        }
        // And the cycle has to wrap the way playback indexes it.
        assert_eq!(step(0), step(STEP_VARIANTS));
    }

    #[test]
    fn generation_is_reproducible() {
        // The noise voices seed a fixed RNG; two builds must agree.
        assert_eq!(eat(), eat());
        assert_eq!(shot(), shot());
        assert_eq!(step(1), step(1));
    }

    #[test]
    fn a_truncated_buffer_reports_no_duration() {
        assert!(duration_secs(&[]).abs() < f32::EPSILON);
        assert!(duration_secs(b"RIFF").abs() < f32::EPSILON);
    }
}
