#!/usr/bin/env python3
"""Draws the four 16x16 movement arrows of the labyrinth action area into
assets/textures/ui/: straight ahead, a curve to the left, a curve to the right
and a U-turn back. White on transparent; the game tints them."""
from PIL import Image
import os

N = 16
OUT = os.path.join(os.path.dirname(__file__), "..", "assets", "textures", "ui")


def blank():
    return [[0] * N for _ in range(N)]


def dot(g, x, y):
    if 0 <= x < N and 0 <= y < N:
        g[y][x] = 1


def rect(g, x0, y0, x1, y1):
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            dot(g, x, y)


def head_up(g, cx, y):
    """Arrow head pointing up with its tip at row y, centred between columns
    cx-1 and cx: rows 2, 4, 6, 8 wide."""
    for i in range(4):
        rect(g, cx - 1 - i, y + i, cx + i, y + i)


def head_left(g, x, cy):
    """Head pointing left, tip at column x, centred between rows cy-1 and cy."""
    for i in range(4):
        rect(g, x + i, cy - 1 - i, x + i, cy + i)


def head_down(g, cx, y):
    """Head pointing down with its tip at row y, centred between cx-1 and cx."""
    for i in range(4):
        rect(g, cx - 1 - i, y - i, cx + i, y - i)


# Every arrow's bounding box is an even number of pixels each way and centred
# on the 16x16 canvas, so it sits dead centre in a button.


def ahead():
    g = blank()
    head_up(g, 8, 2)  # rows 2-5, columns 4-11
    rect(g, 6, 6, 9, 13)  # a 4-wide shaft
    return g


def left():
    g = blank()
    # bar y 4-7 from the head to the corner, then 4 wide down the right side
    rect(g, 6, 4, 13, 7)
    rect(g, 10, 8, 13, 13)
    g[4][13] = 0  # round the corner
    head_left(g, 2, 6)  # tip column 2, base column 5, rows 2-9
    return g


def right():
    g = left()
    return [row[::-1] for row in g]


def back():
    g = blank()
    rect(g, 3, 2, 14, 5)  # over the top
    rect(g, 11, 6, 14, 13)  # up the right side
    rect(g, 3, 6, 6, 9)  # down the left side
    g[2][14] = 0  # round the outer corners
    g[2][3] = 0
    head_down(g, 5, 13)  # rows 10-13, columns 1-8 at its base
    return g


for name, g in [("ahead", ahead()), ("left", left()), ("right", right()), ("back", back())]:
    im = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    for y in range(N):
        for x in range(N):
            if g[y][x]:
                im.putpixel((x, y), (255, 255, 255, 255))
    path = os.path.join(OUT, f"move_{name}.png")
    im.save(path)
    print(path)
