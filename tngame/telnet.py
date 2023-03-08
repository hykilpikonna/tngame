import asyncio
import os

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


async def shell(reader: TelnetReaderUnicode, writer: TelnetWriterUnicode):

    x, y = 0, height - ASCII_HEIGHT


def run():
    # Create a new event loop, start the server and wait for it to close
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    coro = telnetlib3.create_server(port=2323, shell=shell)
    server = loop.run_until_complete(coro)
    loop.run_until_complete(server.wait_closed())

