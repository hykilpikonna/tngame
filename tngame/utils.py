import logging


def get_ascii_dimensions(ascii_art: str) -> tuple[int, int]:
    """Get the dimensions of an ASCII art."""
    height = ascii_art.count('\n')
    width = max(len(line.strip('\n')) for line in ascii_art.splitlines())
    return height, width
