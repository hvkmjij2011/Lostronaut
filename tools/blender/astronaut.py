"""The astronaut: helmet with visor, torso, backpack, two arms, two legs and the terrain tool.

Parts and pivots (Blender space, front = -Y). The game (src/player.rs) swings the limbs about
their pivots around the X axis and raises `arm_r` while the tool is used; `tool` is a child
of `arm_r` and points along the arm, so it aims forward when the arm is raised.
`backpack` is its own part (child of `torso`); the UI uses its pivot as the panel anchor.
Model's left side is +X in Blender (it faces the viewer in front view).
"""
from kit import Model

HIP = 0.8
SHOULDER = 1.34
NECK = 1.44


def astronaut():
    M = Model("astronaut", ground_ao=False)
    M.shot_dir = (0.7, -1.4, 0.45)
    t = M.part("torso", pivot=(0, 0, HIP), smooth_angle=35)
    # waist: dark flexible belt with an orange buckle
    t.cyl(0.215, 0.12, at=(0, 0, HIP - 0.03), m="dark", seg=16, bev=0.03)
    t.box((0.12, 0.05, 0.08), at=(0, -0.21, HIP + 0.03), m="orange", bev=0.015)
    # pelvis shell
    t.box((0.42, 0.3, 0.14), at=(0, 0, HIP - 0.07), m="white", bev=0.06, seg=2)
    # chest shell, slightly tapered by stacking two rounded boxes
    t.box((0.5, 0.34, 0.3), at=(0, 0, HIP + 0.22), m="white", bev=0.09, seg=2)
    t.box((0.56, 0.36, 0.26), at=(0, 0.0, HIP + 0.43), m="white", bev=0.1, seg=2)
    # orange side panels and collar
    for s in (-1, 1):
        t.box((0.04, 0.24, 0.3), at=(s * 0.27, 0, HIP + 0.3), m="orange", bev=0.015)
    t.torus(0.17, 0.045, at=(0, 0, NECK - 0.03), m="orange", seg=20, tseg=8)
    # chest control unit
    t.box((0.26, 0.07, 0.16), at=(0, -0.185, HIP + 0.38), m="dark", bev=0.025)
    t.box((0.14, 0.02, 0.05), at=(-0.04, -0.222, HIP + 0.41), m="g_cyan", bev=0.006)
    t.sphere(0.018, at=(0.08, -0.225, HIP + 0.41), m="g_green", seg=8, rings=5)
    t.sphere(0.018, at=(0.08, -0.225, HIP + 0.35), m="g_red", seg=8, rings=5)
    t.box((0.18, 0.02, 0.025), at=(0, -0.222, HIP + 0.33), m="metal", bev=0.004)
    # hose from the chest unit around to the backpack
    t.pipe([(0.13, -0.19, HIP + 0.32), (0.25, -0.12, HIP + 0.2), (0.27, 0.1, HIP + 0.22), (0.2, 0.2, HIP + 0.32)], 0.022, m="dark", seg=6)

    h = M.part("head", pivot=(0, 0, NECK), parent="torso", smooth_angle=50)
    h.cyl(0.15, 0.08, at=(0, 0, NECK - 0.02), m="metal", seg=16, bev=0.02)
    h.sphere(0.27, at=(0, 0, NECK + 0.24), m="white", seg=20, rings=12)
    # visor: big dark glass bulging out of the helmet front, framed in orange
    h.sphere((0.22, 0.17, 0.17), at=(0, -0.12, NECK + 0.25), m="visor", seg=20, rings=10)
    h.torus(0.2, 0.025, at=(0, -0.2, NECK + 0.25), rot=(90, 0, 0), m="orange", seg=24, tseg=6)
    h.sphere((0.05, 0.02, 0.025), at=(-0.09, -0.275, NECK + 0.33), rot=(-30, 0, 25), m="steel", seg=8, rings=4)
    # ear pods with headlamps, antenna on the left
    for s in (-1, 1):
        h.cyl(0.075, 0.07, at=(s * 0.25, 0, NECK + 0.24), rot=(0, s * 90, 0), m="orange", seg=14, bev=0.015)
        h.box((0.05, 0.08, 0.05), at=(s * 0.25, -0.12, NECK + 0.33), m="dark", bev=0.012)
        h.sphere(0.022, at=(s * 0.25, -0.165, NECK + 0.33), m="g_white", seg=8, rings=5)
    h.cyl(0.012, 0.28, at=(0.27, 0.05, NECK + 0.3), m="dark", seg=6, bev=0)
    h.sphere(0.025, at=(0.27, 0.05, NECK + 0.58), m="g_orange", seg=8, rings=5)

    b = M.part("backpack", pivot=(0, 0.2, HIP + 0.36), parent="torso", smooth_angle=35)
    b.box((0.5, 0.26, 0.6), at=(0, 0.33, HIP + 0.34), m="white", bev=0.07, seg=2)
    b.box((0.52, 0.27, 0.1), at=(0, 0.33, HIP + 0.6), m="orange", bev=0.04)
    b.box((0.36, 0.06, 0.3), at=(0, 0.46, HIP + 0.3), m="dark", bev=0.02)
    for i in range(4):
        b.box((0.3, 0.03, 0.03), at=(0, 0.49, HIP + 0.2 + i * 0.065), m="metal", bev=0.005)
    b.box((0.2, 0.02, 0.035), at=(0, 0.49, HIP + 0.5), m="g_cyan", bev=0.006)
    # twin oxygen tanks on the sides
    for s in (-1, 1):
        b.cyl(0.075, 0.5, at=(s * 0.27, 0.36, HIP + 0.08), m="white", seg=14, bev=0.02)
        b.sphere(0.075, at=(s * 0.27, 0.36, HIP + 0.58), m="white", seg=14, rings=8)
        b.cyl(0.078, 0.06, at=(s * 0.27, 0.36, HIP + 0.36), m="blue", seg=14, bev=0.01)
        b.cyl(0.03, 0.05, at=(s * 0.27, 0.36, HIP + 0.64), m="dark", seg=8, bev=0)
    b.box((0.44, 0.1, 0.06), at=(0, 0.3, HIP + 0.04), m="dark", bev=0.02)
    # thruster nozzles at the bottom
    for s in (-1, 1):
        b.cyl(0.04, 0.06, at=(s * 0.12, 0.38, HIP - 0.03), m="metal", seg=10, r2=0.03, bev=0)

    for side, s in (("l", 1), ("r", -1)):
        x = s * 0.34
        a = M.part(f"arm_{side}", pivot=(x, 0, SHOULDER), parent="torso", smooth_angle=45)
        a.sphere(0.115, at=(x, 0, SHOULDER), m="orange", seg=14, rings=8)
        a.cyl(0.075, 0.24, at=(x, 0, SHOULDER - 0.3), m="white", seg=12, r2=0.085, bev=0.02)
        a.cyl(0.07, 0.05, at=(x, 0, SHOULDER - 0.33), m="dark", seg=12, bev=0.01)
        a.cyl(0.07, 0.22, at=(x, 0, SHOULDER - 0.56), m="white", seg=12, r2=0.075, bev=0.02)
        a.cyl(0.082, 0.06, at=(x, 0, SHOULDER - 0.6), m="orange", seg=12, bev=0.015)
        # glove
        a.box((0.11, 0.13, 0.13), at=(x, -0.01, SHOULDER - 0.67), m="dark", bev=0.04, seg=2)
        a.box((0.04, 0.05, 0.08), at=(x - s * 0.06, -0.06, SHOULDER - 0.66), m="dark", bev=0.015)

        lg = M.part(f"leg_{side}", pivot=(s * 0.13, 0, HIP), parent="torso", smooth_angle=45)
        xx = s * 0.13
        lg.sphere(0.12, at=(xx, 0, HIP - 0.04), m="dark", seg=12, rings=8)
        lg.cyl(0.1, 0.3, at=(xx, 0, HIP - 0.4), m="white", seg=12, r2=0.112, bev=0.02)
        lg.box((0.15, 0.08, 0.13), at=(xx, -0.08, HIP - 0.42), m="orange", bev=0.035, seg=2)
        lg.cyl(0.092, 0.05, at=(xx, 0, HIP - 0.47), m="dark", seg=12, bev=0.01)
        lg.cyl(0.11, 0.3, at=(xx, 0, HIP - 0.72), m="white", seg=12, r2=0.095, bev=0.02)
        lg.cyl(0.115, 0.05, at=(xx, 0, HIP - 0.66), m="orange", seg=12, bev=0.015)
        # boot
        lg.box((0.18, 0.3, 0.13), at=(xx, -0.04, 0.075), m="dark", bev=0.05, seg=2)
        lg.box((0.19, 0.31, 0.035), at=(xx, -0.04, 0.018), m="orange", bev=0.012)
        lg.box((0.15, 0.08, 0.06), at=(xx, -0.16, 0.12), m="metal", bev=0.02)

    # terrain tool: grip in the right glove, body and emitter pointing down the arm
    hx = -0.34
    hz = SHOULDER - 0.68
    g = M.part("tool", pivot=(hx, 0, hz), parent="arm_r", smooth_angle=45)
    g.box((0.06, 0.07, 0.14), at=(hx, 0.0, hz), m="black", bev=0.015)
    g.box((0.13, 0.16, 0.2), at=(hx, -0.07, hz - 0.15), m="orange", bev=0.04, seg=2)
    g.box((0.135, 0.06, 0.12), at=(hx, 0.02, hz - 0.15), m="white", bev=0.02)
    g.box((0.14, 0.02, 0.05), at=(hx, -0.15, hz - 0.12), m="g_cyan", bev=0.006)
    g.cyl(0.06, 0.18, at=(hx, -0.07, hz - 0.43), m="dark", seg=12, r2=0.045, bev=0.01)
    g.cyl(0.075, 0.05, at=(hx, -0.07, hz - 0.46), m="metal", seg=12, bev=0.01)
    g.torus(0.06, 0.02, at=(hx, -0.07, hz - 0.47), m="g_cyan", seg=16, tseg=6)
    g.cyl(0.035, 0.02, at=(hx, -0.07, hz - 0.49), m="g_white", seg=10, bev=0)
    return M


MODELS = {"astronaut": astronaut}
