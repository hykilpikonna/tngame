#![feature(let_chains)]

use std::{env, io};
use std::io::Write;
use std::ops::DerefMut;
use std::string::ToString;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Error, Result};
use rand::Rng;
use termion::cursor::Goto;
use termion::raw::{IntoRawMode};
use tokio::io::{AsyncReadExt, AsyncWriteExt, stdin, stdout};
use tokio::sync::Mutex;

use crate::cowsay::gen_bubble_ascii;

mod cowsay;
mod utils;

const RESET: &str = "\x1b[0m";
const CLEAR: &str = "\x1b[2J";
const HIDE_CURSOR: &str = "\x1b[?25l";
const SHOW_CURSOR: &str = "\x1b[?25h";

/// Constants
const SNOW_DENSITY: f32 = 0.04; // Snow particles per pixel on screen
const SNOW_SPEED: f32 = 6.0; // Snow fall speed in pixels per second
const SNOW_X_RAND: f32 = 0.5; // Snow x velocity randomization factor

/// Colors: Convert them in python using hyfetch - print(repr(RGB.from_hex('#FFFFFF')))
const COLORS_STR: [&str; 3] = [
    // # FFFFFF
    "\x1b[38;2;246;170;183m",
    // # F6AAB7
    "\x1b[38;2;255;255;255m",
    // # 55CDFD
    "\x1b[38;2;85;205;253m"
];
const COLOR_CAT: &str = "\x1b[38;2;255;231;151m";
const COLOR_TREE: &str = "\x1b[38;2;204;255;88m";
const COLOR_HOUSE: &str = "\x1b[38;2;251;194;110m";
const COLOR_GRASS: &str = "\x1b[38;2;181;203;194m";
const GRASS_CHARS: [char; 3] = ['.', ',', ';'];

/// Snow particle struct
struct SnowParticle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    color: &'static str,
}

/// AsciiArt is a struct that holds the ascii art and the credit for the art.
#[derive(Clone, PartialEq, Eq)]
pub struct AsciiArt {
    art: String,
    h: i32,
    w: i32,
    credit: String,
}

