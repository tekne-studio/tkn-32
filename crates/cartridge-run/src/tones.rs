//! What a cartridge plays through `env.tone`, as each engine's import
//! hears it and the contract keeps it, and the message that carries what a
//! cart played, an update at a time, to the chip that sounds it.

use cartridge::{CHANNELS, LOUDEST, MAX_TONES, wave};

/// One tone a cartridge played during an update, in `env.tone`'s words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tone {
    pub channel: u8,
    /// 0 silences the channel.
    pub hz: u32,
    pub ms: u32,
    /// One of [`cartridge::wave`].
    pub wave: u8,
    /// Where the pitch ends.
    pub to_hz: u32,
    /// The loudness at its start and at its end, out of [`LOUDEST`].
    pub loud: u8,
    pub to_loud: u8,
}

impl Tone {
    /// A tone as `env.tone` hears it: its loudness held to [`LOUDEST`].
    pub fn new(
        channel: u8,
        hz: u32,
        ms: u32,
        wave: u8,
        to_hz: u32,
        loud: u32,
        to_loud: u32,
    ) -> Tone {
        let held = |loud: u32| loud.min(LOUDEST) as u8;
        Tone {
            channel,
            hz,
            ms,
            wave,
            to_hz,
            loud: held(loud),
            to_loud: held(to_loud),
        }
    }

    /// A tone that stays at its pitch, at the loudest: `cartridge::tone`'s.
    pub fn steady(channel: u8, hz: u32, ms: u32, wave: u8) -> Tone {
        Tone::new(channel, hz, ms, wave, hz, LOUDEST, LOUDEST)
    }

    /// Its words, in `env.tone`'s order.
    pub fn words(&self) -> [u32; WORDS] {
        [
            u32::from(self.channel),
            self.hz,
            self.ms,
            u32::from(self.wave),
            self.to_hz,
            u32::from(self.loud),
            u32::from(self.to_loud),
        ]
    }
}

/// How many words a tone is: `env.tone`'s seven.
pub const WORDS: usize = 7;

/// What a cart played over some updates, as one message to the arcade's
/// chip (`chip-worklet`'s processor): how many updates a second to lay them
/// at, 0 to play them all at once, then each update's count of tones and
/// their [`WORDS`]. An update that played nothing is one too, so the tones
/// keep the time the game played them in.
pub fn batch<'a>(per_second: u32, updates: impl IntoIterator<Item = &'a [Tone]>) -> Vec<u32> {
    let mut words = vec![per_second];
    for tones in updates {
        words.push(tones.len() as u32);
        words.extend(tones.iter().flat_map(Tone::words));
    }
    words
}

/// Whether a tone on `channel` in `wave` is one the contract plays.
fn kept(channel: u32, wave: u32) -> bool {
    channel < CHANNELS && wave < wave::COUNT
}

/// The tones heard during one update. An engine's `env.tone` hands each
/// call here, whatever it holds.
#[derive(Debug, Default)]
pub struct Tones {
    heard: Vec<Tone>,
    /// The calls heard: the first [`MAX_TONES`] count, kept or dropped, so
    /// a host that hears a game apart from the runner hands over that many
    /// calls an update and keeps no rule of its own.
    calls: usize,
}

impl Tones {
    /// Keeps a tone the contract allows among the first [`MAX_TONES`] calls
    /// of an update; a call past them, or of a channel or a wave outside the
    /// contract's, is dropped.
    #[allow(clippy::too_many_arguments)]
    pub fn hear(
        &mut self,
        channel: u32,
        hz: u32,
        ms: u32,
        wave: u32,
        to_hz: u32,
        loud: u32,
        to_loud: u32,
    ) {
        if self.calls == MAX_TONES {
            return;
        }
        self.calls += 1;
        if kept(channel, wave) {
            let tone = Tone::new(channel as u8, hz, ms, wave as u8, to_hz, loud, to_loud);
            self.heard.push(tone);
        }
    }

    /// Hands over what was heard, and starts again empty.
    pub fn take(&mut self) -> Vec<Tone> {
        self.calls = 0;
        std::mem::take(&mut self.heard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_batch_lays_each_update_with_its_count_of_tones() {
        let boom = Tone::new(3, 880, 120, wave::SAW as u8, 220, 200, 0);
        let hush = Tone::steady(0, 0, 50, 0);
        let updates: [&[Tone]; 3] = [&[boom, hush], &[], &[boom]];
        let mut words = vec![60, 2];
        words.extend(boom.words());
        words.extend(hush.words());
        words.extend([0, 1]);
        words.extend(boom.words());
        assert_eq!(batch(60, updates), words);
        assert_eq!(batch(0, []), [0], "nothing to lay");
    }

    #[test]
    fn a_loudness_past_the_loudest_is_held_to_it() {
        let mut tones = Tones::default();
        tones.hear(2, 160, 250, wave::SQUARE, 45, 9_000, 0);
        let [boom] = tones.take()[..] else {
            panic!("one tone heard");
        };
        assert_eq!((boom.loud, boom.to_loud), (255, 0));
    }
}
