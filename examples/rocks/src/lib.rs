//! Rocks, the first cartridge: a ship against drifting rocks. The ship
//! turns, thrusts with inertia and fires; a rock hit splits in two smaller
//! ones; the screen wraps. Lives, a score, a wave that grows; a title, play,
//! game over and again. A title left alone gives way to a demo, the game
//! flown by a pilot of its own, so a cabinet nobody plays still shows it.
//!
//! Every number is an integer: a position in 1/256 of a pixel, an angle in
//! 1/256 of a turn. The same seed and the same buttons give the same game
//! on every runner.

#![cfg_attr(target_arch = "wasm32", no_std)]

#[cfg(not(target_arch = "wasm32"))]
mod cover;
mod draw;
mod sound;
mod trig;

use cartridge::{Buttons, Game, Palette, Screen, button, wave};
use sound::Sound;

cartridge::cart!(Rocks);
cartridge::cover!();

/// A pixel in position units.
const SUB: i32 = 256;
/// The field, in position units: the screen, wrapped.
const FIELD: [i32; 2] = [
    cartridge::WIDTH as i32 * SUB,
    cartridge::HEIGHT as i32 * SUB,
];

/// A rock's radius in pixels, its speed in position units a tick and the
/// points it scores, by size: 1 small, 2 middle, 3 large.
const RADIUS: [i32; 4] = [0, 4, 8, 13];
const SPEED: [(i32, i32); 4] = [(0, 0), (150, 260), (100, 180), (55, 115)];
const POINTS: [u32; 4] = [0, 90, 40, 15];

/// The ship: turn a tick, thrust a tick, the drag that slows it (a part of
/// its speed a tick, held B the brake's), the fastest it goes on each axis.
const TURN: u8 = 4;
const THRUST: i32 = 7;
const DRAG: i32 = 48;
const BRAKE: i32 = 10;
const TOP_SPEED: i32 = 600;
/// A shot's speed, its life in ticks, the ticks between two.
const SHOT_SPEED: i32 = 700;
const SHOT_LIFE: u32 = 36;
const RELOAD: u32 = 8;

const LIVES: u32 = 3;
const MOST_LIVES: u32 = 6;
const LIFE_EVERY: u32 = 3000;
/// Ticks: the shield after the ship appears, before it comes back, between
/// two waves, before game over takes a start, before it returns to the title.
const SHIELD: u32 = 150;
const RESPAWN: u32 = 110;
const BETWEEN: u32 = 100;
const OVER_WAIT: u32 = 60;
const OVER_IDLE: u32 = 900;
/// Ticks: the title before the demo comes, the longest a demo plays.
const ATTRACT: u32 = 480;
const DEMO: u32 = 1800;
/// How far off its aim the demo's pilot fires, and thrusts, in 1/256 of a
/// turn; how near a rock it stops thrusting, in pixels, and how fast it
/// lets the ship go, in position units a tick on each axis.
const PILOT_FIRES: i8 = 6;
const PILOT_THRUSTS: i8 = 24;
const PILOT_KEEPS_OFF: i32 = 56;
const PILOT_SPEED: i32 = 160;
/// No rock is placed nearer the ship than this, in pixels.
const CLEARANCE: i32 = 48;

/// The palette's colours, by what they paint.
mod colour {
    pub const SPACE: u8 = 0;
    pub const FAINT_STAR: u8 = 1;
    pub const STAR: u8 = 2;
    /// A rock's colour by its size.
    pub const ROCK: [u8; 4] = [0, 5, 4, 3];
    pub const SHIP: u8 = 6;
    pub const FLAME: u8 = 7;
    pub const SHOT: u8 = 8;
    pub const TEXT: u8 = 9;
    pub const DIM: u8 = 10;
    pub const ALERT: u8 = 11;
    pub const HOT: u8 = 12;
    pub const TITLE: u8 = 13;
    pub const GLOW: u8 = 14;
    pub const HUD: u8 = 15;
}

