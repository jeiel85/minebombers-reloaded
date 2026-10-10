"""Clean-room pilot (issue #63): draw a subset of the SIKA.SPY glyphs from the engine's layout spec only
(crates/mb-core/src/glyphs.rs coordinates and the MapValue names), never from the original image.

Usage: python gen_sika.py [OUT_DIR]   (default: out/ next to this script; needs Pillow)

Outputs:
  SIKA.SPY          640x480, 16 colours, glyphs at the engine's exact coordinates (undrawn = colour 0)
  sika_pilot.png    the same sheet as PNG
  scene_pilot.png   640x480 game screen composed with the engine's rules, scaled x2 (stand-in HUD)
  sprites_zoom.png  labelled close-up of every pilot sprite
  walk_p1.gif       player 1 walk cycle, 4 directions
"""
import math
import random
import zlib
import re
import sys
from pathlib import Path

from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[2]
OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent / "out"
OUT.mkdir(parents=True, exist_ok=True)

# ---------------------------------------------------------------- spec parsing
level_rs = (REPO / "crates/mb-core/src/world/map/level.rs").read_text(encoding="utf-8")
enum_body = level_rs[level_rs.index("pub enum MapValue") :]
enum_body = enum_body[: enum_body.index("\n}")]
VALUE = {n: int(v, 0) for n, v in re.findall(r"^\s+([A-Z][A-Za-z0-9]+)\s*=\s*(0x[0-9A-Fa-f]+|\d+),", enum_body, re.M)}
assert VALUE["Passage"] == 0x30 and len(VALUE) == 256, len(VALUE)

glyphs_rs = (REPO / "crates/mb-core/src/glyphs.rs").read_text(encoding="utf-8")
mg = glyphs_rs[glyphs_rs.index("pub const MAP_GLYPHS") :]
mg = mg[mg.index("= [") + 3 : mg.index("];")]
UNMAPPED = (50, 70)
map_glyphs = []
for tok in re.findall(r"\((-?\d+),\s*(-?\d+)\)|UNMAPPED", mg):
    map_glyphs.append(UNMAPPED if tok == ("", "") else (int(tok[0]), int(tok[1])))
assert len(map_glyphs) == 135, len(map_glyphs)
SPECIAL = {
    "BlackHoleBomb": (140, 10), "BlackHoleActive": (20, 30), "FreezeBomb": (150, 10),
    "DrillDroneRight": (160, 20), "DrillDroneLeft": (170, 20), "DrillDroneUp": (180, 20), "DrillDroneDown": (190, 20),
}


def map_xy(name):
    if name in SPECIAL:
        return SPECIAL[name]
    v = VALUE[name]
    lo, hi = VALUE["Passage"], VALUE["Item182"]
    return map_glyphs[v - lo] if lo <= v <= hi else UNMAPPED


# ---------------------------------------------------------------- palette
def hx(s):
    return tuple(int(s[i : i + 2], 16) for i in (1, 3, 5))

# SPY images are 4 bitplanes: 16 colours for the whole sheet. Every named colour below is one of them.
PAL = [hx(c) for c in (
    "#1c1510",  # 0 passage (near-black brown)
    "#7f5428",  # 1 sand dark / boots / gold shade
    "#a8733d",  # 2 sand
    "#c48f55",  # 3 sand light / fuse
    "#4b4f57",  # 4 stone dark / overalls shade
    "#767a82",  # 5 stone
    "#a3a6ad",  # 6 stone light
    "#f2f2ee",  # 7 white
    "#cc3328",  # 8 red
    "#7a2216",  # 9 dark red / brick
    "#e8b830",  # 10 gold / yellow
    "#ff8a1e",  # 11 orange
    "#3f6fc8",  # 12 blue
    "#3aa84a",  # 13 green
    "#f0b48a",  # 14 skin
    "#0c0c0e",  # 15 black
)]
IDX = {
    "void": 0, "pass": 0, "pass2": 0,
    "sand": 2, "sandL": 3, "sandD": 1, "sandDD": 1,
    "grav": 1, "peb": 5, "pebL": 6, "pebD": 4,
    "stone": 5, "stoneL": 6, "stoneD": 4, "stoneDD": 0,
    "metal": 4, "metalL": 6, "metalD": 15, "rivet": 7,
    "brick": 9, "brickL": 8, "mortar": 0,
    "gold": 10, "goldL": 7, "goldD": 1,
    "dia": 12, "diaL": 7, "diaD": 4,
    "white": 7, "red": 8, "redD": 9,
    "black": 15, "blackL": 6, "bomb": 4,
    "fuse": 3, "spark1": 10, "spark2": 11, "spark3": 8,
    "dyn": 8, "dynL": 11,
    "exW": 7, "exY": 10, "exO": 11, "exR": 8,
    "smoke": 5, "smokeL": 6, "smokeD": 4,
    "skin": 14, "eye": 15, "boot": 1, "lamp": 10,
}
C = {k: PAL[i] for k, i in IDX.items()}
C.update({"hud": hx("#24262c"), "hudL": hx("#3b3e47"), "hudT": hx("#d7dbe3")})  # stand-in HUD only
PLAYER = [  # helmet, helmet shade, overalls, overalls shade -- all palette entries
    (PAL[8], PAL[9], PAL[12], PAL[4]),
    (PAL[12], PAL[4], PAL[5], PAL[4]),
    (PAL[13], PAL[4], PAL[9], PAL[0]),
    (PAL[10], PAL[1], PAL[13], PAL[4]),
]


