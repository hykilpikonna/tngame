import asyncio
import os
import random
import time
from dataclasses import dataclass

import numba
import numpy as np
import telnetlib3
from hyfetch.color_util import RGB
from telnetlib3 import TelnetReaderUnicode, TelnetWriterUnicode

from .cowsay import generate_bubble
from .utils import setup_logger, get_ascii_dimensions

DEBUG = bool(os.environ.get("DEBUG", False))
log = setup_logger(DEBUG)


class AsciiArt:
    """An ASCII art object."""
    art: str
    h: int
    w: int
    credit: str

    def __init__(self, art: str, credit: str):
        self.art = art.strip("\n")
        self.h, self.w = get_ascii_dimensions(self.art)


ASC_CAT = AsciiArt(r"""
 /\_/\ 
( | | )
 >   < """, "Azalea")

ASC_TREE = AsciiArt(r"""
          %%%,%%%%%%%
       ,'%% \\-*%%%%%%%
 ;%%%%%*%   _%%%%"
  ,%%%       \(_.*%%%%.
  % *%%, ,%%%%*(    '
%^     ,*%%% )\|,%%*%,_
     *%    \/ #).-"*%%*
         _.) ,/ *%,
          /)#(
         /   \ """, "b'ger from ascii.co.uk/art/tree")

ASC_HOUSE = AsciiArt(r"""
         _ 
     ,--l l--------,
    / /^/    /^/  / \
   /_.--.___.--._/   \
   | ,--,   ,--, |  ,|
 ,%| '--'._.'--' |,o%o
.*%|_,%%_| |_%%,_|#%%%*""", "Modified from hjw from ascii.co.uk/art/house")

SNOW_DENSITY = 0.05  # Snow particles per pixel on screen
SNOW_SPEED = 8  # Snow fall speed in pixels per second
SNOW_X_RAND = 0.8  # Snow x velocity randomization factor


COLORS = [RGB.from_hex(v) for v in {
    '#FFFFFF',
    '#F6AAB7',
    '#55CDFD'
}]

print(repr(RGB.from_hex('#FFFFFF')))
exit(0)

# Snow fall data structure
@dataclass
class SnowParticle:
    last_x: int  # last rendered x position (pixels)
    last_y: int  # last rendered y position
    x: float  # x position
    y: float  # y position
    xv: float  # x velocity in pixels per second
    yv: float  # y velocity in pixels per second
    color: RGB  # color


@numba.jit(nopython=True)
def draw_buf_jit(width: int, height: int, last_buf: np.matrix, buf: np.matrix) -> str:
    char_buf = ''
    last_color = np.zeros(3, dtype=np.ubyte)

    # Iterate over pixels
    for x in range(width):
        for y in range(height):
            # Get pixel color
            color = buf[x, y, 0:3]
            char = buf[x, y, 3]
            if char == 0:
                char = 32  # Set empty pixels to space

            # Check if pixel color has changed
            if not np.array_equal(color, last_color) and not np.array_equal(color, last_buf[x, y, 0:3]):
                # Set pixel color
                char_buf += (f"\x1b[{y + 1};{x + 1}H"
                             f"\x1b[38;2;{color[0]};{color[1]};{color[2]}m{chr(char)}")
                last_color[:] = color[:]

            # Check if pixel character has changed
            elif char != last_buf[x, y, 3]:
                # Set pixel character
                char_buf += f"\x1b[{y + 1};{x + 1}H{chr(char)}"

    # Update last buffer
    last_buf[:] = buf[:]
    buf[:, :, :] = 0

    return char_buf