#[rustfmt::skip]
const PALETTE: Palette = [
    [11, 14, 26, 255],    // space
    [42, 49, 80, 255],    // a faint star
    [108, 122, 168, 255], // a star
    [200, 184, 154, 255], // a large rock
    [224, 160, 96, 255],  // a middle rock
    [242, 210, 122, 255], // a small rock
    [127, 232, 224, 255], // the ship
    [255, 122, 60, 255],  // its flame
    [255, 255, 255, 255], // a shot
    [232, 236, 248, 255], // text
    [136, 144, 176, 255], // dim text
    [255, 77, 106, 255],  // alert
    [255, 192, 77, 255],  // sparks
    [156, 124, 255, 255], // the title
    [75, 58, 153, 255],   // its shadow
    [140, 255, 160, 255], // the score
    // 16 to 31: unused, black.
    [0, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255],
    [0, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255],
    [0, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255],
    [0, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255],
];

/// A position and a velocity, in position units.
#[derive(Clone, Copy, Default)]
struct Body {
    x: i32,
    y: i32,
    vx: i32,
    vy: i32,
}

impl Body {
    fn at(x: i32, y: i32) -> Body {
        Body { x, y, vx: 0, vy: 0 }
    }

    fn drift(&mut self) {
        self.x = (self.x + self.vx).rem_euclid(FIELD[0]);
        self.y = (self.y + self.vy).rem_euclid(FIELD[1]);
    }

    /// The square of the shortest distance to `other` across the wrap.
    fn gap2(&self, other: &Body) -> i64 {
        let dx = i64::from(wrap(self.x - other.x, FIELD[0]));
        let dy = i64::from(wrap(self.y - other.y, FIELD[1]));
        dx * dx + dy * dy
    }

    fn within(&self, other: &Body, pixels: i32) -> bool {
        let reach = i64::from(pixels * SUB);
        self.gap2(other) < reach * reach
    }
}

/// `d` as the shortest step across a span that wraps.
fn wrap(d: i32, span: i32) -> i32 {
    (d + span / 2).rem_euclid(span) - span / 2
}

#[derive(Clone, Copy, Default)]
struct Ship {
    body: Body,
    angle: u8,
    alive: bool,
    thrusting: bool,
    /// Ticks of the shield left: rocks pass through it.
    shield: u32,
    reload: u32,
}

#[derive(Clone, Copy, Default)]
struct Shot {
    body: Body,
    /// Ticks left; 0 is no shot.
    life: u32,
}

#[derive(Clone, Copy, Default)]
struct Rock {
    body: Body,
    /// 1 to 3; 0 is no rock.
    size: u8,
    angle: u8,
    spin: i8,
    /// Each corner's distance from the centre, in hundredths of the radius.
    corners: [u8; 9],
}

#[derive(Clone, Copy, Default)]
struct Spark {
    body: Body,
    life: u32,
    colour: u8,
}

#[derive(Clone, Copy, Default)]
struct Star {
    x: i32,
    y: i32,
    colour: u8,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Title,
    Play,
    Over,
    /// Play flown by the pilot, until a start, its last life or its time.
    Demo,
}

/// The only source of chance: a xorshift from the seed.
#[derive(Clone, Copy)]
struct Dice(u32);

impl Dice {
    fn new(seed: u32) -> Dice {
        Dice(match seed ^ 0x6d2b_79f5 {
            0 => 1,
            s => s,
        })
    }

    fn roll(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    /// A number in `lo..hi`.
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.roll() % (hi - lo).max(1) as u32) as i32
    }

    fn angle(&mut self) -> u8 {
        (self.roll() >> 24) as u8
    }
}

pub struct Rocks {
    dice: Dice,
    mode: Mode,
    /// Ticks since the mode began.
    tick: u32,
    before: Buttons,
    ship: Ship,
    shots: [Shot; 5],
    rocks: [Rock; 40],
    sparks: [Spark; 64],
    stars: [Star; 36],
    score: u32,
    best: u32,
    lives: u32,
    wave: u32,
    next_life: u32,
    /// Ticks until the next wave comes; 0 while one is on.
    between: u32,
    /// Ticks until the ship comes back.
    respawn: u32,
    sound: Sound,
}