class Tile:
    def __init__(self, w=10, h=10, fill="pass"):
        self.w, self.h = w, h
        self.px = [[C[fill]] * w for _ in range(h)]

    def set(self, x, y, c):
        if 0 <= x < self.w and 0 <= y < self.h:
            self.px[y][x] = C[c] if isinstance(c, str) else c

    def get(self, x, y):
        return self.px[y][x]

    def copy(self):
        t = Tile(self.w, self.h)
        t.px = [row[:] for row in self.px]
        return t

    def image(self):
        im = Image.new("RGB", (self.w, self.h))
        im.putdata([p for row in self.px for p in row])
        return im


def passage(seed=1):
    r = random.Random(seed)
    t = Tile()
    for _ in range(4):
        t.set(r.randrange(10), r.randrange(10), "pass2")
    return t


def speckled(base, light, dark, seed, pl=0.13, pd=0.15, w=10, h=10, darker=None):
    r = random.Random(seed)
    t = Tile(w, h, base)
    for y in range(h):
        for x in range(w):
            v = r.random()
            if v < pl:
                t.set(x, y, light)
            elif v < pl + pd:
                t.set(x, y, dark)
            elif darker and v < pl + pd + 0.03:
                t.set(x, y, darker)
    return t


def sand(seed):
    return speckled("sand", "sandL", "sandD", seed, darker="sandDD")


def pebble(t, x, y, big=False):
    pts = [(0, 0), (1, 0), (0, 1), (1, 1)] + ([(2, 0), (2, 1), (0, 2), (1, 2), (2, 2)] if big else [])
    for dx, dy in pts:
        t.set(x + dx, y + dy, "peb")
    t.set(x, y, "pebL")
    s = 2 if big else 1
    t.set(x + s, y + s, "pebD")


def gravel(seed, heavy):
    r = random.Random(seed)
    t = speckled("grav", "sandL", "sandD", seed + 100, pl=0.08, pd=0.12)
    spots = [(1, 1), (6, 2), (2, 6), (6, 6)] if heavy else [(1, 2), (6, 6)]
    for i, (x, y) in enumerate(spots):
        pebble(t, x + r.randrange(2), y + r.randrange(2), big=heavy and i % 2 == 0)
    return t


