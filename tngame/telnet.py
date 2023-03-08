
ASCII_CAT = r"""
 /\_/\
( | | )
 >   <""".strip('\n')

ASCII_HEIGHT = ASCII_CAT.count('\n')
ASCII_WIDTH = max(len(line.strip('\n')) for line in ASCII_CAT.splitlines())