impl Game for Rocks {
    fn start(seed: u32) -> Rocks {
        let mut dice = Dice::new(seed);
        let mut stars = [Star::default(); 36];
        for star in &mut stars {
            star.x = dice.range(0, cartridge::WIDTH as i32);
            star.y = dice.range(0, cartridge::HEIGHT as i32);
            star.colour = if dice.roll() % 4 == 0 {
                colour::STAR
            } else {
                colour::FAINT_STAR
            };
        }
        let mut game = Rocks {
            dice,
            mode: Mode::Title,
            tick: 0,
            before: Buttons::default(),
            ship: Ship::default(),
            shots: [Shot::default(); 5],
            rocks: [Rock::default(); 40],
            sparks: [Spark::default(); 64],
            stars,
            score: 0,
            best: 0,
            lives: 0,
            wave: 0,
            next_life: LIFE_EVERY,
            between: 0,
            respawn: 0,
            sound: Sound::default(),
        };
        game.title();
        game
    }

    fn update(&mut self, buttons: Buttons, screen: &mut Screen) {
        let pressed = buttons.pressed_since(self.before);
        self.before = buttons;
        let starts = pressed.held(button::START) || pressed.held(button::A);
        match self.mode {
            Mode::Title | Mode::Demo if starts => self.new_game(),
            Mode::Title if self.tick >= ATTRACT => self.begin(Mode::Demo),
            Mode::Over if starts && self.tick >= OVER_WAIT => self.new_game(),
            Mode::Over if self.tick >= OVER_IDLE => self.title(),
            Mode::Demo if self.tick >= DEMO || self.out_of_lives() => self.title(),
            Mode::Demo => self.play(self.pilot()),
            Mode::Play => self.play(buttons),
            _ => {}
        }
        self.drift();
        match self.mode {
            // A demo is silent: a cabinet left alone does not call out to
            // everyone who passes it. What it would play is dropped.
            Mode::Demo => self.sound = Sound::default(),
            _ => self.sound.play(),
        }
        self.tick += 1;
        self.draw(screen);
    }

    fn palette(&self) -> Palette {
        PALETTE
    }
}

impl Rocks {
    fn title(&mut self) {
        (self.mode, self.tick) = (Mode::Title, 0);
        self.clear_field();
        self.place_wave(5);
    }

    fn new_game(&mut self) {
        self.begin(Mode::Play);
        self.sound.tune(&[(523, 6), (659, 6), (784, 6), (1047, 12)]);
    }

    /// A game from its first wave, played or flown by the pilot.
    fn begin(&mut self, mode: Mode) {
        (self.mode, self.tick) = (mode, 0);
        self.clear_field();
        (self.score, self.lives, self.wave, self.next_life) = (0, LIVES, 0, LIFE_EVERY);
        (self.between, self.respawn) = (BETWEEN / 2, 0);
        self.launch();
    }

    /// The last ship is gone and no other comes.
    fn out_of_lives(&self) -> bool {
        !self.ship.alive && self.respawn == 0 && self.lives == 0
    }

    /// What the demo's pilot holds this tick: it turns to where the nearest
    /// rock will be when a shot reaches it, fires once it is aimed, and
    /// thrusts toward it while it is far and the ship slow, so the demo
    /// shows every move the game has. It knows only the field, the same on
    /// every runner, so it flies alike on all.
    fn pilot(&self) -> Buttons {
        let ship = &self.ship;
        let nearest = self
            .rocks
            .iter()
            .filter(|rock| rock.size > 0)
            .min_by_key(|rock| rock.body.gap2(&ship.body));
        let (Some(rock), true) = (nearest, ship.alive) else {
            return Buttons(0);
        };
        let dx = wrap(rock.body.x - ship.body.x, FIELD[0]);
        let dy = wrap(rock.body.y - ship.body.y, FIELD[1]);
        // A shot carries the ship's speed: the rock moves against it, for
        // as long as a shot takes to cover the distance, near enough.
        let (ax, ay) = (dx.abs(), dy.abs());
        let distance = ax.max(ay) + ax.min(ay) / 2;
        let lead = |v: i32| v * distance / SHOT_SPEED;
        let aim = trig::heading(
            dx + lead(rock.body.vx - ship.body.vx),
            dy + lead(rock.body.vy - ship.body.vy),
        );
        let off = aim.wrapping_sub(ship.angle) as i8;
        let mut held = 0;
        if off < -(TURN as i8) / 2 {
            held |= button::LEFT;
        } else if off > TURN as i8 / 2 {
            held |= button::RIGHT;
        }
        if off.unsigned_abs() <= PILOT_FIRES as u8 {
            held |= button::A;
        }
        let far = !rock
            .body
            .within(&ship.body, PILOT_KEEPS_OFF + RADIUS[rock.size as usize]);
        let slow = ship.body.vx.abs().max(ship.body.vy.abs()) < PILOT_SPEED;
        if far && slow && off.unsigned_abs() <= PILOT_THRUSTS as u8 {
            held |= button::UP;
        }
        Buttons(held)
    }

