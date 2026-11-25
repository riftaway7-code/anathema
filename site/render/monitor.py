# studio render of a generic pc monitor showing anathema's real boot captures, for the hero of
# anathema.summyt.net. builds the scene from scratch, then renders a turn (the page scrubs it
# with scroll) and two boot stills.
#
#   blender -b -P site/render/monitor.py -- KERNEL.png LOADER.png OUTDIR [--frames N] [--samples S] [--size WxH] [--only K] [--from K]
#   (captures: site/render/captures/kernel.png and loader.png)
#
# outputs OUTDIR/turn_000.png ... (screen on, kernel), OUTDIR/boot_off.png, OUTDIR/boot_loader.png
import bmesh
import bpy
import math
import os
import sys

argv = sys.argv[sys.argv.index("--") + 1:]
KERNEL, LOADER, OUT = argv[0], argv[1], argv[2]
opt = dict(zip(argv[3::2], argv[4::2]))
FRAMES = int(opt.get("--frames", 48))
SAMPLES = int(opt.get("--samples", 160))
RES = [int(v) for v in opt.get("--size", "1600x900").split("x")]
ONLY = opt.get("--only")  # render just one turn frame index, for previews
FROM = int(opt.get("--from", 0))  # resume a turn part way (skips the boot stills)
START_DEG = -38.0

os.makedirs(OUT, exist_ok=True)
bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene

# ---------- render settings ----------
scene.render.engine = "CYCLES"
prefs = bpy.context.preferences.addons["cycles"].preferences
for kind in ("METAL", "OPTIX", "CUDA", "HIP", "ONEAPI"):
    try:
        prefs.compute_device_type = kind
        break
    except TypeError:
        continue
prefs.refresh_devices()
# metal: background kernel specialisation has crashed blender 5.2 mid-run; plain kernels are fine
if hasattr(prefs, "kernel_optimization_level"):
    prefs.kernel_optimization_level = "OFF"
for d in prefs.devices:
    d.use = True
scene.cycles.device = "GPU" if any(d.type != "CPU" for d in prefs.devices) else "CPU"
scene.cycles.samples = SAMPLES
scene.cycles.use_adaptive_sampling = True
scene.cycles.adaptive_threshold = 0.006
scene.cycles.use_denoising = True
scene.cycles.denoiser = "OPENIMAGEDENOISE"
scene.cycles.max_bounces = 10
scene.cycles.glossy_bounces = 6
scene.cycles.transmission_bounces = 6
scene.render.resolution_x, scene.render.resolution_y = RES
scene.render.resolution_percentage = 100
scene.render.film_transparent = True  # the page supplies the black; the floor only catches shadow
scene.render.image_settings.file_format = "PNG"
scene.render.image_settings.color_mode = "RGBA"
scene.render.image_settings.color_depth = "8"
scene.view_settings.view_transform = "AgX"
try:
    scene.view_settings.look = "AgX - Medium High Contrast"
except TypeError:
    pass

world = bpy.data.worlds.new("studio")
scene.world = world
world.use_nodes = True
bg = world.node_tree.nodes["Background"]
bg.inputs["Color"].default_value = (0.0016, 0.0017, 0.0018, 1)
bg.inputs["Strength"].default_value = 1.0


# ---------- materials ----------
def principled(name, **kw):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    p = m.node_tree.nodes["Principled BSDF"]
    for k, v in kw.items():
        p.inputs[k].default_value = v
    return m, p


glass, _ = principled("black glass", **{"Base Color": (0.004, 0.004, 0.005, 1), "Roughness": 0.08, "Coat Weight": 1.0, "Coat Roughness": 0.015})
alu, _ = principled("anodized aluminium", **{"Base Color": (0.66, 0.67, 0.69, 1), "Metallic": 1.0, "Roughness": 0.3, "Anisotropic": 0.55})
dark_alu, _ = principled("dark aluminium", **{"Base Color": (0.16, 0.165, 0.175, 1), "Metallic": 1.0, "Roughness": 0.33, "Anisotropic": 0.4})
floor_mat, _ = principled("studio floor", **{"Base Color": (0.0022, 0.0023, 0.0025, 1), "Roughness": 0.9, "Specular IOR Level": 0.08})

# matte anti-glare panel: shows the capture cleanly instead of mirroring the studio
screen_mat, sp = principled("screen", **{"Base Color": (0, 0, 0, 1), "Roughness": 0.6, "Specular IOR Level": 0.0})
tex = screen_mat.node_tree.nodes.new("ShaderNodeTexImage")
tex.interpolation = "Cubic"
img_kernel = bpy.data.images.load(KERNEL)
img_loader = bpy.data.images.load(LOADER)
tex.image = img_kernel
screen_mat.node_tree.links.new(tex.outputs["Color"], sp.inputs["Emission Color"])
sp.inputs["Emission Strength"].default_value = 2.6


