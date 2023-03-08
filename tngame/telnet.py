import asyncio
import os
import random
import time
from dataclasses import dataclass
from typing import NamedTuple

import telnetlib3
from hyfetch.color_util import RGB
from telnetlib3 import TelnetReaderUnicode, TelnetWriterUnicode

from .utils import setup_logger

DEBUG = bool(os.environ.get("DEBUG", False))
log = setup_logger(DEBUG)


ASCII_CAT = r"""
 /\_/\
( | | )
 >   <""".strip('\n')

ASCII_HEIGHT = ASCII_CAT.count('\n')
ASCII_WIDTH = max(len(line.strip('\n')) for line in ASCII_CAT.splitlines())

SNOW_DENSITY = 0.05  # Snow particles per pixel on screen


COLORS = [RGB.from_hex(v) for v in {
    '#FFFFFF',
    '#F6AAB7',
    '#55CDFD'
}]


# Snow fall data structure
@dataclass
class SnowParticle:
    x: float  # x position
    y: float  # y position
    xv: float  # x velocity in pixels per second
    yv: float  # y velocity in pixels per second
    color: RGB  # color


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
            xv = random.randint(-1, 1)
            yv = random.randint(1, 2)

            # Generate random color
            color = random.choice(COLORS)

            snow.append(SnowParticle(x, y, xv, yv, color))

        return snow

    # Update snow particles
    def update_snow():
        nonlocal snow
        # Erase snow particles
        for particle in snow:
            # Erase snow particle
            writer.write(f'\x1b[{particle.y};{particle.x}H\x1b[0m ')

        # Update snow particles
        for particle in snow:
            # Update position
            particle.x += particle.xv
            particle.y += particle.yv

            # Wrap around the screen
            if particle.x >= width:
                particle.x = 0
            elif particle.x < 0:
                particle.x = width - 1

            if particle.y >= height:
                particle.y = 0

            # Draw snow particle
            writer.write(f'\x1b[{int(particle.y)};{int(particle.x)}H'
                         f'{particle.color.to_ansi_rgb()}*'
                         f'\x1b[0m')

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
    def print_ascii(asc: str, x: int, y: int):
        asc = asc.strip('\n')
        # Write ascii line by line
        for i, line in enumerate(asc.splitlines()):
            writer.write(f'\x1b[{y + i};{x}H')
            writer.write(line)

    # Clear the screen
    def clear():
        # Clear the screen
        writer.write('\x1b[2J')
        writer.write('\x1b[H')

    # Move the cat along the x-axis
    async def move(delta: int):
        nonlocal x
        x = min(max(x + delta, 0), width - ASCII_WIDTH)

    # Update frame function
    async def update():
        # Calculate the time since last update in seconds
        nonlocal last_update_ns
        now = time.time_ns()
        dt = (now - last_update_ns) / 1e9
        last_update_ns = now

        clear()
        # Update snow
        update_snow()
        # Move the cat
        print_ascii(ASCII_CAT, x, y)
        # Move the cursor to the bottom
        writer.write('\x1b[9999;9999H')
        # Flush the output
        await writer.drain()

    # Handle input function
    async def on_input(inp: str):
        nonlocal x, y
        log.info(repr(inp))
        # Switch case
        match inp:
            case '\x1b[C':  # Right
                await move(1)
            case '\x1b[D':  # Left
                await move(-1)
            case '\x1b':  # Escape
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
            # Wait for 0.1s
            await asyncio.sleep(0.1)

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
    x, y = (width - ASCII_WIDTH) // 2, height - ASCII_HEIGHT

    # Create snow particles
    snow = create_snow(100)

    # Run listen and update in parallel
    await asyncio.gather(listen_input(), listen_update())


def run():
    # Create a new event loop, start the server and wait for it to close
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    coro = telnetlib3.create_server(port=2323, shell=shell)
    server = loop.run_until_complete(coro)
    loop.run_until_complete(server.wait_closed())

