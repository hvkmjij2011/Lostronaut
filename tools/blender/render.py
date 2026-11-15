"""Studio renders: model preview shots (contact sheets) and 128 px item / structure icons."""
import bpy
import math
import numpy as np
from mathutils import Vector


def setup_scene():
    sc = bpy.context.scene
    sc.render.engine = "CYCLES"
    sc.cycles.samples = 48
    sc.cycles.use_denoising = True
    try:
        sc.cycles.denoiser = "OPENIMAGEDENOISE"
    except Exception:
        pass
    sc.cycles.device = "CPU"
    sc.render.film_transparent = True
    sc.view_settings.view_transform = "Standard"
    sc.view_settings.look = "None"
    sc.view_settings.exposure = 0.0
    sc.render.image_settings.file_format = "PNG"
    sc.render.image_settings.color_mode = "RGBA"
    world = bpy.data.worlds.new("lr_world")
    world.use_nodes = True
    bg = world.node_tree.nodes["Background"]
    bg.inputs["Color"].default_value = (0.16, 0.18, 0.22, 1.0)
    bg.inputs["Strength"].default_value = 1.0
    sc.world = world

    def sun(name, rot, energy, color):
        d = bpy.data.lights.new(name, "SUN")
        d.energy = energy
        d.color = color
        d.angle = math.radians(8)
        o = bpy.data.objects.new(name, d)
        o.rotation_euler = [math.radians(a) for a in rot]
        sc.collection.objects.link(o)

    # key from front-left-above, cool fill from the right, warm rim from behind
    sun("key", (50, 0, -35), 3.6, (1.0, 0.96, 0.9))
    sun("fill", (65, 0, 70), 1.0, (0.75, 0.85, 1.0))
    sun("rim", (60, 0, 165), 2.2, (1.0, 0.9, 0.8))
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    sc.collection.objects.link(cam)
    sc.camera = cam
    return cam


def _coords(objs):
    pts = []
    dg = bpy.context.evaluated_depsgraph_get()
    for ob in objs:
        if ob.type != "MESH":
            continue
        mw = ob.matrix_world
        for v in ob.data.vertices:
            pts.append(mw @ v.co)
    return pts


def frame(cam, objs, direction=(0.95, -1.3, 0.8), lens=60, margin=1.08):
    """Points the camera along `direction` (from target to camera) and fits the objects."""
    bpy.context.view_layer.update()
    pts = _coords(objs)
    lo = Vector([min(p[i] for p in pts) for i in range(3)])
    hi = Vector([max(p[i] for p in pts) for i in range(3)])
    centre = (lo + hi) / 2
    d = Vector(direction).normalized()
    cam.data.lens = lens
    cam.data.sensor_width = 36
    cam.rotation_euler = (-d).to_track_quat("-Z", "Y").to_euler()
    bpy.context.view_layer.update()
    # fit: grow the distance until every point is inside the frustum (with margin)
    half = math.atan(18 / lens) / margin
    right = cam.matrix_world.to_3x3() @ Vector((1, 0, 0))
    up = cam.matrix_world.to_3x3() @ Vector((0, 1, 0))
    dist = 0.0
    for p in pts:
        v = p - centre
        z = v.dot(d)
        for axis in (right, up):
            s = abs(v.dot(axis))
            dist = max(dist, z + s / math.tan(half))
    cam.location = centre + d * dist
    cam.data.clip_start = 0.01
    cam.data.clip_end = dist * 4 + 10


def show_only(collections, visible):
    for c in collections:
        c.hide_render = c not in visible


def render(path, size):
    sc = bpy.context.scene
    sc.render.resolution_x = size
    sc.render.resolution_y = size
    sc.render.resolution_percentage = 100
    sc.render.filepath = path
    bpy.ops.render.render(write_still=True)


def outline(path, radius=1, color=(0.08, 0.09, 0.11), strength=0.9):
    """Adds a dark outline around the alpha so the icon reads on any background."""
    img = bpy.data.images.load(path, check_existing=False)
    w, h = img.size
    px = np.array(img.pixels[:], dtype=np.float32).reshape(h, w, 4)
    a = px[:, :, 3]
    dil = a.copy()
    for dy in range(-radius, radius + 1):
        for dx in range(-radius, radius + 1):
            if dx * dx + dy * dy > radius * radius + radius:
                continue
            dil = np.maximum(dil, np.roll(np.roll(a, dy, 0), dx, 1))
    ra = np.maximum(a, dil * strength)
    rgb = px[:, :, :3] * a[..., None] + np.array(color)[None, None, :] * (ra - a)[..., None]
    rgb = np.where(ra[..., None] > 1e-4, rgb / np.maximum(ra, 1e-4)[..., None], 0)
    out = np.dstack([rgb, ra]).astype(np.float32)
    img.pixels[:] = out.ravel()
    img.filepath_raw = path
    img.file_format = "PNG"
    img.save()
    bpy.data.images.remove(img)
