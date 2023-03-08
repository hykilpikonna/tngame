
def wrap_lines(lines: list[str], max_width: int = 30) -> list[str]:
    new_lines = []
    for line in lines:
        for line_part in [line[i:i+max_width] for i in range(0, len(line), max_width)]:
            new_lines.append(line_part)
    return new_lines


# Modified from https://github.com/VaasuDevanS/cowsay-python
def generate_bubble(text: str) -> str:
    lines = [line.strip() for line in str(text).split("\n")]
    lines = wrap_lines([line for line in lines if line])
    text_width = max([len(line) for line in lines])
    output = []
    output.append("." + "=" * (text_width + 2) + ".")
    for line in lines:
        output.append("| " + line + " " * (text_width - len(line) + 1) + "|")
    output.append("'" + "=" * (text_width + 2) + "'")
    return '\n'.join(output)
