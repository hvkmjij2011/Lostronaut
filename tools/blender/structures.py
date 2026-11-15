"""Buildable structures, the crashed pod, the starship parts, wreckage and pickups.

Design language ("one manufacturer"): chamfered white hulls, orange trim and corner caps,
dark machine bases standing on round pads, dark panel grooves, small glowing status lamps.
Blender space: Z up, front faces -Y, origin on the ground. Footprint radii come from
`StructDef::radius` in src/items.rs and are checked by build_assets.py.

Animation tags (part.anim), evaluated by src/models.rs:
    spin <axis> <rad/s> <cond>       rotate about the pivot
    swing <axis> <amp> <hz> <cond>   oscillate about the pivot
    slide <axis> <amp> <hz> <cond>   oscillate along an axis
    pulse <amp> <hz> <cond>          scale pulse
    sun                              yaw the part towards the sun
axis letters are Blender axes here (converted on export); cond = always | job | fuel
"""
import math
import random
from mathutils import Vector

from kit import Model, look_rot

TAU = math.tau


# ------------------------------------------------------------------ shared details
def pads(p, pts, r=0.13, h=0.1):
    for x, y in pts:
        p.cyl(r, h, at=(x, y, 0), m="dark", seg=10, r2=r * 0.85, bev=0.02)
        p.cyl(r * 0.45, 0.12, at=(x, y, h), m="metal", seg=8, bev=0)


def slab(p, w, d, z0=0.08, h=0.2, m="dark", corners=True):
    """Machine base: a dark chamfered slab with orange corner caps, on four pads."""
    pads(p, [(sx * (w / 2 - 0.16), sy * (d / 2 - 0.16)) for sx in (-1, 1) for sy in (-1, 1)])
    p.box((w, d, h), at=(0, 0, z0 + h / 2), m=m, bev=0.05)
    p.box((w - 0.08, d - 0.08, 0.025), at=(0, 0, z0 + h + 0.005), m="metal", bev=0.01)
    if corners:
        for sx in (-1, 1):
            for sy in (-1, 1):
                p.box((0.22, 0.22, h + 0.04), at=(sx * (w / 2 - 0.09), sy * (d / 2 - 0.09), z0 + h / 2), m="orange", bev=0.04)
    return z0 + h


def oct_base(p, r, z0=0.08, h=0.2, m="dark"):
    pads(p, [(math.cos(a) * (r - 0.15), math.sin(a) * (r - 0.15)) for a in (TAU / 8, 3 * TAU / 8, 5 * TAU / 8, 7 * TAU / 8)])
    p.cyl(r, h, at=(0, 0, z0), rot=(0, 0, 22.5), m=m, seg=8, bev=0.05)
    p.cyl(r - 0.05, 0.03, at=(0, 0, z0 + h), rot=(0, 0, 22.5), m="metal", seg=8, bev=0.01)
    return z0 + h


def lamp(p, at, m="g_green", r=0.045, facing=(0, -1, 0)):
    d = Vector(facing)
    p.cyl(r * 1.6, 0.03, at=Vector(at) - d * 0.02, rot=look_rot(d), m="dark", seg=10, bev=0.005)
    p.sphere(r, at=Vector(at) + d * 0.015, m=m, seg=10, rings=6)


def grille(p, at, w, h, slats=4, facing="-y", m="dark", slat="metal"):
    """A recessed vent: dark frame with horizontal slats. Faces -y (default), +x or -x."""
    x, y, z = at
    if facing in ("-y", "+y"):
        s = -1 if facing == "-y" else 1
        p.box((w, 0.05, h), at=(x, y, z), m=m, bev=0.012)
        for i in range(slats):
            zz = z - h / 2 + (i + 0.5) * h / slats
            p.box((w - 0.06, 0.03, h / slats * 0.45), at=(x, y + s * 0.025, zz), m=slat, bev=0.006)
    else:
        s = -1 if facing == "-x" else 1
        p.box((0.05, w, h), at=(x, y, z), m=m, bev=0.012)
        for i in range(slats):
            zz = z - h / 2 + (i + 0.5) * h / slats
            p.box((0.03, w - 0.06, h / slats * 0.45), at=(x + s * 0.025, y, zz), m=slat, bev=0.006)


def bolt_ring(p, r, z, n=8, m="metal", br=0.03):
    for i in range(n):
        a = i / n * TAU
        p.cyl(br, 0.03, at=(math.cos(a) * r, math.sin(a) * r, z), m=m, seg=6, bev=0)


def fan(p, r, blades=5, m="dark", hub="orange", z=0.0, pitch=25):
    p.cyl(r * 0.28, 0.08, at=(0, 0, z), m=hub, seg=12, bev=0.015)
    for i in range(blades):
        a = i / blades * 360
        p.box((r * 0.72, r * 0.26, 0.025), at=(math.cos(math.radians(a)) * r * 0.55, math.sin(math.radians(a)) * r * 0.55, z + 0.04),
              rot=(pitch, 0, a), m=m, bev=0.008)
        # rotate box about its own centre: box() applies rot before translation, so the
        # blade is pitched about its long axis (X) then turned to angle a about Z
    return p


