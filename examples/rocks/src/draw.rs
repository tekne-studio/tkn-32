//! The picture: lines on space, each shape drawn again across an edge it
//! straddles, so the field wraps on the screen as it does in play.

use cartridge::{HEIGHT, Screen, WIDTH};

use crate::{BETWEEN, Body, Mode, OVER_WAIT, RADIUS, Rocks, SUB, colour, trig};

const SIDE: [i32; 2] = [WIDTH as i32, HEIGHT as i32];

/// The ship, nose up, in pixels: a dart with a notched tail.
const SHIP: [(i32, i32); 6] = [(0, -6), (3, 1), (5, 5), (0, 3), (-5, 5), (-3, 1)];
/// A life left, as the corner of the screen shows it.
const LIFE: [(i32, i32); 4] = [(0, -3), (2, 3), (0, 2), (-2, 3)];

/// The mark every first screen of ours carries, and the cover too.
pub(crate) const MARK: &str = "© 2026 TEKNE STUDIO";

impl Rocks {
    pub(crate) fn draw(&self, screen: &mut Screen) {
        self.draw_field(screen);
        match self.mode {
            Mode::Title => self.draw_title(screen),
            Mode::Play => self.draw_play(screen),
            Mode::Over => self.draw_over(screen),
            Mode::Demo => self.draw_demo(screen),
        }
    }

    /// Space, its stars and what flies in it.
    pub(crate) fn draw_field(&self, screen: &mut Screen) {
        screen.clear(colour::SPACE);
        for star in &self.stars {
            screen.pixel(star.x, star.y, star.colour);
        }
        for rock in self.rocks.iter().filter(|rock| rock.size > 0) {
            let radius = RADIUS[rock.size as usize] * SUB;
            let mut corners = [(0, 0); 9];
            for (k, corner) in corners.iter_mut().enumerate() {
                let angle = ((k * 256 / 9) as u8).wrapping_add(rock.angle);
                *corner = trig::along(angle, radius * i32::from(rock.corners[k]) / 100);
            }
            outline(
                screen,
                &rock.body,
                &corners,
                true,
                colour::ROCK[rock.size as usize],
            );
        }
        for shot in self.shots.iter().filter(|shot| shot.life > 0) {
            streak(screen, &shot.body, 1, colour::SHOT);
        }
        for spark in self.sparks.iter().filter(|spark| spark.life > 0) {
            streak(screen, &spark.body, 2, spark.colour);
        }
        let ship = &self.ship;
        if ship.alive && (ship.shield == 0 || self.tick / 4 % 2 == 0) {
            let turned = SHIP.map(|(x, y)| trig::turn(x * SUB, y * SUB, ship.angle));
            outline(screen, &ship.body, &turned, true, colour::SHIP);
            if ship.thrusting && self.tick % 4 < 2 {
                let tail = 8 + (self.tick % 3) as i32;
                let flame = [(-2, 4), (0, tail), (2, 4)]
                    .map(|(x, y)| trig::turn(x * SUB, y * SUB, ship.angle));
                outline(screen, &ship.body, &flame, false, colour::FLAME);
            }
        }
    }

    fn draw_title(&self, screen: &mut Screen) {
        draw_name(screen, 22);
        if self.tick / 30 % 2 == 0 {
            centred(screen, 70, "PRESS START", colour::TEXT, 1);
        }
        if self.best > 0 {
            centred(
                screen,
                82,
                Words::new().put("BEST ").number(self.best, 6).as_str(),
                colour::DIM,
                1,
            );
        }
        centred(
            screen,
            100,
            "<> TURN  ^ THRUST  A FIRE  B BRAKE",
            colour::DIM,
            1,
        );
        centred(screen, 111, MARK, colour::DIM, 1);
    }

    fn draw_play(&self, screen: &mut Screen) {
        self.draw_hud(screen);
        if self.between > 0 && self.between < BETWEEN {
            let words = Words::new().put("WAVE ").number(self.wave + 1, 1);
            centred(screen, 48, words.as_str(), colour::TEXT, 2);
        }
    }

    fn draw_demo(&self, screen: &mut Screen) {
        self.draw_play(screen);
        centred(screen, 100, "DEMO", colour::DIM, 1);
        if self.tick / 30 % 2 == 0 {
            centred(screen, 110, "PRESS START", colour::TEXT, 1);
        }
    }

    fn draw_over(&self, screen: &mut Screen) {
        self.draw_hud(screen);
        centred(screen, 44, "GAME OVER", colour::ALERT, 2);
        centred(
            screen,
            62,
            Words::new().put("SCORE ").number(self.score, 6).as_str(),
            colour::TEXT,
            1,
        );
        if self.tick >= OVER_WAIT && self.tick / 30 % 2 == 0 {
            centred(screen, 76, "PRESS START", colour::TEXT, 1);
        }
    }

