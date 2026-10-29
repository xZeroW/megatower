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
    """Arrow head pointing up with its tip at (cx, y): rows 1, 3, 5, 7 wide."""
    for i in range(4):
        rect(g, cx - i, y + i, cx + i, y + i)


def head_left(g, x, cy):
    for i in range(4):
        rect(g, x + i, cy - i, x + i, cy + i)


def head_right(g, x, cy):
    for i in range(4):
        rect(g, x - i, cy - i, x - i, cy + i)


def head_down(g, cx, y):
    for i in range(4):
        rect(g, cx - i, y - i, cx + i, y - i)


def ahead():
    g = blank()
    head_up(g, 7, 1)
    rect(g, 6, 5, 8, 14)
    return g


def left():
    # Up from the bottom, then bending left: a quarter turn.
    g = blank()
    rect(g, 10, 9, 12, 14)
    # quarter arc from (11, 9) round to (6, 4), three wide
    for x, y in [(11, 8), (11, 7), (10, 6), (9, 5), (8, 5), (7, 5)]:
        rect(g, x - 1, y - 1, x + 1, y + 1)
    head_left(g, 2, 5)
    return g


def right():
    g = left()
    return [row[::-1] for row in g]


def back():
    # Up the right side, over the top and down the left with the head down.
    g = blank()
    rect(g, 11, 6, 13, 14)
    for x, y in [(12, 5), (11, 4), (10, 3), (8, 3), (6, 3), (5, 4), (4, 5)]:
        rect(g, x - 1, y - 1, x + 1, y + 1)
    rect(g, 3, 5, 5, 10)
    head_down(g, 4, 14)
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
