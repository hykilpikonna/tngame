import asyncio

import telnetlib3
from telnetlib3 import TelnetReaderUnicode, TelnetWriterUnicode
from hypy_utils.logging_utils import setup_logger


log = setup_logger()


async def shell(reader: TelnetReaderUnicode, writer: TelnetWriterUnicode):
    # Get the size of the terminal
    async def get_size() -> tuple[int, int]:
        writer.write('\x1b[18t')
        await writer.drain()
        size = await reader.read(100)

        # Parse the size, it's in the format of \x1b[8;{height};{width}t
        height, width = size.split(';')[1:3]
        height, width = int(height), int(width[:-1])

        return height, width

    # Run tngame-rs
    h, w = await get_size()
    proc = await asyncio.create_subprocess_exec(
        './tngame-rs/target/release/tngame-rs',
        stdin=asyncio.subprocess.PIPE,
        stdout=asyncio.subprocess.PIPE,
        env={'TN_TERM_SIZE': f'{w}x{h}'}
    )

    async def relay_stdout():
        # Listen input
        while not writer.is_closing():
            # Close if subprocess is closed
            if proc.returncode is not None:
                break

            # Read output
            try:
                out = await proc.stdout.readuntil(b'\00\00\00')
                if out:
                    # print("Sending output from tngame-rs to telnet:", repr(out))
                    writer.write(out.decode())
                    await writer.drain()
            except asyncio.IncompleteReadError:
                break

        print("Closing reader.")
        raise asyncio.CancelledError("Closing reader.")

    async def relay_stdin():
        while not reader.connection_closed and not reader.at_eof():
            # Close if subprocess is closed
            if proc.returncode is not None:
                break

            inp: str = await reader.read(3)
            if inp:
                # print("Sending input from telnet to tngame-rs:", repr(inp))
                proc.stdin.write(inp.encode())
                await proc.stdin.drain()

        print("Closing writer.")
        raise asyncio.CancelledError("Closing writer.")

    tasks = [asyncio.create_task(relay_stdout()), asyncio.create_task(relay_stdin())]
    try:
        await asyncio.gather(*tasks)
    except asyncio.CancelledError:
        for task in tasks:
            task.cancel()

    if proc.returncode is None:
        print("Killing process.")
        proc.kill()

    # Print exit message
    writer.write("\r\nThanks for visiting <3\r\n")

    # Make cursor visible again
    writer.write("\x1b[?25h")

    await writer.drain()
    writer.close()
    reader.close()
    print("Connection closed.")


if __name__ == '__main__':
    # Create a new event loop, start the server and wait for it to close
    print("Starting server on port 2323.")
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    coro = telnetlib3.create_server(port=2323, shell=shell)
    server = loop.run_until_complete(coro)
    loop.run_until_complete(server.wait_closed())