    fn draw_hud(&self, screen: &mut Screen) {
        screen.text(
            3,
            3,
            Words::new().number(self.score, 6).as_str(),
            colour::HUD,
            1,
        );
        if self.wave > 0 {
            centred(
                screen,
                3,
                Words::new().put("WAVE ").number(self.wave, 1).as_str(),
                colour::DIM,
                1,
            );
        }
        for life in 0..self.lives as i32 {
            let (x, y) = (SIDE[0] - 6 - life * 7, 6);
            for (i, &(ax, ay)) in LIFE.iter().enumerate() {
                let (bx, by) = LIFE[(i + 1) % LIFE.len()];
                screen.line(x + ax, y + ay, x + bx, y + by, colour::SHIP);
            }
        }
    }
}

/// The game's name, large, its shadow under it, from `y` down.
pub(crate) fn draw_name(screen: &mut Screen, y: i32) {
    let x = (SIDE[0] - Screen::text_width("ROCKS", 5)) / 2;
    screen.text(x + 2, y + 2, "ROCKS", colour::GLOW, 5);
    screen.text(x, y, "ROCKS", colour::TITLE, 5);
}

/// Lines between `corners`, offsets in position units from `centre`,
/// joined back to the first when `closed`.
fn outline(screen: &mut Screen, centre: &Body, corners: &[(i32, i32)], closed: bool, colour: u8) {
    let reach = corners
        .iter()
        .map(|(x, y)| x.abs().max(y.abs()))
        .max()
        .unwrap_or(0)
        / SUB
        + 1;
    let (xs, nx) = copies(centre.x / SUB, reach, SIDE[0]);
    let (ys, ny) = copies(centre.y / SUB, reach, SIDE[1]);
    let lines = if closed {
        corners.len()
    } else {
        corners.len() - 1
    };
    for &ox in &xs[..nx] {
        for &oy in &ys[..ny] {
            for i in 0..lines {
                let (a, b) = (corners[i], corners[(i + 1) % corners.len()]);
                let (x0, y0) = ((centre.x + a.0) >> 8, (centre.y + a.1) >> 8);
                let (x1, y1) = ((centre.x + b.0) >> 8, (centre.y + b.1) >> 8);
                screen.line(x0 + ox, y0 + oy, x1 + ox, y1 + oy, colour);
            }
        }
    }
}

/// A moving point drawn with a tail: where it was `1 / part` of a tick ago.
fn streak(screen: &mut Screen, body: &Body, part: i32, colour: u8) {
    let (x, y) = (body.x >> 8, body.y >> 8);
    let (tx, ty) = (
        (body.x - body.vx / part) >> 8,
        (body.y - body.vy / part) >> 8,
    );
    screen.line(tx, ty, x, y, colour);
}

/// Where a shape `reach` pixels around `at` is drawn on a side of `side`:
/// in place, and once more across the edge it straddles.
fn copies(at: i32, reach: i32, side: i32) -> ([i32; 2], usize) {
    if at < reach {
        ([0, side], 2)
    } else if at >= side - reach {
        ([0, -side], 2)
    } else {
        ([0, 0], 1)
    }
}

pub(crate) fn centred(screen: &mut Screen, y: i32, text: &str, colour: u8, scale: i32) {
    screen.text(
        (SIDE[0] - Screen::text_width(text, scale)) / 2,
        y,
        text,
        colour,
        scale,
    );
}

/// A line of text put together with no allocator.
struct Words {
    bytes: [u8; 32],
    len: usize,
}

impl Words {
    fn new() -> Words {
        Words {
            bytes: [0; 32],
            len: 0,
        }
    }

    fn put(mut self, text: &str) -> Words {
        for &byte in text.as_bytes() {
            if self.len < self.bytes.len() {
                self.bytes[self.len] = byte;
                self.len += 1;
            }
        }
        self
    }

    /// `n` in decimal, padded with zeros to `width` digits.
    fn number(self, mut n: u32, width: usize) -> Words {
        let mut digits = [b'0'; 10];
        let mut count = 0;
        while n > 0 || count < width.max(1) {
            digits[9 - count] = b'0' + (n % 10) as u8;
            n /= 10;
            count += 1;
            if count == digits.len() {
                break;
            }
        }
        let text = core::str::from_utf8(&digits[10 - count..]).unwrap_or("");
        self.put(text)
    }

    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap_or("")
    }
}