# ---------- geometry ----------
def slab(name, w, h, d, corner, edge, mat, loc=(0, 0, 0), rot=(0, 0, 0), parent=None):
    """a w x h plate in the xz plane, d deep along +y, rounded corners, softened edges"""
    bm = bmesh.new()
    vs = [bm.verts.new((x, 0, z)) for x, z in ((-w / 2, -h / 2), (w / 2, -h / 2), (w / 2, h / 2), (-w / 2, h / 2))]
    bm.faces.new(vs)
    if corner > 0:
        bmesh.ops.bevel(bm, geom=vs, offset=corner, segments=18, affect="VERTICES", profile=0.5)
    ret = bmesh.ops.extrude_face_region(bm, geom=bm.faces[:])
    moved = [g for g in ret["geom"] if isinstance(g, bmesh.types.BMVert)]
    bmesh.ops.translate(bm, vec=(0, d, 0), verts=moved)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces[:])
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new(name, me)
    scene.collection.objects.link(ob)
    ob.location, ob.rotation_euler = loc, rot
    if parent:
        ob.parent = parent
    if edge > 0:
        b = ob.modifiers.new("soft edge", "BEVEL")
        b.width, b.segments, b.limit_method = edge, 5, "ANGLE"
        b.angle_limit = math.radians(40)
        b.harden_normals = True
    for poly in me.polygons:
        poly.use_smooth = True
    ob.data.materials.append(mat)
    return ob


SW, SH = 0.600, 0.3375  # 27 inch, 16:9
BEZ, CHIN, DEPTH = 0.0065, 0.010, 0.011
PW, PH = SW + 2 * BEZ, SH + 2 * BEZ + CHIN
PZ = 0.445  # panel centre height

rig = bpy.data.objects.new("monitor", None)
scene.collection.objects.link(rig)

slab("panel", PW, PH, DEPTH, 0.009, 0.0012, glass, loc=(0, -DEPTH / 2, PZ), parent=rig)
slab("rear shell", PW - 0.004, PH - 0.004, 0.010, 0.008, 0.002, dark_alu, loc=(0, DEPTH / 2 - 0.001, PZ), parent=rig)
slab("rear housing", 0.36, 0.205, 0.024, 0.03, 0.006, dark_alu, loc=(0, DEPTH / 2 + 0.008, PZ - 0.01), parent=rig)
slab("neck", 0.062, 0.34, 0.012, 0.006, 0.002, alu, loc=(0, 0.052, 0.008 + 0.17), rot=(math.radians(-5), 0, 0), parent=rig)
slab("base", 0.235, 0.175, 0.007, 0.04, 0.0025, alu, loc=(0, 0.028, 0.0), rot=(math.radians(90), 0, 0), parent=rig)

# the screen: a plane flush with the glass, uv mapped to the capture
bpy.ops.mesh.primitive_plane_add(size=1)
scr = bpy.context.active_object
scr.name = "screen"
scr.scale = (SW, SH, 1)
scr.rotation_euler = (math.radians(90), 0, 0)
scr.location = (0, -DEPTH / 2 - 0.0004, PZ + CHIN / 2)
scr.data.materials.append(screen_mat)
scr.parent = rig

# studio floor
bpy.ops.mesh.primitive_plane_add(size=30, location=(0, 0, 0))
floor = bpy.context.active_object
floor.data.materials.append(floor_mat)
floor.is_shadow_catcher = True

# ---------- lights ----------
target = bpy.data.objects.new("aim", None)
scene.collection.objects.link(target)
target.location = (0, 0, 0.315)


def area(name, loc, size, size_y, power, aim=True, color=(1, 1, 1), spread=180):
    li = bpy.data.lights.new(name, "AREA")
    li.shape = "RECTANGLE"
    li.spread = math.radians(spread)  # narrower spread keeps light off the floor
    li.size, li.size_y, li.energy, li.color = size, size_y, power, color
    ob = bpy.data.objects.new(name, li)
    scene.collection.objects.link(ob)
    ob.location = loc
    if aim:
        c = ob.constraints.new("TRACK_TO")
        c.target, c.track_axis, c.up_axis = target, "TRACK_NEGATIVE_Z", "UP_Y"
    return ob


area("softbox top", (0, -0.15, 1.45), 1.1, 0.75, 130, aim=False, spread=55)
area("key left", (-1.35, -1.45, 0.95), 1.3, 0.7, 170, spread=40)
area("rim right", (1.05, 0.75, 0.62), 0.08, 1.5, 95)
area("rim left", (-1.05, 0.75, 0.62), 0.08, 1.5, 70)
area("fill low", (0.2, -1.9, 0.45), 2.2, 0.25, 6, spread=35)
area("edge top", (0, 0.5, 1.2), 1.2, 0.06, 60)

# ---------- camera ----------
cam_data = bpy.data.cameras.new("camera")
cam_data.lens, cam_data.sensor_width = 70, 36
cam = bpy.data.objects.new("camera", cam_data)
scene.collection.objects.link(cam)
cam.location = (0, -2.5, 0.47)
c = cam.constraints.new("TRACK_TO")
c.target, c.track_axis, c.up_axis = target, "TRACK_NEGATIVE_Z", "UP_Y"
scene.camera = cam


# ---------- render ----------
def shoot(path):
    scene.render.filepath = path
    bpy.ops.render.render(write_still=True)


def turn(i):
    t = i / max(FRAMES - 1, 1)
    rig.rotation_euler = (0, 0, math.radians(START_DEG * (1 - t)))


if ONLY is not None:
    turn(int(ONLY))
    shoot(os.path.join(OUT, "preview.png"))
    sys.exit(0)

if FROM == 0:
    turn(0)
    sp.inputs["Emission Strength"].default_value = 0.0
    shoot(os.path.join(OUT, "boot_off.png"))
    sp.inputs["Emission Strength"].default_value = 2.6
    tex.image = img_loader
    shoot(os.path.join(OUT, "boot_loader.png"))
    tex.image = img_kernel
for i in range(FROM, FRAMES):
    turn(i)
    shoot(os.path.join(OUT, "turn_%03d.png" % i))
