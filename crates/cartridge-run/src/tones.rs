//! What a cartridge plays through `env.tone`, as each engine's import
//! hears it and the contract keeps it, and how what a shelf's carts played
//! crosses, an update at a time, to the host that sounds it.

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

/// What one cart played in one update, in the order played. An update that
/// played nothing is one too: a player lays each update one after the
/// other, so the tones keep the time the game played them in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Played {
    /// The cart's name on its shelf.
    pub cart: u32,
    pub tones: Vec<Tone>,
}

/// Updates played, as plain words: how they cross from a worker that runs
/// the carts to the host that sounds them. Each update is its cart's
/// name, how many tones, then each tone's [`WORDS`].
pub fn played_words(played: &[Played]) -> Vec<u32> {
    let mut words = Vec::new();
    for update in played {
        words.extend([update.cart, update.tones.len() as u32]);
        words.extend(update.tones.iter().flat_map(Tone::words));
    }
    words
}

/// The updates in [`played_words`], up to the first that cannot be read: one
/// cut short or saying more tones than an update plays. A tone the contract
/// does not play is left out of its update.
pub fn played_from_words(words: &[u32]) -> Vec<Played> {
    let mut played = Vec::new();
    let mut rest = words;
    while let [cart, count, after @ ..] = rest {
        let count = *count as usize;
        if count > MAX_TONES || after.len() < count * WORDS {
            break;
        }
        let (tones, after) = after.split_at(count * WORDS);
        let tones = tones
            .chunks_exact(WORDS)
            .filter_map(|tone| {
                let &[channel, hz, ms, wave, to_hz, loud, to_loud] = tone else {
                    return None;
                };
                kept(channel, wave)
                    .then(|| Tone::new(channel as u8, hz, ms, wave as u8, to_hz, loud, to_loud))
            })
            .collect();
        played.push(Played { cart: *cart, tones });
        rest = after;
    }
    played
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
}

impl Tones {
    /// Keeps a tone the contract allows, up to [`MAX_TONES`] an update; a
    /// channel or a wave outside the contract's is dropped.
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
        if self.heard.len() < MAX_TONES && kept(channel, wave) {
            let tone = Tone::new(channel as u8, hz, ms, wave as u8, to_hz, loud, to_loud);
            self.heard.push(tone);
        }
    }

    /// Hands over what was heard, and starts again empty.
    pub fn take(&mut self) -> Vec<Tone> {
        std::mem::take(&mut self.heard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updates_cross_as_words_and_only_tones_come_back() {
        let tone = Tone::new(3, 880, 120, wave::SAW as u8, 220, 200, 0);
        let played = [
            Played {
                cart: 7,
                tones: vec![tone, Tone::steady(0, 0, 50, 0)],
            },
            Played {
                cart: 7,
                tones: vec![],
            },
            Played {
                cart: 9,
                tones: vec![tone],
            },
        ];
        let words = played_words(&played);
        assert_eq!(played_from_words(&words), played);
        let mut strange = words.clone();
        strange.extend([1, 2, CHANNELS, 880, 120, 0, 880, 255, 255]);
        strange.extend([0, 880, 120, wave::COUNT, 880, 255, 255]);
        let read = played_from_words(&strange);
        assert_eq!(read[..3], played);
        let left_out = Played {
            cart: 1,
            tones: vec![],
        };
        assert_eq!(read[3], left_out, "its tones left out");
        let mut cut = words.clone();
        cut.extend([1, 1, 0, 880, 120]);
        assert_eq!(played_from_words(&cut), played, "an update cut short");
        let mut many = words;
        many.extend([1, MAX_TONES as u32 + 1]);
        many.extend(vec![0; (MAX_TONES + 1) * WORDS]);
        assert_eq!(
            played_from_words(&many),
            played,
            "more than an update plays"
        );
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
