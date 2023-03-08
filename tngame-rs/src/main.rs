#![feature(let_chains)]

use std::io::{stdin, Write};
use std::{io, mem, thread};
use std::borrow::ToOwned;
use std::ptr::null;
use std::string::ToString;
use std::time::{Duration, Instant};
use rand::Rng;
use anyhow::Result;
use termion::color::{Fg, Rgb};
use termion::cursor::Goto;

const RESET: &str = "\x1b[0m";

/// Constants
const SNOW_DENSITY: f32 = 0.05; // Snow particles per pixel on screen
const SNOW_SPEED: f32 = 8.0; // Snow fall speed in pixels per second
const SNOW_X_RAND: f32 = 0.8; // Snow x velocity randomization factor

/// Colors: Convert them in python using hyfetch - print(repr(RGB.from_hex('#FFFFFF')))
const COLORS_STR: [&str; 3] = [
    // # FFFFFF
    "\x1b[38;2;246;170;183m",
    // # F6AAB7
    "\x1b[38;2;255;255;255m",
    // # 55CDFD
    "\x1b[38;2;85;205;253m"
];