async def shell(reader: TelnetReaderUnicode, writer: TelnetWriterUnicode):
    """
    The main shell function.
    """
    height: int
    width: int
    x: int
    y: int
    snow: list[SnowParticle]
    last_update_ns: int

    # Bitmap buffer
    last_buf: np.matrix
    buf: np.matrix

    # Initialize buffers
    def init_buffers():
        nonlocal last_buf, buf

        # Initialize color buffer
        buf = np.zeros((width, height, 4), dtype=np.ubyte)
        last_buf = buf.copy()

    # Draw buffer to screen, dynamically updating only changed pixels
    def draw_buf():
        char_buf = draw_buf_jit(width, height, last_buf, buf)
        # Write buffer to screen
        writer.write(char_buf)


    def rand_velocity() -> tuple[float, float]:
        return random.uniform(-SNOW_X_RAND, SNOW_X_RAND) * SNOW_SPEED, \
            random.uniform(1, 2) * SNOW_SPEED

    # Create snow particles
    def create_snow(count: int | None) -> list[SnowParticle]:
        # Calculate snow particle count based on screen size and density
        if count is None:
            count = int(width * height * SNOW_DENSITY)

        snow = []

        for _ in range(count):
            # Generate random x and y position
            x = random.randint(0, width)
            y = random.randint(0, height)

            # Generate random x and y velocity
            xv, yv = rand_velocity()

            # Generate random color
            color = random.choice(COLORS)

            snow.append(SnowParticle(0, 0, x, y, xv, yv, color))

        # Sort snow particles by y position
        return sorted(snow, key=lambda p: p.y)

    # Update snow particles
    def update_snow(dt: float):
        nonlocal snow

        # Update snow particles
        for p in snow:
            # Update position
            p.x += p.xv * dt
            p.y += p.yv * dt

            # Wrap around the screen
            p.x = max(1.0, min(width - 1.0, p.x))
            if p.y >= height - 1:
                p.y = 1
                p.xv, p.yv = rand_velocity()

            # Draw new position to buffer
            buf[round(p.x), round(p.y), 0:3] = tuple(p.color)
            buf[round(p.x), round(p.y), 3] = ord('*')

    # Get the size of the terminal
    async def get_size() -> tuple[int, int]:
        # Get the size of the terminal
        writer.write('\x1b[18t')
        await writer.drain()
        size = await reader.read(100)

        # Parse the size, it's in the format of \x1b[8;{height};{width}t
        height, width = size.split(';')[1:3]
        height, width = int(height), int(width[:-1])

        return height, width

    # Print ascii art
    def print_ascii(asc: str | AsciiArt, x: int, y: int, color: RGB | None = None):
        if isinstance(asc, AsciiArt):
            asc = asc.art
        asc = asc.strip('\n')
        # Write ascii line by line
        for i, line in enumerate(asc.splitlines()):
            if color is not None:
                # Set color
                buf[x:x + len(line), y + i - 1, 0:3] = tuple(color)
            # Write line
            buf[x:x + len(line), y + i - 1, 3] = np.frombuffer(line.encode(), dtype=np.ubyte)

    # Clear the screen
    def clear():
        # Clear the screen
        writer.write('\x1b[2J')
        writer.write('\x1b[H')

    # Draw the tree
    def draw_tree():
        # print_ascii(ASC_TREE, (width - 2 * ASC_TREE.w) // 4, height - ASC_TREE.h, RGB.from_hex('#ccff58'))
        # print_ascii(ASC_TREE, (width + 2 * ASC_TREE.w) // 2, height - ASC_TREE.h, RGB.from_hex('#ccff58'))
        # print_ascii(ASC_HOUSE, (width + ASC_HOUSE.w) // 2, height - ASC_HOUSE.h, RGB.from_hex('#fbc26e'))
        pass

    def draw_cat():
        print_ascii(ASC_CAT, x, y, RGB.from_hex('#ffe797'))
        print_ascii(generate_bubble('I hope I can sleep\n on the tree'), x + 6, y - 3, RGB.from_hex('#ffe797'))

    # Move the cat along the x-axis
    async def move(delta: int):
        nonlocal x
        old_x = x
        x = min(max(x + delta, 0), width - ASC_CAT.w)

        if old_x > x:
            # Erase old cat right side
            for i in range(ASC_CAT.h):
                writer.write(f'\x1b[{y + i};{old_x + ASC_CAT.w - 1}H\x1b[0m ')
        elif old_x < x:
            # Erase old cat left side
            for i in range(ASC_CAT.h):
                writer.write(f'\x1b[{y + i};{old_x}H\x1b[0m ')

    draw_buf_time = 0
    draw_buf_count = 0

    # Update frame function
    async def update():
        # Calculate the time since last update in seconds
        nonlocal last_update_ns, draw_buf_time, draw_buf_count
        now = time.time_ns()
        dt = (now - last_update_ns) / 1e9
        last_update_ns = now

        # Update snow
        update_snow(dt)
        db_time = time.time_ns()
        draw_buf()
        draw_buf_count += 1
        if draw_buf_count != 1:
            draw_buf_time += time.time_ns() - db_time
        log.info(f'Average draw_buf time: {draw_buf_time / draw_buf_count / 1e6}ms')
        # Draw cat
        draw_tree()
        draw_cat()
        # Move the cursor to the bottom
        writer.write('\x1b[9999;9999H')
        # Flush the output
        await writer.drain()

    # Handle input function
    async def on_input(inp: str):
        nonlocal x, y
        # Switch case
        match inp:
            case '\x1b[C' | 'd':  # Right
                await move(1)
            case '\x1b[D' | 'a':  # Left
                await move(-1)
            case '\x1b' | 'q' | '\x03':  # Escape or q or Ctrl+C
                writer.write('\r\nBye!\r\n')
                writer.close()
                return True
            case _:
                log.info(f'Unknown input: {repr(inp)}')

        await update()

    # Update frame function
    async def listen_update():
        while True:
            await update()
            # 20 fps
            await asyncio.sleep(1 / 20)

    # Listen input function
    async def listen_input():
        while True:
            inp: str = await reader.read(3)
            if inp and await on_input(inp):
                return

    # Initialize the shell
    clear()
    height, width = await get_size()
    last_update_ns = time.time_ns()
    log.info(f"Size: {width}x{height}")

    # Position the cat at center bottom by default
    x, y = (width - ASC_CAT.w) // 2, height - ASC_CAT.h

    # Create snow particles
    snow = create_snow(100)
    init_buffers()

    # Run listen and update in parallel
    await asyncio.gather(listen_input(), listen_update())


def run():
    # Create a new event loop, start the server and wait for it to close
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    coro = telnetlib3.create_server(port=2323, shell=shell)
    server = loop.run_until_complete(coro)
    loop.run_until_complete(server.wait_closed())

