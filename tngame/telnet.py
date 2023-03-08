import asyncio
import os
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


COLORS = {RGB.from_hex(v) for v in {
    '#FFFFFF',
    '#F6AAB7',
    '#55CDFD'
}}


# Snow fall data structure
class Snow(NamedTuple):
    x: int
    y: int
    color: RGB


async def shell(reader: TelnetReaderUnicode, writer: TelnetWriterUnicode):
    """
    The main shell function.
    """
    height: int
    width: int
    x: int
    y: int
    snow: list[Snow]

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

    clear()
    height, width = await get_size()
    log.info(f"Size: {width}x{height}")

    # Position the cat at center bottom by default
    x, y = (width - ASCII_WIDTH) // 2, height - ASCII_HEIGHT

    # Draw the cat function
    def draw_cat():
        log.info(f"Drawing cat at {x}, {y}")
        clear()

        # Draw the cat
        print_ascii(ASCII_CAT, x, y)

    # Move the cat along the x-axis
    async def move(delta: int):
        nonlocal x
        x = min(max(x + delta, 0), width - ASCII_WIDTH)

    # Update frame function
    async def update():
        # Move the cat
        draw_cat()
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

    # Run listen and update in parallel
    await asyncio.gather(listen_input(), listen_update())


def run():
    # Create a new event loop, start the server and wait for it to close
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    coro = telnetlib3.create_server(port=2323, shell=shell)
    server = loop.run_until_complete(coro)
    loop.run_until_complete(server.wait_closed())