    fn game_over(&mut self) {
        (self.mode, self.tick) = (Mode::Over, 0);
        self.best = self.best.max(self.score);
        self.sound
            .tune(&[(440, 10), (370, 10), (311, 10), (220, 30)]);
    }

    fn clear_field(&mut self) {
        self.ship.alive = false;
        self.shots = [Shot::default(); 5];
        self.rocks = [Rock::default(); 40];
        self.sparks = [Spark::default(); 64];
    }

    /// The ship at the centre, still, shielded.
    fn launch(&mut self) {
        self.ship = Ship {
            body: Body::at(FIELD[0] / 2, FIELD[1] / 2),
            alive: true,
            shield: SHIELD,
            ..Ship::default()
        };
    }

    /// One tick of play: the ship, the shots, what they hit, the waves.
    fn play(&mut self, held: Buttons) {
        if self.ship.alive {
            self.steer(held);
        } else if self.respawn > 0 {
            self.respawn -= 1;
        } else if self.lives == 0 {
            self.game_over();
        } else if self
            .rocks
            .iter()
            .all(|rock| rock.size == 0 || !rock.body.within(&self.centre(), CLEARANCE / 2))
        {
            self.launch();
        }
        self.hit_rocks();
        self.crash();
        if self.between > 0 {
            self.between -= 1;
            if self.between == 0 {
                self.wave += 1;
                self.place_wave((2 + self.wave).min(9) as i32);
                self.sound
                    .sweep(sound::BLASTS, 220, 660, 20, wave::TRIANGLE);
            }
        } else if self.rocks.iter().all(|rock| rock.size == 0) {
            self.between = BETWEEN;
        }
    }

    fn centre(&self) -> Body {
        Body::at(FIELD[0] / 2, FIELD[1] / 2)
    }

    fn steer(&mut self, held: Buttons) {
        let ship = &mut self.ship;
        if held.held(button::LEFT) {
            ship.angle = ship.angle.wrapping_sub(TURN);
        }
        if held.held(button::RIGHT) {
            ship.angle = ship.angle.wrapping_add(TURN);
        }
        ship.thrusting = held.held(button::UP);
        if ship.thrusting {
            let (ax, ay) = trig::along(ship.angle, THRUST);
            (ship.body.vx, ship.body.vy) = (ship.body.vx + ax, ship.body.vy + ay);
            if self.tick % 4 == 0 {
                self.sound.sweep(
                    sound::ENGINE,
                    60 + (self.tick % 16) as i32 * 3,
                    50,
                    4,
                    wave::NOISE,
                );
            }
        }
        let drag = if held.held(button::B) { BRAKE } else { DRAG };
        for v in [&mut ship.body.vx, &mut ship.body.vy] {
            *v = (*v - *v / drag).clamp(-TOP_SPEED, TOP_SPEED);
            if v.abs() < 3 {
                *v = 0;
            }
        }
        ship.body.drift();
        ship.shield = ship.shield.saturating_sub(1);
        ship.reload = ship.reload.saturating_sub(1);
        if held.held(button::A) && ship.reload == 0 {
            self.fire();
        }
    }

    fn fire(&mut self) {
        let ship = self.ship;
        let Some(shot) = self.shots.iter_mut().find(|shot| shot.life == 0) else {
            return;
        };
        let (nx, ny) = trig::along(ship.angle, 6 * SUB);
        let (vx, vy) = trig::along(ship.angle, SHOT_SPEED);
        shot.body = Body {
            x: ship.body.x + nx,
            y: ship.body.y + ny,
            vx: ship.body.vx + vx,
            vy: ship.body.vy + vy,
        };
        shot.life = SHOT_LIFE;
        self.ship.reload = RELOAD;
        self.sound.sweep(sound::SHOTS, 1400, 500, 6, wave::SQUARE);
    }

