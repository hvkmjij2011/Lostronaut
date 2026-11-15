"""Modelling kit for the Lostranout asset pipeline.

Every model is built from code: a `Model` owns named `Part`s, each part is one bmesh that
primitives (bevelled boxes, cylinders, spheres, convex hulls, tori, pipes) are merged into.
Coordinates are Blender's: metres, Z up, the model's front faces -Y, origin on the ground.

A part has a pivot (model-space point) and an optional parent part. Its geometry is stored
relative to the pivot, so the game can rotate it about that point. Parts never carry a
rotation, which keeps the Blender -> game conversion a pure axis swap:
    game = (-x, z, y)        (a proper rotation; Blender -Y front -> game -Z front)

Colours: each face gets a palette material (sRGB colour + glow flag). Optional per-part colour
effects (soot, ground shading) are evaluated per corner and stored in the "Col" attribute,
which is what both the exporter and the Blender renders read.
"""
import bmesh
import bpy
import math
import struct
from mathutils import Euler, Matrix, Quaternion, Vector, noise

# ------------------------------------------------------------------ palette
# name -> (sRGB colour, glow)
PAL = {
    "white": ((0.91, 0.91, 0.89), False),
    "shell": ((0.74, 0.76, 0.78), False),
    "orange": ((0.96, 0.43, 0.08), False),
    "dorange": ((0.70, 0.27, 0.05), False),
    "dark": ((0.15, 0.16, 0.18), False),
    "black": ((0.07, 0.07, 0.08), False),
    "metal": ((0.44, 0.46, 0.50), False),
    "steel": ((0.62, 0.64, 0.68), False),
    "glass": ((0.24, 0.52, 0.80), False),
    "visor": ((0.10, 0.13, 0.22), False),
    "yellow": ((0.97, 0.76, 0.12), False),
    "red": ((0.85, 0.16, 0.12), False),
    "blue": ((0.16, 0.34, 0.80), False),
    "cell": ((0.10, 0.16, 0.42), False),
    "green": ((0.22, 0.62, 0.30), False),
    "g_cyan": ((0.40, 0.95, 1.00), True),
    "g_orange": ((1.00, 0.55, 0.12), True),
    "g_amber": ((1.00, 0.78, 0.35), True),
    "g_green": ((0.45, 1.00, 0.45), True),
    "g_red": ((1.00, 0.22, 0.16), True),
    "g_yellow": ((1.00, 0.90, 0.45), True),
    "g_pink": ((1.00, 0.50, 0.90), True),
    "g_blue": ((0.40, 0.62, 1.00), True),
    "g_white": ((1.00, 0.97, 0.90), True),
}


def srgb_to_lin(c):
    return tuple((x / 12.92) if x <= 0.04045 else ((x + 0.055) / 1.055) ** 2.4 for x in c)


def color_key(c, glow=False):
    """Register an ad-hoc colour (e.g. an item colour) and return its palette name."""
    name = ("g_" if glow else "c_") + "".join("%02x" % int(max(0, min(1, x)) * 255) for x in c)
    PAL.setdefault(name, (tuple(c), glow))
    return name


def shade(c, k):
    return tuple(min(1.0, x * k) for x in c)


def mix(a, b, t):
    return tuple(x + (y - x) * t for x, y in zip(a, b))


_MATS = {}


def material(name):
    """Blender material for a palette entry. Base colour comes from the "Col" attribute."""
    if name in _MATS:
        return _MATS[name]
    col, glow = PAL[name]
    m = bpy.data.materials.new("lr_" + name)
    m["lr_glow"] = glow
    m.diffuse_color = (*srgb_to_lin(col), 1.0)
    m.use_nodes = True
    nt = m.node_tree
    nt.nodes.clear()
    out = nt.nodes.new("ShaderNodeOutputMaterial")
    attr = nt.nodes.new("ShaderNodeVertexColor")
    attr.layer_name = "Col"
    if glow:
        em = nt.nodes.new("ShaderNodeEmission")
        em.inputs["Strength"].default_value = 2.2
        nt.links.new(attr.outputs["Color"], em.inputs["Color"])
        nt.links.new(em.outputs[0], out.inputs["Surface"])
    else:
        bs = nt.nodes.new("ShaderNodeBsdfPrincipled")
        rough = {"visor": 0.12, "glass": 0.2, "steel": 0.35, "metal": 0.45}.get(name, 0.62)
        bs.inputs["Roughness"].default_value = rough
        if name in ("steel", "metal"):
            bs.inputs["Metallic"].default_value = 0.4
        nt.links.new(attr.outputs["Color"], bs.inputs["Base Color"])
        nt.links.new(bs.outputs[0], out.inputs["Surface"])
    _MATS[name] = m
    return m


