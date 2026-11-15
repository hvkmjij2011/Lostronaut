"""Lostranout asset build: every model, the embedded model pack and all icons.

Regenerate everything (run from the repository root):

    tools/blender/build.sh                 # models + icons + contact sheets
    tools/blender/build.sh --only Smelter  # preview a single model (no export)

which runs
    flatpak run org.blender.Blender -b --factory-startup -P $PWD/tools/blender/build_assets.py -- [options]

Options after "--":
    --only NAME[,NAME]  build / preview only these models (skips the pack and icons)
    --no-icons          skip icon renders
    --no-sheet          skip the contact-sheet renders
    --no-export         do not write assets/models/models.lrm

Outputs:
    assets/models/models.lrm        binary model pack, embedded by src/models.rs
    assets/icons/item_<Item>.png    128 px icons, one per Item variant
    assets/icons/sk_<SK>.png        128 px icons, one per SK variant
    tools/blender/build/shots/*.png large three-quarter preview of every model
"""
import os
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))

import bpy  # noqa: E402

import kit  # noqa: E402
import render  # noqa: E402
import structures  # noqa: E402

try:
    import astronaut  # noqa: E402
except ImportError:
    astronaut = None
try:
    import items  # noqa: E402
except ImportError:
    items = None

# footprint radius per structure (src/items.rs StructDef::radius)
RADIUS = {
    "Pod": 2.2, "Smelter": 1.4, "Research": 1.4, "Fabricator": 1.5, "Beacon": 0.5, "BioGen": 1.0,
    "Storage": 1.0, "Solar": 1.2, "Battery": 0.8, "ChemLab": 1.5, "Shuttle": 1.8, "Rtg": 1.0,
    "Shipyard": 4.0, "Tether": 0.25, "Pylon": 0.3,
}


def args():
    a = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    opt = {"only": None, "icons": True, "sheet": True, "export": True}
    i = 0
    while i < len(a):
        if a[i] == "--only":
            opt["only"] = a[i + 1].split(",")
            i += 1
        elif a[i] == "--no-icons":
            opt["icons"] = False
        elif a[i] == "--no-sheet":
            opt["sheet"] = False
        elif a[i] == "--no-export":
            opt["export"] = False
        i += 1
    if opt["only"]:
        opt["icons"] = False
        opt["export"] = False
    return opt


def all_builders():
    b = dict(structures.STRUCTURES)
    if astronaut:
        b.update(astronaut.MODELS)
    return b


def main():
    t0 = time.time()
    opt = args()
    for ob in list(bpy.data.objects):
        bpy.data.objects.remove(ob)
    cam = render.setup_scene()
    builders = all_builders()
    names = opt["only"] or list(builders)
    models, colls = [], {}
    for name in names:
        mdl = builders[name]()
        coll = bpy.data.collections.new(name)
        bpy.context.scene.collection.children.link(coll)
        mdl.build(coll)
        models.append(mdl)
        colls[name] = coll
        r = mdl.radius()
        lim = RADIUS.get(name)
        flag = "" if lim is None or r <= lim + 1e-3 else f"  !! exceeds footprint {lim}"
        print(f"[model] {name:14s} parts={len(mdl.parts):2d} tris={mdl.tri_count():6d} radius={r:.2f}{flag}")
    if opt["export"]:
        path = os.path.join(ROOT, "assets", "models", "models.lrm")
        os.makedirs(os.path.dirname(path), exist_ok=True)
        size, tris = kit.export_models(models, path)
        print(f"[export] {path}: {size / 1024:.0f} KiB, {tris} triangles")
    if opt["sheet"]:
        shots = os.path.join(HERE, "build", "shots")
        os.makedirs(shots, exist_ok=True)
        for mdl in models:
            if getattr(mdl, "no_shot", False):
                continue
            render.show_only(list(colls.values()), [colls[mdl.name]])
            render.frame(cam, list(mdl.objs.values()), direction=getattr(mdl, "shot_dir", (0.95, -1.3, 0.8)))
            render.render(os.path.join(shots, f"{mdl.name}.png"), 420)
    if opt["icons"] and items:
        items.render_icons(cam, models, colls, ROOT)
    print(f"[done] {time.time() - t0:.1f}s")


main()