def stone(seed):
    r = random.Random(seed)
    t = speckled("stone", "stoneL", "stoneD", seed + 200, pl=0.05, pd=0.06)
    # one soft highlight blob and one shadow seam, different per variant
    hx0, hy0 = r.randrange(1, 6), r.randrange(1, 6)
    for dx, dy in [(0, 0), (1, 0), (0, 1), (2, 0)]:
        t.set(hx0 + dx, hy0 + dy, "stoneL")
    sx, sy = r.randrange(0, 7), r.randrange(4, 9)
    for i in range(r.randrange(3, 5)):
        t.set(sx + i, sy - (i // 2), "stoneD")
    return t


def crack(t, seed, n):
    r = random.Random(seed)
    for _ in range(n):
        x, y = r.randrange(2, 8), r.randrange(1, 4)
        for _ in range(6):
            t.set(x, y, "stoneDD")
            x += r.choice([-1, 0, 1])
            y += 1
    return t


def stone_corner(seed, corner):
    """Stone tile with one corner rounded off; the cut shows sand (the engine draws a sand border
    over it when a passage is next to it)."""
    t = stone(seed)
    s = sand(seed + 7)
    cx = 0 if "Left" in corner else 9
    cy = 0 if "Top" in corner else 9
    for y in range(10):
        for x in range(10):
            d = math.hypot(x - cx, y - cy)
            if d < 4.6:
                t.set(x, y, s.get(x, y))
            elif d < 5.6:
                t.set(x, y, "stoneDD")
    return t


def metal():
    t = Tile(fill="metal")
    for i in range(10):
        t.set(i, 0, "metalL"); t.set(0, i, "metalL")
        t.set(i, 9, "metalD"); t.set(9, i, "metalD")
    for x, y in [(2, 2), (7, 2), (2, 7), (7, 7)]:
        t.set(x, y, "rivet"); t.set(x + 1, y + 1, "metalD")
    return t


def brick(seed=0, cracks=0):
    t = Tile(fill="mortar")
    for row in range(3):
        y0 = row * 3 + (1 if row else 0)
        off = 0 if row % 2 == 0 else 3
        for x in range(10):
            if (x + off) % 5 == 4:
                continue
            for y in range(y0, min(y0 + 2, 10)):
                t.set(x, y, "brickL" if y == y0 else "brick")
    if cracks:
        r = random.Random(seed)
        for _ in range(cracks):
            x, y = r.randrange(1, 9), r.randrange(0, 4)
            for _ in range(5):
                t.set(x, y, "mortar")
                x += r.choice([-1, 1]); y += 1
    return t


def sprite(rows, colors, base=None):
    t = base.copy() if base else passage(3)
    for y, row in enumerate(rows):
        for x, ch in enumerate(row):
            if ch != ".":
                t.set(x, y, colors[ch])
    return t


def boulder():
    t = passage(5)
    for y in range(10):
        for x in range(10):
            d = math.hypot(x - 4.5, y - 5.0)
            if d < 4.4:
                light = (x - 4.5) + (y - 5.0)
                t.set(x, y, "stoneL" if light < -3 else "stoneD" if light > 2.6 else "stone")
            elif d < 4.9 and (x + y) > 8:
                t.set(x, y, "stoneDD")
    return t


GOLD = {"Y": "gold", "L": "goldL", "D": "goldD", "W": "white", "R": "red", "r": "redD",
        "C": "dia", "c": "diaD", "w": "diaL", "K": "black", "k": "blackL"}


def bomb(frame, big):
    r = 4.2 if big else 3.2
    cx, cy = (4.5, 5.6) if big else (4.5, 6.2)
    t = passage(9)
    for y in range(10):
        for x in range(10):
            d = math.hypot(x - cx, y - cy)
            if d < r - 0.8:
                t.set(x, y, "white" if (x - cx) + (y - cy) < -r * 1.05 else "blackL" if (x - cx) + (y - cy) < -r * 0.5 else "bomb")
            elif d < r:
                t.set(x, y, "black")
    top = int(cy - r)
    t.set(6, top, "fuse"); t.set(7, top - 1, "fuse")
    spark = ["spark1", "spark2", "spark3"][frame]
    sx, sy = 8, max(top - 2, 0)
    t.set(sx, sy, spark)
    if frame != 1:
        t.set(sx + 1, sy, "spark2"); t.set(sx, sy + 1 if frame == 0 else sy, "spark1")
    return t


def dynamite(frame):
    t = passage(11)
    for x in (2, 4, 6):
        for y in range(3, 10):
            t.set(x, y, "dynL" if y == 3 else "dyn")
            t.set(x + 1, y, "dyn" if y == 3 else "redD")
    for x in range(2, 8):
        t.set(x, 6, "fuse")
    t.set(5, 2, "fuse"); t.set(6, 1, "fuse")
    t.set(7, 0, ["spark1", "spark2", "spark3"][frame])
    return t


def explosion(seed):
    r = random.Random(seed)
    t = Tile(fill="exR")
    for y in range(10):
        for x in range(10):
            d = math.hypot(x - 4.5, y - 4.5) + r.uniform(-1.1, 1.1)
            t.set(x, y, "exW" if d < 1.6 else "exY" if d < 3.2 else "exO" if d < 5.0 else "exR")
    return t


def smoke(seed, dense):
    r = random.Random(seed)
    t = passage(seed)
    for _ in range(3 if dense else 2):
        cx, cy, rad = r.uniform(2.5, 6.5), r.uniform(2.5, 6.5), r.uniform(2.2, 3.4 if dense else 2.6)
        for y in range(10):
            for x in range(10):
                d = math.hypot(x - cx, y - cy)
                if d < rad and ((x + y) % 2 == 0 or d < rad * 0.45):
                    t.set(x, y, "smokeL" if d < rad * 0.5 else "smoke")
    return t


def border(kind, side, burned=False):
    """Rough edge drawn over the edge of a dirt/stone cell that faces a passage.
    side = which edge of that cell (Left/Right glyphs are 4x10, Up/Down are 10x3)."""
    horiz = side in ("Left", "Right")
    w, h = (4, 10) if horiz else (10, 3)
    seed = zlib.crc32(f"{kind}{side}{burned}".encode()) & 0xFFFF
    body = sand(seed) if kind == "sand" else stone(seed)
    t = Tile(w, h)
    rim = "sandDD" if kind == "sand" else "stoneDD"
    r = random.Random(seed)
    length = 10
    depth = []
    d = 1
    for _ in range(length):
        d = max(0, min(w - 1 if horiz else h - 1, d + r.choice([-1, 0, 0, 1])))
        depth.append(d)
    for i in range(length):
        for j in range(w if horiz else h):
            # j counts inward from the passage side
            x, y = (j, i) if horiz else (i, j)
            if side == "Left":
                x = j  # passage is to the left of this cell's left edge
            elif side == "Right":
                x = w - 1 - j
            elif side == "Up":
                y = j
            else:
                y = h - 1 - j
            if j < depth[i]:
                t.set(x, y, "pass")
            elif j == depth[i]:
                t.set(x, y, rim)
            else:
                t.set(x, y, body.get(x % 10, y % 10))
    if burned:
        for y in range(h):
            for x in range(w):
                p = t.get(x, y)
                if p != C["pass"]:
                    t.set(x, y, "stoneDD" if p in (C["sandDD"], C["stoneD"], C["sandD"]) else "sandDD" if kind == "sand" else "stoneD")
    return t


# --- player sprites: Right is drawn, Left mirrors it; Up/Down drawn separately
RIGHT_TOP = [
    "...HHH....",
    "..HHHHHL..",
    "..hhhhh...",
    "...SSES...",
    "...SSSS...",
]
RIGHT_BODY = {
    0: ["..BBBBBS..", "..BBBBB...", "...bbbb...", "...K..K...", "..KK..KK.."],
    1: ["..BBBBB...", "..BBBBBS..", "...bbbb...", "....KK....", "....KKK..."],
    2: [".SBBBBB...", "..BBBBB...", "...bbbb...", "..K...K...", ".KK...KK.."],
    3: ["..BBBBB...", "..BBBBBS..", "...bbbb...", "....KK....", "...KKK...."],
}
DOWN_TOP = ["...HHHH...", "..HHLLHH..", "..hhhhhh..", "..SESSES..", "...SSSS..."]
UP_TOP = ["...HHHH...", "..HHHHHH..", "..hhhhhh..", "..hhhhhh..", "...SSSS..."]
FRONT_BODY = {
    0: [".SBBBBBBS.", "..BBBBBB..", "..bbbbbb..", "..K....K..", ".KK....KK."],
    1: [".SBBBBBB..", "..BBBBBBS.", "..bbbbbb..", "..K...KK..", ".KK......."],
    2: [".SBBBBBBS.", "..BBBBBB..", "..bbbbbb..", "..K....K..", ".KK....KK."],
    3: ["..BBBBBBS.", ".SBBBBBB..", "..bbbbbb..", "..KK...K..", ".......KK."],
}


def player(pi, direction, frame):
    helm, helmD, body, bodyD = PLAYER[pi]
    cols = {"H": helm, "h": helmD, "L": C["lamp"], "S": C["skin"], "E": C["eye"],
            "B": body, "b": bodyD, "K": C["boot"]}
    if direction in ("Right", "Left"):
        rows = RIGHT_TOP + RIGHT_BODY[frame]
        if direction == "Left":
            rows = [row[::-1] for row in rows]
    else:
        rows = (DOWN_TOP if direction == "Down" else UP_TOP) + FRONT_BODY[frame]
    return sprite(rows, cols, base=passage(13))


# Pickaxe swing, drawn over the standing pose: raised, half-way, strike, half-way.
# (x, y, colour) for a miner facing right; Left mirrors it, Up/Down swing on the miner's right side.
PICK_SWING = [
    [(7, 4, "skin"), (7, 3, "fuse"), (8, 2, "fuse"), (7, 1, "stoneL"), (8, 1, "stoneL"), (9, 1, "stoneL"), (9, 2, "stoneD")],
    [(7, 5, "skin"), (8, 4, "fuse"), (9, 3, "fuse"), (9, 2, "stoneL"), (9, 4, "stoneL"), (9, 5, "stoneD")],
    [(7, 6, "skin"), (8, 6, "fuse"), (9, 6, "fuse"), (9, 5, "stoneL"), (9, 7, "stoneL"), (9, 8, "stoneD")],
    [(7, 5, "skin"), (8, 4, "fuse"), (9, 3, "fuse"), (9, 2, "stoneL"), (9, 4, "stoneL"), (9, 5, "stoneD")],
]


def player_pickaxe(pi, direction, frame):
    """Glyph::Monster(.., Digging::Pickaxe, ..): shown while the miner digs stone or brick."""
    t = player(pi, direction, 0)
    for x, y, c in PICK_SWING[frame]:
        t.set(9 - x if direction == "Left" else x, y, c)
    return t


# ---------------------------------------------------------------- the pilot glyph set
MAP_TILES = {
    "Passage": passage(1),
    "MetalWall": metal(),
    "Sand1": sand(21), "Sand2": sand(22), "Sand3": sand(23),
    "LightGravel": gravel(31, False), "HeavyGravel": gravel(32, True),
    "Stone1": stone(41), "Stone2": stone(42), "Stone3": stone(43), "Stone4": stone(44),
    "StoneTopLeft": stone_corner(45, "TopLeft"), "StoneTopRight": stone_corner(46, "TopRight"),
    "StoneBottomRight": stone_corner(47, "BottomRight"), "StoneBottomLeft": stone_corner(48, "BottomLeft"),
    "StoneLightCracked": crack(stone(49), 1, 1), "StoneHeavyCracked": crack(stone(50), 2, 3),
    "Boulder": boulder(),
    "Brick": brick(), "BrickLightCracked": brick(3, 1), "BrickHeavyCracked": brick(4, 3),
    "SmallBomb1": bomb(0, False), "SmallBomb2": bomb(1, False), "SmallBomb3": bomb(2, False),
    "BigBomb1": bomb(0, True), "BigBomb2": bomb(1, True), "BigBomb3": bomb(2, True),
    "Dynamite1": dynamite(0), "Dynamite2": dynamite(1), "Dynamite3": dynamite(2),
    "Explosion": explosion(7),
    "Smoke1": smoke(61, True), "Smoke2": smoke(62, False),
    "GoldPileCoins": sprite(["..........", "..........", "....LY....", "...YLYD...", "..LYYDYY..",
                             ".YLYDYLYD.", "LYYDYYYDYY", "DDDDDDDDDD", "..........", ".........."], GOLD),
    "GoldBar": sprite(["..........", "..........", "..........", "...LLLL...", "..LYYYYD..",
                       ".LYYYYYYD.", ".YYYYYYYD.", ".DDDDDDDD.", "..........", ".........."], GOLD),
    "GoldCrown": sprite(["..........", ".Y..R..Y..", ".YY.Y.YY..", ".YYYYYYY..", ".YRYCYRY..",
                         ".YYYYYYY..", ".DDDDDDD..", "..........", "..........", ".........."], GOLD),
    "Diamond": sprite(["..........", "..........", "...wwwC...", "..wCCCCc..", ".wCCCCCCc.",
                       "..cCCCCc..", "...cCCc...", "....cc....", "..........", ".........."], GOLD),
    "Medikit": sprite(["..........", "...KKKK...", "..WWWWWW..", ".WWWRRWWW.", ".WWRRRRWW.",
                       ".WWRRRRWW.", ".WWWRRWWW.", ".WWWWWWWW.", "..........", ".........."], GOLD),
}

sheet = Image.new("RGB", (640, 480), C["void"])
for name, tile in MAP_TILES.items():
    sheet.paste(tile.image(), map_xy(name))

BORDER_RECTS = {
    ("sand", "Left", False): (194, 98), ("sand", "Right", False): (200, 98),
    ("sand", "Up", False): (194, 109), ("sand", "Down", False): (194, 113),
    ("stone", "Left", False): (148, 60), ("stone", "Right", False): (154, 60),
    ("stone", "Up", False): (148, 71), ("stone", "Down", False): (148, 75),
    ("sand", "Left", True): (194, 117), ("sand", "Right", True): (200, 117),
    ("sand", "Up", True): (194, 128), ("sand", "Down", True): (194, 132),
    ("stone", "Left", True): (205, 117), ("stone", "Right", True): (211, 117),
    ("stone", "Up", True): (205, 128), ("stone", "Down", True): (205, 132),
}
BORDERS = {k: border(*k) for k in BORDER_RECTS}
for k, xy in BORDER_RECTS.items():
    sheet.paste(BORDERS[k].image(), xy)

DIRS = ["Right", "Left", "Up", "Down"]
PLAYER_ORIGIN = [(160, 10), (160, 0), (160, 30), (160, 40)]
for pi, (ox, oy) in enumerate(PLAYER_ORIGIN):
    for di, d in enumerate(DIRS):
        for f in range(4):
            sheet.paste(player(pi, d, f).image(), (ox + di * 40 + f * 10, oy))
PICKAXE_ORIGIN = [(160, 200), (0, 200), (0, 210), (160, 210)]
for pi, (ox, oy) in enumerate(PICKAXE_ORIGIN):
    for di, d in enumerate(DIRS):
        for f in range(4):
            sheet.paste(player_pickaxe(pi, d, f).image(), (ox + di * 40 + f * 10, oy))
sheet.save(OUT / "sika_pilot.png")


def pixels(img):
    raw = img.convert("RGB").tobytes()
    return [tuple(raw[i : i + 3]) for i in range(0, len(raw), 3)]


def encode_spy(img):
    """Inverse of mb_core::images::decode_spy: 768-byte palette, then 4 RLE bitplanes.
    RLE: a byte other than 0x01 is a literal; 0x01, value, count repeats value count times."""
    w, h = img.size
    lut = {c: i for i, c in enumerate(PAL)}
    idx = [lut[p] for p in pixels(img)]  # KeyError = colour outside the palette
    pal = bytearray(768)
    for i, (r, g, b) in enumerate(PAL):
        pal[i * 3 : i * 3 + 3] = bytes((r, g, b))
    out = bytearray(pal)
    for plane in range(4):
        raw = bytearray()
        for i in range(0, w * h, 8):
            byte = 0
            for bit in range(8):
                byte |= ((idx[i + bit] >> plane) & 1) << (7 - bit)
            raw.append(byte)
        j = 0
        while j < len(raw):
            v, n = raw[j], 1
            while j + n < len(raw) and raw[j + n] == v and n < 255:
                n += 1
            if n >= 4 or v == 1:
                out += bytes((1, v, n))
            else:
                out += bytes([v] * n)
            j += n
    return bytes(out)


def decode_spy(data, w, h):
    pal, rest = data[:768], iter(data[768:])
    planes = []
    for _ in range(4):
        plane = bytearray()
        while len(plane) < w * h // 8:
            v = next(rest)
            if v != 1:
                plane.append(v)
            else:
                val, n = next(rest), next(rest)
                plane += bytes([val] * n)
        planes.append(plane)
    px = []
    for i in range(w * h // 8):
        for bit in range(7, -1, -1):
            c = sum(((planes[p][i] >> bit) & 1) << p for p in range(4))
            px.append(tuple(pal[c * 3 : c * 3 + 3]))
    return px


spy = encode_spy(sheet)
assert decode_spy(spy, 640, 480) == pixels(sheet), "SPY round trip mismatch"
(OUT / "SIKA.SPY").write_bytes(spy)
print("SIKA.SPY", len(spy), "bytes, round trip ok")


# ---------------------------------------------------------------- scene, composed like the engine does
def glyph_at(name):
    x, y = map_xy(name)
    return sheet.crop((x, y, x + 10, y + 10))


ROWS, COLS = 45, 64
rng = random.Random(2026)
grid = [[rng.choice(["Sand1", "Sand2", "Sand3"]) for _ in range(COLS)] for _ in range(ROWS)]


def blob(cy, cx, rad, fill, jitter=0.35):
    for y in range(max(1, cy - rad - 1), min(ROWS - 1, cy + rad + 2)):
        for x in range(max(1, cx - rad - 1), min(COLS - 1, cx + rad + 2)):
            if math.hypot((y - cy) * 1.15, x - cx) < rad + rng.uniform(-jitter, jitter) * rad:
                grid[y][x] = fill() if callable(fill) else fill


for _ in range(9):
    blob(rng.randrange(4, 41), rng.randrange(4, 60), rng.randrange(2, 5),
         lambda: rng.choice(["Stone1", "Stone2", "Stone3", "Stone4"]))
for _ in range(12):
    blob(rng.randrange(3, 42), rng.randrange(3, 61), rng.randrange(1, 3),
         lambda: rng.choice(["LightGravel", "LightGravel", "HeavyGravel"]))


def carve(y, x, steps):
    d = rng.choice([(0, 1), (0, -1), (1, 0), (-1, 0)])
    for _ in range(steps):
        if rng.random() < 0.22:
            d = rng.choice([(0, 1), (0, -1), (1, 0), (-1, 0)])
        y = min(max(1, y + d[0]), ROWS - 2)
        x = min(max(1, x + d[1]), COLS - 2)
        grid[y][x] = "Passage"


starts = [(2, 2), (2, COLS - 3), (ROWS - 3, 2), (ROWS - 3, COLS - 3)]
for y, x in starts:
    for _ in range(3):
        carve(y, x, 70)
for _ in range(5):
    carve(rng.randrange(5, 40), rng.randrange(5, 59), 50)

# a small brick hut and a metal wall run
for x in range(26, 36):
    grid[18][x] = grid[26][x] = "Brick"
for y in range(18, 27):
    grid[y][26] = grid[y][35] = "Brick"
for y in range(19, 26):
    for x in range(27, 35):
        grid[y][x] = "Passage"
grid[22][26] = "Passage"
grid[18][30] = "BrickLightCracked"; grid[26][32] = "BrickHeavyCracked"
for x in range(44, 56):
    grid[34][x] = "MetalWall"

# stone corners on convex blob corners
is_st = lambda n: n.startswith("Stone")
for y in range(1, ROWS - 1):
    for x in range(1, COLS - 1):
        if not is_st(grid[y][x]):
            continue
        up, dn, lf, rt = (is_st(grid[y - 1][x]), is_st(grid[y + 1][x]), is_st(grid[y][x - 1]), is_st(grid[y][x + 1]))
        if not up and not lf and dn and rt: grid[y][x] = "StoneTopLeft"
        elif not up and not rt and dn and lf: grid[y][x] = "StoneTopRight"
        elif not dn and not rt and up and lf: grid[y][x] = "StoneBottomRight"
        elif not dn and not lf and up and rt: grid[y][x] = "StoneBottomLeft"
        elif rng.random() < 0.05: grid[y][x] = rng.choice(["StoneLightCracked", "StoneHeavyCracked"])

for y in range(ROWS):
    grid[y][0] = grid[y][COLS - 1] = "MetalWall"
for x in range(COLS):
    grid[0][x] = grid[ROWS - 1][x] = "MetalWall"

passages = [(y, x) for y in range(ROWS) for x in range(COLS) if grid[y][x] == "Passage"]
sands = [(y, x) for y in range(ROWS) for x in range(COLS) if grid[y][x].startswith("Sand")]
for name, n in [("GoldPileCoins", 5), ("GoldBar", 3), ("GoldCrown", 2), ("Diamond", 4), ("Boulder", 10)]:
    for y, x in rng.sample(sands, n):
        grid[y][x] = name
for name, n in [("Medikit", 2), ("SmallBomb2", 3), ("BigBomb1", 2), ("Dynamite3", 2)]:
    for y, x in rng.sample(passages, n):
        grid[y][x] = name
# one going-off explosion with smoke around it
ey, ex = 22, 30
for dy in range(-1, 2):
    for dx in range(-2, 3):
        grid[ey + dy][ex + dx] = "Explosion" if abs(dx) + abs(dy) <= 2 else "Smoke1"
grid[ey][ex + 3] = "Smoke2"; grid[ey - 2][ex] = "Smoke2"

screen = Image.new("RGB", (640, 480), C["hud"])
d = ImageDraw.Draw(screen)
for i in range(4):  # simple stand-in HUD (not the original PLAYERS.SPY)
    x0 = 4 + i * 159
    d.rectangle((x0, 3, x0 + 151, 26), fill=C["hudL"], outline=PLAYER[i][0])
    d.text((x0 + 18, 9), f"PLAYER {i + 1}   $ {rng.randrange(80, 900)}", fill=C["hudT"])
    screen.paste(player(i, "Down", 0).image(), (x0 + 4, 10))

for y in range(ROWS):
    for x in range(COLS):
        screen.paste(glyph_at(grid[y][x]), (x * 10, y * 10 + 30))

SAND_LIKE = {"Sand1", "Sand2", "Sand3", "LightGravel", "HeavyGravel"}
OPEN = lambda n: not (n in SAND_LIKE or is_st(n) or n in ("MetalWall", "Brick", "BrickLightCracked", "BrickHeavyCracked"))
OFFS = {"Left": (-9, -5), "Right": (5, -5), "Up": (-5, -8), "Down": (-5, 5)}
STEP = {"Left": (0, -1), "Right": (0, 1), "Up": (-1, 0), "Down": (1, 0)}
REV = {"Left": "Right", "Right": "Left", "Up": "Down", "Down": "Up"}
CORNER = {"Right": ("StoneTopLeft", "StoneBottomLeft"), "Left": ("StoneTopRight", "StoneBottomRight"),
          "Down": ("StoneTopLeft", "StoneTopRight"), "Up": ("StoneBottomRight", "StoneBottomLeft")}
for y in range(1, ROWS - 1):
    for x in range(1, COLS - 1):
        if not OPEN(grid[y][x]):
            continue
        cx, cy = x * 10 + 5, y * 10 + 35
        for kind in ("sand", "stone"):
            for dname in ("Right", "Left", "Up", "Down"):
                n = grid[y + STEP[dname][0]][x + STEP[dname][1]]
                hit = (n in SAND_LIKE or n in CORNER[dname]) if kind == "sand" else is_st(n)
                if hit:
                    ox, oy = OFFS[dname]
                    screen.paste(BORDERS[(kind, REV[dname], False)].image(), (cx + ox, cy + oy))

# players standing in tunnels, mid-walk
for i, (sy, sx) in enumerate(starts):
    py, px = min(passages, key=lambda p: abs(p[0] - sy) + abs(p[1] - sx))
    screen.paste(player(i, DIRS[i], 1).image(), (px * 10, py * 10 + 30))

screen.resize((1280, 960), Image.NEAREST).save(OUT / "scene_pilot.png")

# ---------------------------------------------------------------- labelled close-up
items = [(n, t.image()) for n, t in MAP_TILES.items()]
items += [(f"{k[0]}Border {k[1]}", BORDERS[k].image()) for k in BORDER_RECTS if not k[2]]
for pi in range(4):
    for di, dn in enumerate(DIRS):
        items.append((f"P{pi + 1} {dn}", player(pi, dn, 0).image()))
for f in range(4):
    items.append((f"P1 pick R{f + 1}", player_pickaxe(0, "Right", f).image()))
Z, CELL_W, CELL_H, PER_ROW = 8, 120, 112, 10
rows_n = (len(items) + PER_ROW - 1) // PER_ROW
zoom = Image.new("RGB", (PER_ROW * CELL_W, rows_n * CELL_H), C["hud"])
dz = ImageDraw.Draw(zoom)
for i, (label, im) in enumerate(items):
    gx, gy = (i % PER_ROW) * CELL_W, (i // PER_ROW) * CELL_H
    big = im.resize((im.width * Z, im.height * Z), Image.NEAREST)
    zoom.paste(big, (gx + (CELL_W - big.width) // 2, gy + 6 + (80 - big.height) // 2))
    dz.text((gx + 4, gy + 94), label[:19], fill=C["hudT"])
zoom.save(OUT / "sprites_zoom.png")

frames = []
for f in range(4):
    fr = Image.new("RGB", (4 * 12 * 10, 12 * 10), C["pass"])
    for di, dn in enumerate(DIRS):
        fr.paste(player(0, dn, f).image().resize((100, 100), Image.NEAREST), (di * 120 + 10, 10))
    frames.append(fr)
frames[0].save(OUT / "walk_p1.gif", save_all=True, append_images=frames[1:], duration=140, loop=0)
print("ok", len(MAP_TILES), "map tiles,", len(BORDERS), "borders, 64 player frames")