impl AsciiArt {
    fn new(art: &str, credit: &str) -> Self {
        // Trim empty line breaks from the art and calculate the height and width
        let art = art.trim_matches('\n');
        let h = art.lines().count();
        let w = art.lines().map(|l| l.len()).max().unwrap_or(0);
        Self {
            art: art.to_string(),
            h: h as i32,
            w: w as i32,
            credit: credit.to_string(),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Pixel {
    color: &'static str,
    char: char,
}

fn snow_rand_velocity() -> (f32, f32) {
    let mut rng = rand::thread_rng();
    let vx = rng.gen_range(-SNOW_X_RAND..SNOW_X_RAND) * SNOW_SPEED;
    let vy = rng.gen_range(1.0..2.0) * SNOW_SPEED;
    (vx, vy)
}

fn create_snow(width: i32, height: i32) -> Vec<SnowParticle> {
    let count: u16 = ((width * height) as f32 * SNOW_DENSITY) as u16;
    let mut snow = Vec::with_capacity(count as usize);
    let mut rng = rand::thread_rng();
    for _ in 0..count {
        let x = rng.gen_range(0.0..width as f32);
        let y = rng.gen_range(0.0..height as f32);
        let (vx, vy) = snow_rand_velocity();
        let color = COLORS_STR[rng.gen_range(0..COLORS_STR.len())];
        snow.push(SnowParticle { x, y, vx, vy, color });
    }
    snow
}

struct Consts {
    asc_cat: AsciiArt,
    asc_tree: AsciiArt,
    asc_house: AsciiArt,
    asc_title: AsciiArt,
}

struct Mutes {
    w: i32,
    h: i32,
    x: i32,

    buf: Vec<Vec<Option<Pixel>>>,

    last_update: Instant,

    snow: Vec<SnowParticle>,
    should_exit: bool,
    state: State
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Welcome,
    Exploring
}

struct Main {
    mt: Mutes,
    cn: Consts,
}

impl Consts {
    fn new() -> Self {
        // Initialize the ascii art
        let asc_cat = AsciiArt::new(
            r#"
 /\_/\
( | | )
 >   < "#, "Azalea");
        let asc_tree = AsciiArt::new(
            r#"
          %%%,%%%%%%%
       ,'%% \\-*%%%%%%%
 ;%%%%%*%   _%%%%"
  ,%%%       \(_.*%%%%.
  % *%%, ,%%%%*(    '
%^     ,*%%% )\|,%%*%,_
     *%    \/ #).-"*%%*
         _.) ,/ *%,
          /)#(
         /   \ "#, "b'ger from ascii.co.uk/art/tree");
        let asc_house = AsciiArt::new(
            r#"
         _
     ,--l l--------,
    / /^/    /^/  / \
   /_.--.___.--._/   \
   | ,--,   ,--, |  ,|
 ,%| '--'._.'--' |,o%o
.*%|_,%%_| |_%%,_|#%%%*"#, "Modified from hjw from ascii.co.uk/art/house");
        let asc_title = AsciiArt::new(
            r#"
                 .       *
         _.__. _.| _  _. ' __
        (_] /_(_]|(/,(_]  _)
                                .  .
 __._  _ .    ,  .  .    , _ ._.| _|
_) [ )(_) \/\/ \_|   \/\/ (_)[  |(_]
               ._|                  "#, "Generated by patorjk.com/software/taag with font Contessa");
        Self {
            asc_cat,
            asc_tree,
            asc_house,
            asc_title,
        }
    }
}

impl Mutes {
    fn new(consts: &Consts) -> Self {
        // Get the terminal size
        let width: i32;
        let height: i32;
        if let Ok(size) = env::var("TN_TERM_SIZE") {
            // Environment variable in format "widthxheight"
            let mut split = size.split('x');
            width = split.next().unwrap().parse().unwrap();
            height = split.next().unwrap().parse().unwrap();
        }
        else {
            // Get the terminal size from the terminal
            let (w, h) = termion::terminal_size().unwrap();
            width = w as i32;
            height = h as i32;
            if env::var("TN_DEBUG").is_ok() {
                println!("Terminal size: {}x{}", width, height);
                // Press enter to continue
                let mut input = String::new();
                io::stdin().read_line(&mut input).unwrap();
            }
        }

        // Initialize the buffers
        let buf = vec![vec![None; width as usize]; height as usize];

        // Place cat x in the middle of the screen
        let x = (width - consts.asc_cat.w) / 2;

        // Create snow particles
        let snow = create_snow(width, height);

        Self {
            w: width,
            h: height, x,
            buf,
            last_update: Instant::now(),
            snow,
            should_exit: false,
            state: State::Welcome
        }
    }

    /// Update snow particles
    fn update_snow(&mut self, dt: f32) {
        let scroll = self.get_scroll();

        // Loop through all snow particles
        for p in &mut self.snow {
            // Update the snow particle position
            p.x += p.vx * dt;
            p.y += p.vy * dt;

            // If the snow particle is out of x bounds, wrap it around
            if p.x < 0.0 {
                p.x += self.w as f32;
            } else if p.x > self.w as f32 {
                p.x -= self.w as f32;
            }

            // If the snow particle is out of y bounds, reset it
            if p.y > self.h as f32 {
                let (vx, vy) = snow_rand_velocity();
                p.vx = vx;
                p.vy = vy;
                p.y = 0.0;
            }

            // Draw the snow particle in the buffer
            let x = p.x.round() as i32;
            let y = p.y.round() as i32;
            if x < self.w && y < self.h {
                self.buf[y as usize][(x + self.w - scroll / 2).rem_euclid(self.w) as usize] = Some(Pixel { color: p.color, char: '*' });
            }
        }
    }

    fn get_scroll(&self) -> i32 {
        return 0.max(self.x - (self.w * 3 / 4));
    }

    fn print_ascii(&mut self, art: &AsciiArt, x: i32, y: i32, color: &'static str) {
        let x = x - self.get_scroll();

        // If the ascii art is out of bounds, don't draw it
        if (x + art.w as i32) < 0 || x > self.w || (y + art.h as i32) < 0 || y > self.h {
            return;
        }

        // Loop through all lines in the ascii art
        for (i, line) in art.art.lines().enumerate() {
            let first_non_space = line.chars().position(|c| c != ' ').unwrap_or(0);
            // Loop through all characters in the line
            for (j, c) in line.chars().enumerate() {
                if j < first_non_space { continue; }
                // Draw the character in the buffer
                let x = x + j as i32;
                let y = y + i as i32;
                if 0 <= x && x < self.w as i32 && 0 <= y && y < self.h as i32 {
                    self.buf[y as usize][x as usize] = Some(Pixel { color, char: c });
                }
            }
        }
    }

    fn draw_grass(&mut self) {
        let scroll = self.get_scroll();

        // Choose a grass character for the grass based on pseudo-random number by hashing x
        for x in 0..self.w as i32 {
            // Get hash of x
            let mut hash = utils::hash((x + scroll) as u32);
            let c = GRASS_CHARS[(hash % GRASS_CHARS.len() as u32) as usize];

            self.buf[self.h as usize - 1][x as usize] = Some(Pixel { color: COLOR_GRASS, char: c });
        }
    }

    /// Draw the buffer to the screen, diffing it with the last buffer, and only drawing the changed pixels
    fn draw_buf(&mut self) -> Result<String> {
        // Create a buffer string
        let mut buf_str = String::with_capacity((self.w * self.h) as usize);

        // Keep the last color
        let mut last_color: &str = "";

        // Keep the current cursor
        let mut cursor = (0, 0);

        // No optimization method: clear the screen
        buf_str.push_str(&CLEAR);

        // Loop through all pixels in the buffer
        for y in 0..self.h as usize {
            for x in 0..self.w as usize {
                // Get the pixel
                let ppr = &mut self.buf[y][x];

                // If the current pixel isn't empty
                if let Some(p) = ppr {
                    if cursor != (x, y) {
                        if cursor.1 == y && x - cursor.0 < 8 {
                            // If the cursor is on the same line and with x distance less than 8, use spaces
                            for _ in 0..(x - cursor.0) {
                                buf_str.push(' ');
                            }
                        } else {
                            // Jump to the pixel position
                            buf_str.push_str(&Goto(x as u16 + 1, y as u16 + 1).to_string());
                        }
                    };
                    cursor = (x + 1, y);

                    if p.color != last_color {
                        // Set the color
                        buf_str.push_str(p.color);
                        last_color = p.color;
                    }

                    // Draw the pixel
                    buf_str.push(p.char);

                    // Clear the pixel
                    *ppr = None;
                }
            }
        }

        // Reset the color
        buf_str.push_str(RESET);

        Ok(buf_str)
    }
}

fn draw_ascii_frame(mt: &mut Mutes, cn: &Consts) {
    // Draw the tree
    let tree_1_start = (mt.w - 2 * cn.asc_tree.w) / 4;
    let tree_2_start = (mt.w + 2 * cn.asc_tree.w) / 2;
    mt.print_ascii(&cn.asc_tree, tree_1_start, mt.h - cn.asc_tree.h, COLOR_TREE);
    mt.print_ascii(&cn.asc_tree, tree_2_start, mt.h - cn.asc_tree.h, COLOR_TREE);

    // Draw the house
    let house_start = (mt.w + cn.asc_house.w) / 2;
    mt.print_ascii(&cn.asc_house, house_start, mt.h - cn.asc_house.h, COLOR_HOUSE);

    // Draw title at the center of the screen
    mt.print_ascii(&cn.asc_title, (mt.w - cn.asc_title.w) / 2, (mt.h - cn.asc_title.h) / 2, COLOR_CAT);

    // Draw the cat
    mt.print_ascii(&cn.asc_cat, mt.x, mt.h - cn.asc_cat.h, COLOR_CAT);

    if mt.state == State::Welcome {
        // Draw "Welcome to my snowy world" chat bubble
        let bubble = gen_bubble_ascii("Welcome to my\nsnowy world!");
        mt.print_ascii(&bubble, mt.x + 5, mt.h - cn.asc_cat.h - bubble.h, COLOR_CAT);
    }
    else {
        // Check position, if the cat is near the tree...
        if mt.x > tree_1_start && mt.x < tree_1_start + cn.asc_tree.w {
            // Draw the chat bubble
            let bubble = gen_bubble_ascii("I wish I could\nlive on that tree.");
            mt.print_ascii(&bubble, mt.x + 5, mt.h - cn.asc_cat.h - bubble.h, COLOR_CAT);
        }

        // Else: if the cat is near the house...
        else if mt.x > house_start - cn.asc_cat.w && mt.x < house_start + cn.asc_house.w {
            // Draw the chat bubble
            let bubble = gen_bubble_ascii("I wonder what\nmy friends are doing.");
            mt.print_ascii(&bubble, mt.x + 5, mt.h - cn.asc_cat.h - bubble.h, COLOR_CAT);
        }

        // Else: If the cat is at the edge...
        else if mt.x == 0 {
            // Draw the chat bubble
            let bubble = gen_bubble_ascii("The cliff, it looks so steep.\nI wish I can fly");
            mt.print_ascii(&bubble, mt.x + 5, mt.h - cn.asc_cat.h - bubble.h, COLOR_CAT);
        }
    }
}


async fn start_update_loop(mt: Arc<Mutex<Mutes>>, cn: &Consts) -> Result<()> {

    // Start the loop
    loop {
        // Get the current time
        let now = Instant::now();

        let mut txt: String;
        {
            let mut mt = mt.lock().await;
            if mt.should_exit { break; }

            // Calculate the delta time
            let dt = (now - mt.last_update).as_secs_f32();

            // Update scenes
            mt.last_update = now;
            mt.update_snow(dt);
            draw_ascii_frame(mt.deref_mut(), cn);

            // Draw the buffer, time it, and print it
            txt = mt.draw_buf().unwrap();
        }

        let end = Instant::now();

        let draw_time = (end - now).as_secs_f32();
        // Print draw time at 1, 1
        txt.push_str(&Goto(1, 1).to_string());
        txt.push_str(&*format!("\r{:.2}ms", draw_time * 1000.0));

        // Frame end with 3 Null bytes
        txt.push_str("\x00\x00\x00");
        stdout().write_all(txt.as_bytes()).await?;

        // Use tokio to sleep for 1/20th of a second
        tokio::time::sleep(Duration::from_millis(1000 / 20)).await;
    }

    Ok(())
}

async fn pull_input(mt: Arc<Mutex<Mutes>>, cn: &Consts) -> Result<()> {
    // Read keyboard input in a loop
    let mut stdin = stdin();
    let mut buf = [0; 3];
    loop {
        // Read a byte from stdin
        let n = stdin.read(&mut buf).await?;
        if n == 0 { break; }

        let str = String::from_utf8_lossy(&buf[..n]).to_string();

        {
            let mut mt = mt.lock().await;
            let mut move_x = |amount: i32| {
                mt.x = (mt.x + amount).max(0).min((mt.w - cn.asc_cat.w));
                if mt.state == State::Welcome {
                    mt.state = State::Exploring;
                }
            };

            // Switch on the key
            match str.as_str() {
                // exit on q or ctrl+c or esc
                "q" | "\x03" | "\x1b" => {
                    mt.should_exit = true;
                    break;
                },
                // Move left on a or left arrow
                "a" | "\x1b[D" => move_x(-1),
                // Move right on d or right arrow
                "d" | "\x1b[C" => move_x(1),
                _ => (),
            }
        }

        // Sleep for 1/100th of a second
        tokio::time::sleep(Duration::from_millis(1)).await;
    }

    Ok(())
}

fn run() -> Result<()> {
    pretty_env_logger::init();

    let cn: &Consts = Box::leak(Box::new(Consts::new()));
    let mt = Arc::new(Mutex::new(Mutes::new(&cn)));

    // Set terminal to raw mode
    let mut out = std::io::stdout();
    let raw = std::io::stdout().into_raw_mode();
    if raw.is_ok() {
        print!("Successfully set terminal to raw mode");
    }

    // Clear the screen
    out.write(CLEAR.as_ref())?;
    out.write(HIDE_CURSOR.as_ref())?;
    out.flush()?;


    // Start update_loop and pull_input concurrently and wait for them to finish
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let update_loop = start_update_loop( mt.clone(), cn);
        let pull_input = pull_input(mt.clone(), cn);
        tokio::try_join!(update_loop, pull_input)?;
        Ok::<(), Error>(())
    })?;

    // Reset the terminal
    out.write(SHOW_CURSOR.as_ref())?;
    out.write(CLEAR.as_ref())?;
    out.write("\r\nThanks for visiting <3\n".as_ref())?;
    out.flush()?;

    Ok(())
}

fn main() {
    run().expect("Error running program");
}
