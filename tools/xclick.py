#!/usr/bin/env python3
"""Tiny XTEST driver (xdotool is not usable here): xclick.py x y [button] -> move and click."""
import ctypes
import sys
import time

x11 = ctypes.CDLL("libX11.so.6")
xtst = ctypes.CDLL("libXtst.so.6")
x11.XOpenDisplay.restype = ctypes.c_void_p
display = x11.XOpenDisplay(None)
if not display:
    raise SystemExit("no display")
args = sys.argv[1:]
while args:
    x, y = int(args.pop(0)), int(args.pop(0))
    button = int(args.pop(0)) if args and args[0] in ("1", "2", "3") else 0
    xtst.XTestFakeMotionEvent(ctypes.c_void_p(display), -1, x, y, 0)
    x11.XFlush(ctypes.c_void_p(display))
    time.sleep(0.15)
    if button:
        xtst.XTestFakeButtonEvent(ctypes.c_void_p(display), button, 1, 0)
        x11.XFlush(ctypes.c_void_p(display))
        time.sleep(0.08)
        xtst.XTestFakeButtonEvent(ctypes.c_void_p(display), button, 0, 0)
        x11.XFlush(ctypes.c_void_p(display))
        time.sleep(0.15)