def stripe_ring(p, r, z, h=0.06, m="orange", seg=16):
    p.cyl(r + 0.012, h, at=(0, 0, z), m=m, seg=seg, bev=0.008)


def groove_ring(p, r, z, seg=16):
    p.cyl(r + 0.006, 0.025, at=(0, 0, z), m="dark", seg=seg, bev=0, caps=True)


def hazard_band(p, w, at, h=0.08, n=6, axis="x", facing=-1):
    """Yellow / black diagonal-ish chevrons as alternating blocks."""
    x, y, z = at
    for i in range(n):
        m = "yellow" if i % 2 == 0 else "black"
        o = -w / 2 + (i + 0.5) * w / n
        if axis == "x":
            p.box((w / n, 0.02, h), at=(x + o, y, z), m=m, bev=0.002)
        else:
            p.box((0.02, w / n, h), at=(x, y + o, z), m=m, bev=0.002)


# ------------------------------------------------------------------ structures
def smelter():
    M = Model("Smelter")
    b = M.part("body")
    top = oct_base(b, 1.28)
    # furnace drum
    b.cyl(0.86, 1.15, at=(0, 0, top), m="white", seg=16, bev=0.04)
    groove_ring(b, 0.86, top + 0.32)
    stripe_ring(b, 0.86, top + 0.86, h=0.14)
    b.sphere((0.86, 0.86, 0.38), at=(0, 0, top + 1.15), m="shell", seg=16, rings=10, cut=0.0)
    # hopper on top
    b.cyl(0.2, 0.3, at=(-0.18, 0.08, top + 1.38), m="dark", seg=12, r2=0.42, bev=0.02)
    b.torus(0.42, 0.04, at=(-0.18, 0.08, top + 1.68), m="orange", seg=16, tseg=6)
    b.cyl(0.36, 0.02, at=(-0.18, 0.08, top + 1.6), m="g_orange", seg=12, bev=0)
    # furnace mouth (front)
    b.box((0.78, 0.36, 0.62), at=(0, -0.78, top + 0.46), m="dark", bev=0.05)
    b.box((0.86, 0.4, 0.08), at=(0, -0.8, top + 0.8), m="orange", bev=0.03)
    b.box((0.54, 0.06, 0.34), at=(0, -0.96, top + 0.44), m="g_orange", bev=0.02)
    for i in range(3):
        b.box((0.56, 0.04, 0.03), at=(0, -0.99, top + 0.33 + i * 0.11), m="black", bev=0.005)
    # output tray
    b.box((0.7, 0.34, 0.08), at=(0, -1.0, top + 0.08), m="steel", bev=0.02)
    b.box((0.7, 0.04, 0.1), at=(0, -1.15, top + 0.13), m="steel", bev=0.01)
    # side glow vents
    for a in (-55, 55, 180):
        r = math.radians(a - 90)
        x, y = math.cos(r) * 0.87, math.sin(r) * 0.87
        b.box((0.36, 0.06, 0.16), at=(x, y, top + 0.62), rot=(0, 0, a), m="dark", bev=0.015)
        b.box((0.28, 0.07, 0.06), at=(x, y, top + 0.62), rot=(0, 0, a), m="g_orange", bev=0.01)
    # chimney (back right) with fan on top
    cx, cy = 0.66, 0.62
    b.cyl(0.24, 0.3, at=(cx, cy, top), m="dark", seg=12, bev=0.03)
    b.cyl(0.17, 2.0, at=(cx, cy, top + 0.3), m="metal", seg=12, bev=0.0)
    for z in (0.9, 1.6):
        b.cyl(0.2, 0.07, at=(cx, cy, top + z), m="dark", seg=12, bev=0.01)
    b.tube(0.3, 0.24, 0.22, at=(cx, cy, top + 2.25), m="dark", seg=16)
    b.cyl(0.3, 0.05, at=(cx, cy, top + 2.47), m="orange", seg=16, bev=0.01, caps=False)
    b.pipe([(0.3, 0.55, top + 1.0), (0.5, 0.6, top + 1.15), (cx - 0.14, cy, top + 1.15)], 0.07, m="steel", seg=8)
    # control box and lamp on the left
    b.box((0.3, 0.3, 0.5), at=(-0.82, -0.42, top + 0.3), rot=(0, 0, 30), m="white", bev=0.04)
    b.box((0.2, 0.04, 0.16), at=(-0.9, -0.56, top + 0.4), rot=(0, 0, 30), m="g_cyan", bev=0.01)
    lamp(b, (-0.76, -0.55, top + 0.62), "g_green", facing=(0, 0, 1))
    f = M.part("fan", pivot=(cx, cy, top + 2.32), anim="spin z 6.0 job")
    f.cyl(0.04, 0.12, at=(cx, cy, top + 2.2), m="metal", seg=8, bev=0)
    for i in range(5):
        a = i / 5 * 360
        ar = math.radians(a)
        f.box((0.2, 0.08, 0.02), at=(cx + math.cos(ar) * 0.13, cy + math.sin(ar) * 0.13, top + 2.33), rot=(25, 0, a), m="dark", bev=0.006)
    f.cyl(0.07, 0.06, at=(cx, cy, top + 2.31), m="orange", seg=10, bev=0.01)
    return M


STRUCTURES = {
    "Smelter": smelter,
}