# ------------------------------------------------------------------ geometry helpers
def _rot(rot):
    if rot is None:
        return Matrix.Identity(4)
    if isinstance(rot, Quaternion):
        return rot.to_matrix().to_4x4()
    if isinstance(rot, Matrix):
        return rot.to_4x4()
    return Euler([math.radians(a) for a in rot], "XYZ").to_matrix().to_4x4()


def look_rot(direction):
    """Rotation taking local +Z onto `direction`."""
    return Vector((0, 0, 1)).rotation_difference(Vector(direction).normalized())


def bevel_sharp(bm, offset, segments=1, angle=50.0, edges=None):
    if offset <= 0:
        return
    lim = math.radians(angle)
    es = []
    for e in edges if edges is not None else bm.edges:
        if len(e.link_faces) == 2:
            if e.link_faces[0].normal.angle(e.link_faces[1].normal, 0.0) > lim:
                es.append(e)
        elif len(e.link_faces) == 1:
            pass
    if es:
        bmesh.ops.bevel(bm, geom=es, offset=offset, offset_type="OFFSET", segments=segments,
                        profile=0.5, affect="EDGES", clamp_overlap=True)


class Part:
    def __init__(self, model, name, pivot, parent, anim, smooth_angle):
        self.model = model
        self.name = name
        self.pivot = Vector(pivot)
        self.parent = parent
        self.anim = anim
        self.smooth_angle = smooth_angle
        self.bm = bmesh.new()
        self.mats = []
        self.fx = []  # callables (model-space pos, colour) -> colour

    # --- internal
    def _mi(self, m):
        if m not in self.mats:
            self.mats.append(m)
        return self.mats.index(m)

    def _merge(self, t, m, mat):
        """Transform temp bmesh `t` by `mat`, give it material `m` and append it."""
        t.transform(mat)
        mi = self._mi(m)
        vmap = {v: self.bm.verts.new(v.co) for v in t.verts}
        for f in t.faces:
            try:
                nf = self.bm.faces.new([vmap[v] for v in f.verts])
            except ValueError:
                continue
            nf.material_index = mi
        t.free()

    # --- primitives (all return self for chaining)
    def box(self, size, at=(0, 0, 0), rot=None, m="white", bev=0.03, seg=1):
        t = bmesh.new()
        bmesh.ops.create_cube(t, size=1.0)
        bmesh.ops.scale(t, vec=Vector(size), verts=t.verts)
        bev = min(bev, min(size) * 0.45)
        bevel_sharp(t, bev, seg, angle=30)
        self._merge(t, m, Matrix.Translation(Vector(at)) @ _rot(rot))
        return self

    def cyl(self, r, h, at=(0, 0, 0), rot=None, m="white", seg=16, r2=None, bev=0.02, caps=True):
        """Cylinder / cone along local +Z from 0 to h."""
        t = bmesh.new()
        r2 = r if r2 is None else r2
        bmesh.ops.create_cone(t, cap_ends=caps, cap_tris=False, segments=seg, radius1=r,
                              radius2=max(r2, 1e-4), depth=h)
        bmesh.ops.translate(t, vec=(0, 0, h / 2), verts=t.verts)
        if r2 < 1e-3:
            bmesh.ops.remove_doubles(t, verts=t.verts, dist=2e-4)
        if caps:
            bevel_sharp(t, min(bev, r * 0.3, h * 0.3), 1, angle=55)
        self._merge(t, m, Matrix.Translation(Vector(at)) @ _rot(rot))
        return self

    def tube(self, r_out, r_in, h, at=(0, 0, 0), rot=None, m="white", seg=16, bev=0.01):
        """Hollow cylinder (ring) along +Z."""
        t = bmesh.new()
        vs = []
        for k, (rr, z) in enumerate([(r_out, 0), (r_out, h), (r_in, h), (r_in, 0)]):
            ring = []
            for i in range(seg):
                a = i / seg * math.tau
                ring.append(t.verts.new((math.cos(a) * rr, math.sin(a) * rr, z)))
            vs.append(ring)
        for k in range(4):
            a, b = vs[k], vs[(k + 1) % 4]
            for i in range(seg):
                j = (i + 1) % seg
                t.faces.new([a[i], a[j], b[j], b[i]])
        bmesh.ops.recalc_face_normals(t, faces=t.faces)
        bevel_sharp(t, min(bev, (r_out - r_in) * 0.3, h * 0.3), 1, angle=55)
        self._merge(t, m, Matrix.Translation(Vector(at)) @ _rot(rot))
        return self

    def sphere(self, r, at=(0, 0, 0), rot=None, m="white", seg=16, rings=10, cut=None):
        """Ellipsoid. `cut` keeps only z >= cut (unit sphere space) for domes."""
        t = bmesh.new()
        bmesh.ops.create_uvsphere(t, u_segments=seg, v_segments=rings, radius=1.0)
        if cut is not None:
            geom = t.verts[:] + t.edges[:] + t.faces[:]
            bmesh.ops.bisect_plane(t, geom=geom, dist=1e-4, plane_co=(0, 0, cut), plane_no=(0, 0, 1), clear_inner=True)
            edges = [e for e in t.edges if e.is_boundary]
            if edges:
                bmesh.ops.edgeloop_fill(t, edges=edges)
        if not isinstance(r, (tuple, list)):
            r = (r, r, r)
        bmesh.ops.scale(t, vec=Vector(r), verts=t.verts)
        self._merge(t, m, Matrix.Translation(Vector(at)) @ _rot(rot))
        return self

    def hull(self, pts, at=(0, 0, 0), rot=None, m="white", bev=0.0, seg=1):
        """Convex hull of points, coplanar triangles merged, sharp edges bevelled."""
        t = bmesh.new()
        for p in pts:
            t.verts.new(p)
        res = bmesh.ops.convex_hull(t, input=t.verts[:])
        for g in res["geom_interior"] + res["geom_unused"]:
            if isinstance(g, bmesh.types.BMVert) and g.is_valid:
                t.verts.remove(g)
        bmesh.ops.dissolve_limit(t, angle_limit=math.radians(3), verts=t.verts[:], edges=t.edges[:])
        bmesh.ops.recalc_face_normals(t, faces=t.faces)
        if bev > 0:
            bevel_sharp(t, bev, seg, angle=25)
        self._merge(t, m, Matrix.Translation(Vector(at)) @ _rot(rot))
        return self

    def torus(self, R, r, at=(0, 0, 0), rot=None, m="white", seg=24, tseg=8):
        t = bmesh.new()
        rings = []
        for i in range(seg):
            a = i / seg * math.tau
            ring = []
            for j in range(tseg):
                b = j / tseg * math.tau
                rr = R + math.cos(b) * r
                ring.append(t.verts.new((math.cos(a) * rr, math.sin(a) * rr, math.sin(b) * r)))
            rings.append(ring)
        for i in range(seg):
            a, b = rings[i], rings[(i + 1) % seg]
            for j in range(tseg):
                k = (j + 1) % tseg
                t.faces.new([a[j], b[j], b[k], a[k]])
        bmesh.ops.recalc_face_normals(t, faces=t.faces)
        self._merge(t, m, Matrix.Translation(Vector(at)) @ _rot(rot))
        return self

    def pipe(self, pts, r, m="metal", seg=8, joints=True):
        """Pipe along a polyline with spherical elbows."""
        pts = [Vector(p) for p in pts]
        for a, b in zip(pts, pts[1:]):
            d = b - a
            self.cyl(r, d.length, at=a, rot=look_rot(d), m=m, seg=seg, bev=0, caps=False)
        if joints:
            for p in pts[1:-1]:
                self.sphere(r, at=p, m=m, seg=seg, rings=max(4, seg // 2))
        return self

    def prism(self, profile, depth, at=(0, 0, 0), rot=None, m="white", bev=0.02):
        """Extrude a 2D XZ profile along Y (centred)."""
        pts = [(x, -depth / 2, z) for x, z in profile] + [(x, depth / 2, z) for x, z in profile]
        return self.hull(pts, at, rot, m, bev)

    def arc(self, R, r_in, a0, a1, h, at=(0, 0, 0), rot=None, m="white", seg=12, bev=0.01):
        """Curved shell: the part of a hollow cylinder between angles a0..a1 (degrees)."""
        t = bmesh.new()
        rows = []
        for i in range(seg + 1):
            a = math.radians(a0 + (a1 - a0) * i / seg)
            c, s = math.cos(a), math.sin(a)
            rows.append([t.verts.new((c * rr, s * rr, z)) for rr, z in ((R, 0), (R, h), (r_in, h), (r_in, 0))])
        for i in range(seg):
            a, b = rows[i], rows[i + 1]
            for k in range(4):
                j = (k + 1) % 4
                t.faces.new([a[k], a[j], b[j], b[k]])
        t.faces.new(rows[0][::-1])
        t.faces.new(rows[-1])
        bmesh.ops.recalc_face_normals(t, faces=t.faces)
        bevel_sharp(t, min(bev, (R - r_in) * 0.3), 1, angle=55)
        self._merge(t, m, Matrix.Translation(Vector(at)) @ _rot(rot))
        return self

    def apply(self, rot=None, at=(0, 0, 0)):
        """Transforms everything built so far (e.g. tilt a crashed pod)."""
        self.bm.transform(Matrix.Translation(Vector(at)) @ _rot(rot))
        return self

    def fx_soot(self, strength=0.85, low=0.0, high=1.5, scale=1.6, seed=0.0, color=(0.09, 0.07, 0.06)):
        """Scorch: darkens toward the ground with noisy streaks."""
        def f(p, c, glow):
            if glow:
                return c
            h = 1.0 - min(1.0, max(0.0, (p.z - low) / max(high - low, 1e-3)))
            n = noise.noise(p * scale + Vector((seed, seed * 1.7, 0.3)))
            a = max(0.0, min(1.0, h * 1.2 + n * 0.75 - 0.15)) * strength
            return mix(c, color, a)
        self.fx.append(f)
        return self

    def fx_grime(self, strength=0.35, scale=3.0, seed=0.0):
        def f(p, c, glow):
            if glow:
                return c
            n = noise.noise(p * scale + Vector((seed, 0.0, seed)))
            return shade(c, 1.0 - max(0.0, n) * strength)
        self.fx.append(f)
        return self


class Model:
    def __init__(self, name, ground_ao=True):
        self.name = name
        self.parts = []
        self.ground_ao = ground_ao

    def part(self, name, pivot=(0, 0, 0), parent=None, anim="", smooth_angle=40.0):
        p = Part(self, name, pivot, parent, anim, smooth_angle)
        self.parts.append(p)
        return p

    def get(self, name):
        return next(p for p in self.parts if p.name == name)

    # ------------------------------------------------------------ Blender objects
    def build(self, collection):
        """Creates one Blender object per part, parented, with "Col" corner colours."""
        objs = {}
        for p in self.parts:
            me = bpy.data.meshes.new(f"{self.name}.{p.name}")
            bm = p.bm
            bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
            bm.to_mesh(me)
            me.transform(Matrix.Translation(-p.pivot))
            for m in p.mats:
                me.materials.append(material(m))
            for poly in me.polygons:
                poly.use_smooth = True
            me.set_sharp_from_angle(angle=math.radians(p.smooth_angle))
            attr = me.color_attributes.new("Col", "FLOAT_COLOR", "CORNER")
            cols = []
            for poly in me.polygons:
                mname = p.mats[poly.material_index]
                base, glow = PAL[mname]
                for li in poly.loop_indices:
                    pos = me.vertices[me.loops[li].vertex_index].co + p.pivot
                    c = base
                    for f in p.fx:
                        c = f(pos, c, glow)
                    if self.ground_ao and not glow:
                        k = 0.72 + 0.28 * min(1.0, max(0.0, pos.z / 0.45))
                        c = shade(c, k)
                    cols.append(c)
            for i, c in enumerate(cols):
                attr.data[i].color = (*srgb_to_lin(c), 1.0)
            me["lr_srgb"] = [x for c in cols for x in c]
            ob = bpy.data.objects.new(f"{self.name}.{p.name}", me)
            collection.objects.link(ob)
            ob["lr_part"] = p.name
            ob["lr_anim"] = p.anim
            objs[p.name] = ob
        for p in self.parts:
            ob = objs[p.name]
            if p.parent:
                ob.parent = objs[p.parent]
                ob.location = p.pivot - self.get(p.parent).pivot
            else:
                ob.location = p.pivot
        self.objs = objs
        return objs

    def tri_count(self):
        n = 0
        for ob in self.objs.values():
            ob.data.calc_loop_triangles()
            n += len(ob.data.loop_triangles)
        return n

    def radius(self):
        """Max horizontal distance of any vertex from the origin (footprint check)."""
        r = 0.0
        for p in self.parts:
            for v in p.bm.verts:
                r = max(r, math.hypot(v.co.x, v.co.y))
        return r


# ------------------------------------------------------------------ export
def to_game(v):
    return (-v[0], v[2], v[1])


def anim_to_game(s):
    """Animation tags are authored with Blender axis letters; the game wants its own."""
    w = s.split()
    if len(w) > 1 and w[0] in ("spin", "swing", "slide"):
        w[1] = {"x": "x", "y": "z", "z": "y"}[w[1].lstrip("+-")]
    return " ".join(w)


def _pack_str(s):
    b = s.encode("utf-8")
    return struct.pack("<B", len(b)) + b


def export_models(models, path):
    """LRM1 binary, read by src/models.rs.

    "LRM1" u32 model_count
    model: str name, u16 part_count
    part:  str name, i16 parent, f32x3 offset (game space, relative to parent pivot), str anim,
           2 x submesh (lit, glow): u32 vcount, u32 icount, vcount x (f32x3 pos, i8x4 normal,
           u8x4 sRGB colour), icount x u16 index
    """
    out = bytearray(b"LRM1")
    out += struct.pack("<I", len(models))
    total_tris = 0
    for mdl in models:
        out += _pack_str(mdl.name)
        out += struct.pack("<H", len(mdl.parts))
        names = [p.name for p in mdl.parts]
        for p in mdl.parts:
            ob = mdl.objs[p.name]
            me = ob.data
            me.calc_loop_triangles()
            srgb = me["lr_srgb"]
            normals = me.corner_normals
            subs = [([], [], {}), ([], [], {})]
            for tri in me.loop_triangles:
                mname = p.mats[me.polygons[tri.polygon_index].material_index]
                glow = 1 if PAL[mname][1] else 0
                verts, idx, seen = subs[glow]
                for li in tri.loops:
                    co = me.vertices[me.loops[li].vertex_index].co
                    n = normals[li].vector
                    c = srgb[li * 3: li * 3 + 3]
                    key = (round(co.x, 4), round(co.y, 4), round(co.z, 4),
                           round(n.x, 2), round(n.y, 2), round(n.z, 2),
                           int(c[0] * 255 + 0.5), int(c[1] * 255 + 0.5), int(c[2] * 255 + 0.5))
                    k = seen.get(key)
                    if k is None:
                        k = len(verts)
                        seen[key] = k
                        verts.append(key)
                    idx.append(k)
                total_tris += 1
            parent = names.index(p.parent) if p.parent else -1
            off = p.pivot - (mdl.get(p.parent).pivot if p.parent else Vector())
            out += _pack_str(p.name)
            out += struct.pack("<h", parent)
            out += struct.pack("<3f", *to_game(off))
            out += _pack_str(anim_to_game(p.anim))
            for verts, idx, _ in subs:
                # winding: the axis swap is a rotation, so the order stays valid
                assert len(verts) < 65536, f"{mdl.name}.{p.name} too many vertices"
                out += struct.pack("<II", len(verts), len(idx))
                for k in verts:
                    pos = to_game(k[0:3])
                    nor = to_game(k[3:6])
                    out += struct.pack("<3f", *pos)
                    out += struct.pack("<4b", *[max(-127, min(127, int(round(x * 127)))) for x in nor], 0)
                    out += struct.pack("<4B", k[6], k[7], k[8], 255)
                out += struct.pack(f"<{len(idx)}H", *idx)
    with open(path, "wb") as f:
        f.write(out)
    return len(out), total_tris