    /// Everything that moves on its own: shots, rocks, sparks.
    fn drift(&mut self) {
        for shot in self.shots.iter_mut().filter(|shot| shot.life > 0) {
            shot.body.drift();
            shot.life -= 1;
        }
        for rock in self.rocks.iter_mut().filter(|rock| rock.size > 0) {
            rock.body.drift();
            rock.angle = rock.angle.wrapping_add_signed(rock.spin);
        }
        for spark in self.sparks.iter_mut().filter(|spark| spark.life > 0) {
            spark.body.drift();
            spark.life -= 1;
        }
    }

    fn hit_rocks(&mut self) {
        for s in 0..self.shots.len() {
            if self.shots[s].life == 0 {
                continue;
            }
            let shot = self.shots[s].body;
            let hit = self.rocks.iter().position(|rock| {
                rock.size > 0 && rock.body.within(&shot, RADIUS[rock.size as usize])
            });
            if let Some(r) = hit {
                self.shots[s].life = 0;
                self.split(r);
            }
        }
    }

    /// The ship against the rocks: a touch ends a life, and splits the rock.
    fn crash(&mut self) {
        if !self.ship.alive || self.ship.shield > 0 {
            return;
        }
        let ship = self.ship.body;
        let Some(r) = self.rocks.iter().position(|rock| {
            rock.size > 0 && rock.body.within(&ship, RADIUS[rock.size as usize] + 3)
        }) else {
            return;
        };
        self.ship.alive = false;
        self.lives -= 1;
        self.respawn = RESPAWN;
        self.burst(&ship, 24, colour::SHIP, 420);
        self.burst(&ship, 12, colour::HOT, 300);
        self.sound.sweep(sound::BLASTS, 320, 40, 45, wave::NOISE);
        self.split(r);
    }

    /// Rock `r` is hit: it scores, bursts, and splits in two smaller ones.
    fn split(&mut self, r: usize) {
        let rock = self.rocks[r];
        self.rocks[r].size = 0;
        let size = rock.size as usize;
        self.score += POINTS[size];
        if self.score >= self.next_life {
            self.next_life += LIFE_EVERY;
            if self.lives < MOST_LIVES {
                self.lives += 1;
                self.sound.tune(&[(880, 4), (1175, 4), (1760, 10)]);
            }
        }
        self.burst(&rock.body, 4 + 3 * size as u32, colour::ROCK[size], 260);
        let (from, to, ticks) = [(0, 0, 0), (420, 160, 8), (260, 90, 11), (160, 50, 15)][size];
        self.sound
            .sweep(sound::BLASTS, from, to, ticks, wave::NOISE);
        if size > 1 {
            let heading = self.dice.angle();
            let apart = heading.wrapping_add(96 + self.dice.range(0, 64) as u8);
            for angle in [heading, apart] {
                let mut child = rock.body;
                (child.vx, child.vy) = (child.vx / 2, child.vy / 2);
                self.place(size as u8 - 1, child, angle);
            }
        }
    }

    /// `count` large rocks spread along the field's edges, where it wraps,
    /// so they drift in from all sides, each clear of the ship.
    fn place_wave(&mut self, count: i32) {
        // The top edge and the left one: the field wraps, so they are the
        // bottom and the right as well.
        let rim = FIELD[0] + FIELD[1];
        let step = rim / count.max(1);
        let start = self.dice.range(0, rim);
        for k in 0..count {
            let ship = if self.ship.alive {
                self.ship.body
            } else {
                self.centre()
            };
            let mut at = Body::default();
            for _ in 0..16 {
                let along = (start + k * step + self.dice.range(0, step / 2)).rem_euclid(rim);
                at = if along < FIELD[0] {
                    Body::at(along, 0)
                } else {
                    Body::at(0, along - FIELD[0])
                };
                if !at.within(&ship, CLEARANCE) {
                    break;
                }
            }
            let angle = self.dice.angle();
            self.place(3, at, angle);
        }
    }

