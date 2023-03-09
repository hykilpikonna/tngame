#![feature(let_chains)]

use std::{mem};
use std::io::Write;
use std::ops::DerefMut;
use std::process::exit;
use std::string::ToString;
use std::sync::{Arc};
use std::time::{Duration, Instant};

use anyhow::{Error, Result};
use rand::Rng;
use termion::cursor::Goto;
use termion::raw::{IntoRawMode, RawTerminal};
use tokio::io::{AsyncReadExt, stdin, stdout, AsyncWriteExt};
use tokio::sync::Mutex;
use crate::cowsay::gen_bubble_ascii;

mod cowsay;

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
    h: u16,
    w: u16,
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
            h: h as u16,
            w: w as u16,
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

fn create_snow(width: u16, height: u16) -> Vec<SnowParticle> {
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
    w: u16,
    h: u16,
    x: u16,

    buf: Vec<Vec<Option<Pixel>>>,
    last_buf: Vec<Vec<Option<Pixel>>>,

    last_update: Instant,

    snow: Vec<SnowParticle>,
    should_exit: bool,
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
                    .
 __._  _ .    ,  .  |_  _ ._ _  _
_) [ )(_) \/\/ \_|  [ )(_)[ | )(/,
               ._|                "#, "Generated by patorjk.com/software/taag");
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
        let (width, height) = termion::terminal_size().unwrap();

        // Initialize the buffers
        let buf = vec![vec![None; width as usize]; height as usize];
        let last_buf = buf.clone();

        // Place cat x in the middle of the screen
        let x = (width - consts.asc_cat.w) / 2;

        // Create snow particles
        let snow = create_snow(width, height);

        Self {
            w: width,
            h: height, x,
            buf, last_buf,
            last_update: Instant::now(),
            snow,
            should_exit: false,
        }
    }

    /// Update snow particles
    fn update_snow(&mut self, dt: f32) {
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
            let x = p.x.round() as u16;
            let y = p.y.round() as u16;
            if x < self.w && y < self.h {
                self.buf[y as usize][x as usize] = Some(Pixel { color: p.color, char: '*' });
            }
        }
    }

    fn print_ascii(&mut self, art: &AsciiArt, x: u16, y: u16, color: &'static str) {
        // Loop through all lines in the ascii art
        for (i, line) in art.art.lines().enumerate() {
            // Loop through all characters in the line
            for (j, c) in line.chars().enumerate() {
                // Draw the character in the buffer
                let x = x + j as u16;
                let y = y + i as u16;
                if x < self.w && y < self.h {
                    self.buf[y as usize][x as usize] = Some(Pixel { color, char: c });
                }
            }
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
        let mut ensure_cursor = |x: u16, y: u16, a: u16, buf_str: &mut String|
            if cursor != (x, y) {
                // Go to the pixel position
                buf_str.push_str(&Goto(x + 1, y + 1).to_string());
                cursor = (x + a, y);
            };

        // Loop through all pixels in the buffer
        for y in 0..self.h {
            for x in 0..self.w {
                // Get the pixel
                let p = &self.buf[y as usize][x as usize];

                // Get the last pixel
                let last_p = &self.last_buf[y as usize][x as usize];

                // If color changed and isn't the same as last color, update the color prefix
                if let Some(p) = p && (last_p.is_none() || p.color != last_p.as_ref().unwrap().color) && p.color != last_color {
                    ensure_cursor(x, y, 0, &mut buf_str);
                    // Set the color
                    buf_str.push_str(p.color);
                    last_color = p.color;
                }

                // If the char changed, update the char
                if let Some(p) = p && (last_p.is_none() || p.char != last_p.as_ref().unwrap().char) {
                    ensure_cursor(x, y, 1, &mut buf_str);
                    // Set the char
                    buf_str.push(p.char);
                }

                // If the pixel is empty but the last pixel wasn't, clear the pixel
                if last_p.is_some() {
                    if p.is_none() {
                        ensure_cursor(x, y, 1, &mut buf_str);
                        // Clear the pixel
                        buf_str.push(' ');
                    }

                    // Clear the last pixel
                    self.last_buf[y as usize][x as usize] = None;
                }
            }
        }

        // Since last_buf is cleared, we can swap it with buf
        mem::swap(&mut self.buf, &mut self.last_buf);

        // Reset the color
        buf_str.push_str(RESET);

        // Flush the buffer
        buf_str.push_str(&Goto(1, self.h as u16 + 1).to_string());

        Ok(buf_str)
    }
}

fn draw_ascii_frame(mt: &mut Mutes, cn: &Consts) {
    // Draw the tree
    mt.print_ascii(&cn.asc_tree, (mt.w - 2 * cn.asc_tree.w) / 4, mt.h - cn.asc_tree.h,
                   "\x1b[38;2;204;255;88m");
    mt.print_ascii(&cn.asc_tree, (mt.w + 2 * cn.asc_tree.w) / 2, mt.h - cn.asc_tree.h,
                   "\x1b[38;2;204;255;88m");

    // Draw the house
    mt.print_ascii(&cn.asc_house, (mt.w + cn.asc_house.w) / 2, mt.h - cn.asc_house.h,
                   "\x1b[38;2;251;194;110m");

    // Draw title at the center of the screen
    mt.print_ascii(&cn.asc_title, (mt.w - cn.asc_title.w) / 2, (mt.h - cn.asc_title.h) / 2,
                   "\x1b[38;2;255;231;151m");

    // Draw the cat
    mt.print_ascii(&cn.asc_cat, mt.x, mt.h - cn.asc_cat.h, "\x1b[38;2;255;231;151m");

    // Draw chat bubble
    let bubble = gen_bubble_ascii("I wish I could\nlive on that tree.");
    mt.print_ascii(&bubble, mt.x + 5, mt.h - cn.asc_cat.h - bubble.h, "\x1b[38;2;255;231;151m");
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
        txt.push_str(&*format!("\rDraw time: {:.2}ms", draw_time * 1000.0));
        stdout().write_all(txt.as_bytes()).await?;
        // print!("{}", txt);

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

        // Print bytes for debug
        for i in 0..n {
            for e in std::ascii::escape_default(buf[i]) {
                print!("{}", e as char);
            }
        }
        // print!("\r\n");

        {
            let mut mt = mt.lock().await;
            let mut move_x = |amount: i32| {
                mt.x = (mt.x as i32 + amount).max(0).min((mt.w - 1) as i32) as u16;
            };

            // Switch on the key
            match str.as_str() {
                "q" => {
                    mt.should_exit = true;
                    break;
                },
                "a" => move_x(-1),
                "d" => move_x(1),
                _ => (),
            }
        }

        // Sleep for 1/100th of a second
        tokio::time::sleep(Duration::from_millis(30)).await;
    }

    Ok(())
}

fn run() -> Result<()> {
    pretty_env_logger::init();

    let cn: &Consts = Box::leak(Box::new(Consts::new()));
    let mt = Arc::new(Mutex::new(Mutes::new(&cn)));

    // Set terminal to raw mode
    let mut out = std::io::stdout().into_raw_mode().unwrap();

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
        // tokio::try_join!(pull_input)?;
        Ok::<(), Error>(())
    })?;

    // Reset the terminal
    out.suspend_raw_mode().unwrap();
    out.write(SHOW_CURSOR.as_ref())?;
    out.write(CLEAR.as_ref())?;
    out.write("Exiting...".as_ref())?;
    out.flush()?;

    Ok(())
}

fn main() {
    run().expect("Error running program");
}