    /// A rock of `size` at `at`, adding its own speed along `angle` to the
    /// velocity it is given.
    fn place(&mut self, size: u8, mut at: Body, angle: u8) {
        let (lo, hi) = SPEED[size as usize];
        let bonus = self.wave.saturating_sub(1).min(8) as i32 * 9;
        let (vx, vy) = trig::along(angle, self.dice.range(lo, hi) + bonus);
        (at.vx, at.vy) = (at.vx + vx, at.vy + vy);
        let mut corners = [0; 9];
        for corner in &mut corners {
            *corner = self.dice.range(70, 101) as u8;
        }
        // A large rock turns slowly, a smaller one may turn twice as fast.
        let spin = match (size, self.dice.roll() % 4) {
            (3, n) => [-1, -1, 1, 1][n as usize],
            (_, n) => [-2, -1, 1, 2][n as usize],
        };
        let rock = Rock {
            body: at,
            size,
            angle: self.dice.angle(),
            spin,
            corners,
        };
        if let Some(free) = self.rocks.iter_mut().find(|rock| rock.size == 0) {
            *free = rock;
        }
    }

    /// `count` sparks flying from `at` in `colour`, at most `speed`.
    fn burst(&mut self, at: &Body, count: u32, colour: u8, speed: i32) {
        for _ in 0..count {
            let angle = self.dice.angle();
            let (vx, vy) = trig::along(angle, self.dice.range(speed / 4, speed));
            let life = self.dice.range(12, 34) as u32;
            if let Some(spark) = self.sparks.iter_mut().find(|spark| spark.life == 0) {
                *spark = Spark {
                    body: Body {
                        x: at.x,
                        y: at.y,
                        vx: at.vx / 2 + vx,
                        vy: at.vy / 2 + vy,
                    },
                    life,
                    colour,
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(game: &mut Rocks, screen: &mut Screen, ticks: u32, buttons: u32) {
        for _ in 0..ticks {
            game.update(Buttons(buttons), screen);
        }
    }

    fn rocks(game: &Rocks) -> [usize; 4] {
        let mut count = [0; 4];
        for rock in &game.rocks {
            count[rock.size as usize] += 1;
        }
        count
    }

    #[test]
    fn a_rock_hit_splits_in_two_smaller_ones() {
        let mut game = Rocks::start(7);
        let mut screen = Screen::new();
        run(&mut game, &mut screen, 1, button::START);
        run(&mut game, &mut screen, BETWEEN, 0);
        let before = rocks(&game);
        assert_eq!(before[3], 3, "the first wave is three large rocks");
        let large = game.rocks.iter().position(|rock| rock.size == 3).unwrap();
        game.split(large);
        let after = rocks(&game);
        assert_eq!((after[3], after[2]), (2, 2));
        assert_eq!(game.score, POINTS[3]);
    }

    #[test]
    fn a_title_left_alone_gives_way_to_a_demo_that_plays_and_ends() {
        let mut game = Rocks::start(7);
        let mut screen = Screen::new();
        run(&mut game, &mut screen, ATTRACT, 0);
        assert!(game.mode == Mode::Title, "the title holds its time");
        run(&mut game, &mut screen, 1, 0);
        assert!(game.mode == Mode::Demo);
        let mut scored = 0;
        let mut demo = 1;
        while game.mode == Mode::Demo {
            run(&mut game, &mut screen, 1, 0);
            scored = scored.max(game.score);
            demo += 1;
        }
        assert!(scored > 0, "the pilot hits rocks");
        assert!(demo <= DEMO + 1, "a demo ends in its time");
        assert!(
            game.mode == Mode::Title && game.best == 0,
            "a demo's score is no best"
        );
    }

    #[test]
    fn a_start_in_the_demo_begins_a_game() {
        let mut game = Rocks::start(7);
        let mut screen = Screen::new();
        run(&mut game, &mut screen, ATTRACT + 120, 0);
        assert!(game.mode == Mode::Demo);
        run(&mut game, &mut screen, 1, button::START);
        assert!(game.mode == Mode::Play && game.score == 0 && game.lives == LIVES);
    }

    #[test]
    fn the_same_seed_and_buttons_give_the_same_screens() {
        let script = |tick: u32| match tick {
            0 => button::START,
            60..=90 => button::UP | button::A,
            91..=140 => button::LEFT | button::A,
            _ => button::A,
        };
        let hash = |seed| {
            let (mut game, mut screen) = (Rocks::start(seed), Screen::new());
            let mut hash = 0xcbf2_9ce4_8422_2325_u64;
            for tick in 0..600 {
                game.update(Buttons(script(tick)), &mut screen);
                for &byte in screen.pixels() {
                    hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
                }
            }
            hash
        };
        assert_eq!(hash(1), hash(1));
        assert_ne!(hash(1), hash(2));
    }
}
