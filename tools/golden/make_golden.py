"""Make the arrays Volsa's tests are held against, with SynthSeg's own Python.

    python make_golden.py SYNTHSEG_CHECKOUT [GROUP ...]

SYNTHSEG_CHECKOUT is a copy of github.com/BBillot/SynthSeg; no weights are needed. Each group
of cases is written under crates/volsa-segment/tests/golden/GROUP/. With no group named, every
group is made. README.md beside this file says what each case holds and in what order.

Every volume here is made up from a seed, so a second run writes the same bytes.

This script ships nothing and is no part of Volsa: it is not built, run or tested by cargo.
"""

import json
import math
import sys
import tempfile
import zlib
from pathlib import Path

import numpy as np

TOOLS = Path(__file__).resolve().parents[1]
GOLDEN = TOOLS.parent / "crates" / "volsa-segment" / "tests" / "golden"

sys.path.insert(0, str(TOOLS / "synthseg-to-onnx"))
from convert import use_synthseg  # noqa: E402  (how SynthSeg's Keras 2 code is made importable)

VOLUME_HOLDS = ("little-endian f32; columns fastest, then rows, then slices: "
                "values[(slice * rows + row) * cols + col]")
IMAGE_HOLDS = ("little-endian f32; the image before padding, as the network's [x, y, z] with z fastest: "
               "values[(x * size_y + y) * size_z + z]")
TO_PATIENT = ("voxel (column, row, slice) to the patient in left-posterior-superior millimetres, as Volsa holds it: "
              "where a step along each axis leads, and where voxel (0, 0, 0) lies. SynthSeg was given the same "
              "with the x and y rows negated (right-anterior-superior). The transform a case starts from holds "
              "32-bit floats, as a NIfTI file does: take each number as one. The transform of a resampled volume "
              "is SynthSeg's result, in double precision.")


# ---------------------------------------------------------------- what every group shares

def seeded(group: str, case: str):
    """A source of random numbers that is the same on every run, and its own for each case."""
    return np.random.default_rng([zlib.crc32(group.encode()), zlib.crc32(case.encode())])


def made_up(rng, shape) -> np.ndarray:
    """Blobs and noise: a few soft balls of different brightness on a dim, noisy ground.
    Indexed [column, row, slice], 32-bit floats. It is nobody's scan."""
    grid = np.stack(np.meshgrid(*[np.arange(n) for n in shape], indexing="ij"), axis=-1)
    volume = np.full(shape, 20.0)
    for _ in range(5):
        centre = rng.uniform(0.2, 0.8, 3) * shape
        radius = rng.uniform(0.15, 0.35) * min(shape)
        volume += rng.uniform(100, 900) * np.exp(-np.sum((grid - centre) ** 2, axis=-1) / (2 * radius ** 2))
    volume += rng.normal(0, 15, shape)
    return volume.astype(np.float32)


def affine(linear, origin) -> np.ndarray:
    """A 4 x 4 transform, right-anterior-superior, with the numbers a NIfTI file can hold."""
    aff = np.eye(4)
    aff[:3, :3] = linear
    aff[:3, 3] = origin
    return np.float32(aff).astype(np.float64)


def to_patient(aff_ras: np.ndarray) -> dict:
    """SynthSeg's right-anterior-superior transform as Volsa holds one: left-posterior-superior."""
    lps = np.diag([-1.0, -1.0, 1.0, 1.0]) @ aff_ras
    numbers = lambda column: [float(v) + 0.0 for v in column]  # + 0.0: no minus zero in the file
    return {"columns": [numbers(lps[:3, c]) for c in range(3)], "origin": numbers(lps[:3, 3])}


def write_floats(path: Path, array: np.ndarray, fastest: str) -> None:
    """`fastest` is "first" for a volume in Volsa's order, "last" for a network's image."""
    path.write_bytes(np.asarray(array).astype("<f4").tobytes(order="F" if fastest == "first" else "C"))


def write_json(path: Path, content: dict) -> None:
    with open(path, "w", encoding="utf-8", newline="\n") as file:
        file.write(json.dumps(content, indent=1) + "\n")


def made_with() -> dict:
    import nibabel
    import scipy

    return {"numpy": np.__version__, "scipy": scipy.__version__, "nibabel": nibabel.__version__}


def turn_of(aff_ras: np.ndarray, edit_volumes) -> tuple:
    """How SynthSeg turns a volume placed by `aff_ras`: for each of its x, y and z the volume's
    axis it takes, and whether it turns that axis round. The order is SynthSeg's own answer;
    the directions are read off as its align_volume_to_ref reads them, and both are checked
    against what that function does to a small block of numbers."""
    order = [int(a) for a in edit_volumes.get_ras_axes(aff_ras)]
    turned_round = [bool(aff_ras[j, order[j]] < 0) for j in range(3)]
    block = np.arange(4 * 5 * 6, dtype=np.float64).reshape(4, 5, 6)
    theirs = edit_volumes.align_volume_to_ref(block, aff_ras, aff_ref=np.eye(4), n_dims=3, return_copy=True)
    ours = np.transpose(block, order)
    for axis in range(3):
        if turned_round[axis]:
            ours = np.flip(ours, axis=axis)
    if not np.array_equal(theirs, ours):
        raise SystemExit(f"The turn written down is not the one SynthSeg makes, for\n{aff_ras}")
    return order, turned_round


# ---------------------------------------------------------------- the group "prepare"

def turned_about_x(degrees: float) -> np.ndarray:
    c, s = math.cos(math.radians(degrees)), math.sin(math.radians(degrees))
    return np.array([[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]])


def along(columns, rows, slices) -> np.ndarray:
    """Where a step along the columns, the rows and the slices leads, right-anterior-superior."""
    return np.column_stack([columns, rows, slices]).astype(np.float64)


# name: (what it is, (columns, rows, slices), the steps in right-anterior-superior mm, where voxel 0 lies)
PREPARE = {
    "iso": ("1 mm, stored as SynthSeg wants it: columns to the right, rows to the front, slices to the top",
            (20, 12, 16), np.eye(3), (-9.75, -6.25, -8.0)),
    "sagittal": ("1 mm sagittal slices: columns to the front, rows to the top, slices to the left (turned round)",
                 (16, 20, 12), along([0, 1, 0], [0, 0, 1], [-1, 0, 0]), (6.0, -8.0, -9.25)),
    "coronal": ("1 mm coronal slices: columns to the right, rows to the top, slices to the back (turned round)",
                (12, 20, 16), along([1, 0, 0], [0, 0, 1], [0, -1, 0]), (-6.0, 8.0, -9.75)),
    "tilted": ("1 mm, turned 10 degrees about x",
               (16, 12, 20), turned_about_x(10.0), (-8.0, -6.25, -9.25)),
    "thick": ("1 x 1 x 4 mm, as a scanner stores axial slices: columns to the left, rows to the back",
              (20, 16, 12), along([-1, 0, 0], [0, -1, 0], [0, 0, 4]), (9.75, 8.0, -23.5)),
    "fine": ("0.75 mm: columns to the front, rows to the left, slices to the top",
             (32, 20, 16), along([0, 0.75, 0], [-0.75, 0, 0], [0, 0, 0.75]), (6.75, -11.75, -6.0)),
    "uneven": ("0.28125 x 1.08 x 2.7 mm, turned 7 degrees about x. Along the columns SynthSeg's rule gives one "
               "sample more than the spacing times the count, rounded up. The rows, a little over 1 mm apart, "
               "are where a blur would show if one were made there",
               (16, 10, 12), turned_about_x(-7.0) @ np.diag([0.28125, 1.08, 2.7]), (-6.25, -8.5, -15.75)),
}


def prepare_case(name: str, out: Path, folder: str, edit_volumes, preprocess) -> None:
    import nibabel as nib

    what, shape, linear, origin = PREPARE[name]
    volume = made_up(seeded("prepare", name), shape)
    path = str(Path(folder) / f"{name}.nii")
    nib.save(nib.Nifti1Image(volume, affine(linear, origin)), path)
    read = nib.load(path)
    aff = np.array(read.affine, dtype=np.float64)  # what SynthSeg is given
    if not np.array_equal(np.asarray(read.dataobj), volume):
        raise SystemExit(f"{name}: the NIfTI file does not hold the made-up values as they were.")

    # SynthSeg's own preparing, as its predict() calls it.
    image, aff_after, _, _, size, pad, crop = preprocess(path_image=path, ct=False, crop=None, min_pad=128)
    image = image[0, ..., 0]
    size, before = [int(s) for s in size], [int(p) for p in pad[:3]]
    core = image[tuple(slice(b, b + s) for b, s in zip(before, size))]
    if crop is not None or np.count_nonzero(image) != np.count_nonzero(core):
        raise SystemExit(f"{name}: the image is not its middle with zeros around it.")
    order, turned_round = turn_of(aff_after, edit_volumes)

    out.mkdir(parents=True, exist_ok=True)
    case = {"what": what, "made_with": made_with(), "to_patient": TO_PATIENT,
            "volume": {"file": "volume.f32", "holds": VOLUME_HOLDS, "dims": list(shape), "to_patient": to_patient(aff)},
            "resampled": None}
    write_floats(out / "volume.f32", volume, "first")
    if not np.array_equal(aff_after, aff):  # SynthSeg resampled it
        at_1mm, aff_1mm = edit_volumes.resample_volume(np.asarray(read.get_fdata()), aff, np.ones(3))
        if not np.array_equal(aff_1mm, aff_after):
            raise SystemExit(f"{name}: resample_volume alone does not give what preprocess did.")
        case["resampled"] = {"file": "resampled.f32", "holds": VOLUME_HOLDS, "dims": list(at_1mm.shape),
                             "to_patient": to_patient(aff_1mm)}
        write_floats(out / "resampled.f32", at_1mm, "first")
        check_uneven(name, shape, aff, at_1mm.shape)
    elif name in ("thick", "fine", "uneven"):
        raise SystemExit(f"{name} was to be resampled and was not.")
    case["image"] = {"file": "image.f32", "holds": IMAGE_HOLDS, "size": size, "padded": list(image.shape),
                     "padding_before": before, "order": order, "turned_round": turned_round}
    write_floats(out / "image.f32", core, "last")
    write_json(out / "case.json", case)
    print(f"prepare/{name}: {tuple(shape)} gives {tuple(size)} in {image.shape}, order {order}, "
          f"turned round {turned_round}")


def check_uneven(name: str, shape, aff: np.ndarray, shape_1mm) -> None:
    """`uneven` is there to show that SynthSeg's count of samples can pass the spacing times the
    count, rounded up. Say so if it stops showing it."""
    if name != "uneven":
        return
    spacing = np.sqrt(np.sum(aff[:3, :3] ** 2, axis=0))
    expected = [math.ceil(n * f) for n, f in zip(shape, spacing)]
    more = [got - want for got, want in zip(shape_1mm, expected)]
    if sorted(more) != [0, 0, 1]:
        raise SystemExit(f"uneven: samples beyond the spacing times the count are {more}; one axis was to have one.")


def turns(out: Path, edit_volumes) -> None:
    """Twelve made-up transforms and how SynthSeg turns a volume placed by each: unequal steps,
    slanted axes, and several where two of SynthSeg's choices fall on the same axis at first."""
    rng = seeded("prepare", "turns")
    kept, kinds = [], set()
    while len(kept) < 12:
        linear = rng.normal(size=(3, 3))
        if len(kept) % 2 == 0:  # every other one a true rotation with unequal steps
            linear = np.linalg.qr(linear)[0] @ np.diag(rng.uniform(0.3, 6.0, 3))
        aff = affine(linear, rng.normal(size=3) * 50)
        steps = np.sqrt(np.sum(aff[:3, :3] ** 2, axis=0))
        if abs(np.linalg.det(aff[:3, :3])) < 0.2 * np.prod(steps):
            continue  # too flat to be a volume's transform
        first = tuple(int(np.argmax(np.abs(np.linalg.inv(aff)[:3, j]))) for j in range(3))
        if first in kinds:
            continue  # one of each kind of first choice
        kinds.add(first)
        order, turned_round = turn_of(aff, edit_volumes)
        kept.append({"to_patient": to_patient(aff), "order": order, "turned_round": turned_round})
    mended = sum(len(set(kind)) < 3 for kind in kinds)
    write_json(out / "turns.json", {"what": " ".join(turns.__doc__.split()), "made_with": made_with(),
                                    "to_patient": TO_PATIENT, "turns": kept})
    print(f"prepare/turns: {len(kept)} transforms, {mended} where SynthSeg mends its first choice")


def prepare_group(checkout: Path, out: Path) -> None:
    """SynthSeg's preparing of a volume: resample, turn, scale, pad."""
    from ext.lab2im import edit_volumes
    from SynthSeg.predict_synthseg import preprocess

    with tempfile.TemporaryDirectory() as folder:
        for name in PREPARE:
            prepare_case(name, out / name, folder, edit_volumes, preprocess)
    turns(out, edit_volumes)


# ---------------------------------------------------------------- the group "box"

BOX_SIDE = 8  # the side of the box in these cases; the one SynthSeg's scores network is given is 224
STRUCTURES = 33  # a voxel's top structure is a place from 0 to 32
NOT_COUNTED = (0, 24)  # the two places SynthSeg's layer leaves out when it looks for where the structures lie

PLACES_HOLD = ("one byte a voxel: the place of the voxel's top structure, as the network's [x, y, z] with z fastest: "
               "values[(x * size_y + y) * size_z + z]")
BOX_HOLDS = ("one byte a voxel: a place, or 0 where the box lies outside the grid; [x, y, z] with z fastest and "
             "`side` voxels along each: values[(x * side + y) * side + z]")

# name: (what it is, the grid's size along x, y and z, where the blob's block begins and how long it is along each)
BOXES = {
    "middle": ("a blob in the middle of the grid: the box is cut out round it, with one voxel more before the blob "
               "than after it where the room to share is odd", (16, 12, 20), ((6, 4, 7), (3, 4, 5))),
    "corner": ("a blob in a corner, at the start of x and the end of y and z: the box runs out of the grid on "
               "three sides, and is 0 there", (16, 12, 20), ((0, 9, 17), (3, 3, 3))),
    "longer": ("a blob longer than the box along x and z: the box keeps the blob's middle, with one voxel more cut "
               "from its end than from its start where what hangs over is odd", (16, 12, 20), ((2, 4, 3), (11, 4, 13))),
    "none": ("no blob, only the two places that are not counted: the box is the start of the grid along x and z, "
             "and along y, where the grid is shorter than the box, the grid lies in its middle", (12, 6, 20), None),
}


def write_bytes(path: Path, array: np.ndarray) -> None:
    """One byte a number, the last index fastest."""
    array = np.asarray(array)
    if array.min() < 0 or array.max() > 255:
        raise SystemExit(f"{path.name}: a number that is not a byte.")
    path.write_bytes(array.astype(np.uint8).tobytes(order="C"))


def top_structures(rng, shape, blob) -> np.ndarray:
    """A made-up map of top structures, indexed [x, y, z]. Outside the blob it is place 0 with a sprinkling of
    place 24, the two places that are not counted. The blob is a ball of the other places that touches every face
    of its block, so what counts lies in exactly that block."""
    places = np.where(rng.random(shape) < 0.125, 24, 0)
    if blob is not None:
        start, size = np.array(blob[0]), np.array(blob[1])
        grid = np.stack(np.meshgrid(*[np.arange(n) for n in shape], indexing="ij"), axis=-1)
        inside = np.sum(((grid - (start + (size - 1) / 2)) / (size / 2)) ** 2, axis=-1) <= 1
        counted = [place for place in range(STRUCTURES) if place not in NOT_COUNTED]
        places[inside] = rng.choice(counted, size=int(inside.sum()))
        where = np.argwhere(inside)
        if not (np.array_equal(where.min(axis=0), start) and np.array_equal(where.max(axis=0) + 1, start + size)):
            raise SystemExit(f"The blob does not touch every face of its block, {blob}.")
    return places.astype(np.int32)


def check_box(name: str, places: np.ndarray, box: np.ndarray, blob) -> None:
    """Say so if a case stops showing what it is there for: how much of the blob the box holds and where the box
    is 0 and, with no blob, that the box is the start of the grid with the grid's short side in its middle."""
    counted = lambda array: int(np.count_nonzero(~np.isin(array, NOT_COUNTED)))
    side = np.array(places.shape)
    if box.shape != (BOX_SIDE,) * 3:
        raise SystemExit(f"{name}: the box is {box.shape}.")
    if blob is None:
        # The grid is longer than the box along x and z, and two voxels shorter along y: a slab of zeros before
        # the grid and one after, and between them the start of the grid as it is.
        whole = (counted(places) == 0 and np.all(side[[0, 2]] > BOX_SIDE) and side[1] == BOX_SIDE - 2
                 and not box[:, [0, -1], :].any() and np.array_equal(box[:, 1:-1, :], places[:BOX_SIDE, :, :BOX_SIDE]))
    else:
        longer = np.array(blob[1]) > BOX_SIDE
        # Where the box runs out of the grid it is 0: before the grid along x, after it along y and z.
        off_the_grid = not (box[0].any() or box[:, -1].any() or box[:, :, -1].any())
        whole = {"middle": counted(box) == counted(places) and not longer.any(),
                 "corner": counted(box) == counted(places) and not longer.any() and off_the_grid,
                 "longer": 0 < counted(box) < counted(places) and longer.sum() == 2}[name]
    if not whole:
        raise SystemExit(f"{name}: the case no longer shows what it is there for.")


def box_group(checkout: Path, out: Path) -> None:
    """The box SynthSeg cuts from a map of top structures for its scores network: its layer MakeShape."""
    import tensorflow as tf
    from SynthSeg.predict_synthseg import MakeShape

    layer = MakeShape(BOX_SIDE)
    for name, (what, shape, blob) in BOXES.items():
        places = top_structures(seeded("box", name), shape, blob)
        # As SynthSeg's build_model feeds its layer: the map and the grid's size, each with an axis for the batch.
        box = layer([tf.constant(places[np.newaxis]), tf.constant([shape], dtype=tf.int32)]).numpy()[0]
        check_box(name, places, box, blob)
        (out / name).mkdir(parents=True, exist_ok=True)
        write_bytes(out / name / "places.u8", places)
        write_bytes(out / name / "box.u8", box)
        write_json(out / name / "case.json", {
            "what": what, "made_with": {"numpy": np.__version__, "tensorflow": tf.__version__},
            "places": {"file": "places.u8", "holds": PLACES_HOLD, "size": list(shape), "structures": STRUCTURES},
            "box": {"file": "box.u8", "holds": BOX_HOLDS, "side": BOX_SIDE}})
        print(f"box/{name}: {tuple(shape)} gives a box of {box.shape}, {int(np.count_nonzero(box))} of its voxels "
              f"not 0")


# ---------------------------------------------------------------- the group "tidy"

CHANCES_HOLD = ("little-endian f32; the chance of each structure at each voxel of the padded grid, as a network's "
                "[x, y, z, structure] with the structure fastest: "
                "values[((x * padded_y + y) * padded_z + z) * structures + structure]")
AREAS_HOLD = ("one byte a voxel of the padded grid: the cortex area there as Volsa's cortex network gives it, 1 plus "
              "the place of the largest of the chances of areas 1 to 68; [x, y, z] with z fastest: "
              "values[(x * padded_y + y) * padded_z + z]")
LABELS_HOLD = ("little-endian i32; SynthSeg's label at each voxel of the grid as stored: columns fastest, then rows, "
               "then slices: values[(slice * rows + row) * cols + col]")
QUARTER = np.float32(0.25)  # what SynthSeg's tidying keeps is above this

MARK = np.float32(0.2)  # with `fast`, a chance other than the first must be above this to be kept

# name: what it is; where a step along the columns, rows and slices leads (right-anterior-superior); the size of the
# grid once turned, along x, y and z; where a cut begins and how long it is, if there is one; the zeros before the
# image; the padded size; and whether SynthSeg's tidying is told `fast`, as its own program tells it for the robust
# networks. SynthSeg pads to 128 at least: the rule is the same on these, and the files are small.
TIDY = {
    "pieces": dict(
        what="a block of white matter with other structures in it, some in two pieces: the smaller piece of a class "
             "goes, pieces are joined by faces only, of two as large the first met is kept, a chance of exactly 0.25 "
             "joins nothing, and of two chances that are equal the first wins",
        steps=np.eye(3), whole=(8, 10, 12), cut=None, before=(1, 1, 2), padded=(10, 12, 16)),
    "speck": dict(
        what="a blob with specks and blobs round it, in faint noise: what is not joined to the largest piece goes, "
             "a voxel joins when its chances add to more than 0.25 though none is over it, and a piece may be joined "
             "through the padding. Stored as sagittal slices",
        steps=along([0, 1, 0], [0, 0, 1], [-1, 0, 0]), whole=(6, 8, 10), cut=None, before=(1, 1, 1),
        padded=(8, 10, 12)),
    "cortex": dict(
        what="white matter with cortex on both sides, in faint noise, and an area for every voxel: the cortex takes "
             "the areas' labels and nothing else does. A box cut from a larger grid, stored as coronal slices: "
             "outside the box every label is 0",
        steps=along([1, 0, 0], [0, 0, 1], [0, -1, 0]), whole=(10, 12, 16), cut=((2, 3, 4), (6, 8, 10)),
        before=(1, 1, 1), padded=(8, 10, 12)),
    "close": dict(
        what="chances chosen to fall on the edge of SynthSeg's own arithmetic: sums that are over 0.25 only when added "
             "in NumPy's order, or only when not; two chances one step apart that become equal when divided by "
             "their voxel's sum, and two that do not; a voxel of zeros; and a voxel whose chances add to nought",
        steps=along([-1, 0, 0], [0, -1, 0], [0, 0, 1]), whole=(6, 8, 10), cut=None, before=(1, 1, 1),
        padded=(8, 10, 12)),
    "robust_pieces": dict(
        what="tidied the fast way, as SynthSeg's program tidies what its robust networks give: a blob with a "
             "structure in two pieces, which both stay, since no piece of a class is looked for; and a piece joined "
             "to the blob only through the padding, which goes, since the padding is taken off first. Stored as "
             "sagittal slices",
        steps=along([0, 1, 0], [0, 0, 1], [-1, 0, 0]), whole=(6, 6, 8), cut=None, before=(1, 1, 1),
        padded=(8, 8, 10), fast=True),
    "robust_marks": dict(
        what="tidied the fast way: voxels in which a chance of 0.22 is the only one over 0.2 and so wins, a chance "
             "of exactly 0.2 goes and the next number above it stays, and nothing is left when no chance is over "
             "0.2; with sums and divisions on the edge of the arithmetic, as in `close`",
        steps=along([-1, 0, 0], [0, -1, 0], [0, 0, 1]), whole=(6, 6, 8), cut=None, before=(1, 1, 1),
        padded=(8, 8, 10), fast=True),
    "robust_cortex": dict(
        what="tidied the fast way: white matter with cortex on both sides, in faint noise, and an area for every "
             "voxel; and two voxels of left cortex apart from the rest, in the right white matter, which stay and "
             "take their areas. A box cut from a larger grid, stored as coronal slices",
        steps=along([1, 0, 0], [0, 0, 1], [0, -1, 0]), whole=(8, 10, 12), cut=((1, 2, 3), (6, 6, 8)),
        before=(1, 1, 1), padded=(8, 8, 10), fast=True),
}


class Scene:
    """Made-up chances on a padded grid. A place is given in the grid before padding, as [x, y, z]. Each block says
    what SynthSeg must make of it, so the script stops if a case no longer shows what it is there for."""

    def __init__(self, lists: dict, case: dict, rng, noise: float = 0.0):
        self.place = {int(label): place for place, label in enumerate(lists["labels"])}
        self.before, self.rng = np.array(case["before"]), rng
        self.chances = np.zeros((*case["padded"], len(self.place)), np.float32)
        self.chances[..., 1:] = noise * rng.random(self.chances[..., 1:].shape, dtype=np.float32)
        self.chances[..., 0] = self.rest(self.chances)
        self.taken = np.zeros(case["padded"], bool)
        self.grounds, self.sure = [], []  # (where, the label SynthSeg must give there)
        self.areas = self.area_chances = None  # only a scene with cortex areas has them

    @staticmethod
    def rest(chances: np.ndarray) -> np.ndarray:
        """What is left for the first structure, which is nothing, when the others have had theirs."""
        return np.maximum(np.float32(1) - chances[..., 1:].sum(axis=-1, dtype=np.float32), np.float32(0))

    def where(self, start, size) -> tuple:
        return tuple(slice(int(b + s), int(b + s + n)) for b, s, n in zip(self.before, start, size))

    def fill(self, where: tuple, chances: dict, noisy: bool) -> None:
        """Gives every voxel of a block these chances, by label. A noisy block keeps the noise of its other
        structures, has each chance lowered by up to a twelfth, and nothing takes the rest; a plain one holds
        exactly what is given, and nothing takes the rest unless its chance is given too."""
        block = self.chances[where]
        if not noisy:
            block[...] = 0
        for label, chance in chances.items():
            lowered = 1 - self.rng.random(block.shape[:3], dtype=np.float32) / 12 if noisy else 1
            block[..., self.place[label]] = np.float32(chance) * lowered
        if noisy or 0 not in chances:
            block[..., 0] = self.rest(block)

    def ground(self, start, size, chances: dict, becomes: int, noisy: bool = False) -> None:
        """A block that others may be put into: what is left of it must become `becomes`."""
        where = self.where(start, size)
        self.fill(where, chances, noisy)
        self.grounds.append((where, becomes))

    def block(self, start, size, chances: dict, becomes: int | None, noisy: bool = False) -> None:
        """A block of its own, which no other block may share a voxel with. `becomes` is the label SynthSeg must
        give all of it; None when the case says so itself."""
        where = self.where(start, size)
        if self.taken[where].any():
            raise SystemExit("Two blocks of a scene share a voxel.")
        self.taken[where] = True
        self.fill(where, chances, noisy)
        if becomes is not None:
            self.sure.append((where, becomes))

    def voxel(self, at, values: np.ndarray, becomes: int) -> None:
        """One voxel with exactly these chances, in the networks' order."""
        self.block(at, (1, 1, 1), {}, becomes)
        self.chances[self.where(at, (1, 1, 1))] = values

    def check(self, name: str, image: np.ndarray, cut_from) -> None:
        """Holds SynthSeg's labels, turned to the image's axes, against what each block said."""
        shift = np.array(cut_from) - self.before
        for is_ground, blocks in ((True, self.grounds), (False, self.sure)):
            for where, becomes in blocks:
                made = image[tuple(slice(int(s.start + d), int(s.stop + d)) for s, d in zip(where, shift))]
                # Of a ground, only what no other block took.
                mine = ~self.taken[where] if is_ground else np.ones(made.shape, bool)
                if not np.array_equal(made[mine], np.broadcast_to(becomes, made.shape)[mine]):
                    raise SystemExit(f"{name}: SynthSeg makes {np.unique(made[mine])} of the block at "
                                     f"{[s.start for s in where]}, which was to become {np.unique(becomes)}.")


def pieces_scene(scene: Scene) -> None:
    """Each structure put into the white matter has a class of its own, so each shows one rule. A piece that goes
    becomes white matter, the next most likely there."""
    scene.ground((0, 0, 0), (8, 10, 12), {2: 0.75}, 2)
    part = lambda label: {label: 0.5625, 2: 0.3125}
    # Left thalamus in two pieces of 12 and 4 voxels: the smaller goes.
    scene.block((0, 0, 0), (2, 2, 3), part(10), 10)
    scene.block((0, 0, 5), (2, 2, 1), part(10), 2)
    # Left caudate, 5 voxels and 3 that touch them at a corner only; left accumbens, 4 and 2 that touch along an edge.
    scene.block((3, 0, 0), (1, 1, 5), part(11), 11)
    scene.block((4, 1, 5), (1, 1, 3), part(11), 2)
    scene.block((6, 0, 0), (1, 1, 4), part(26), 26)
    scene.block((7, 1, 2), (1, 1, 2), part(26), 2)
    # Left putamen, twice 3 voxels: the one met first, x by x, then y, then z, is kept. Slice by slice as the grid is
    # stored, the other would be met first.
    scene.block((1, 8, 8), (1, 1, 3), part(12), 12)
    scene.block((6, 3, 0), (1, 1, 3), part(12), 2)
    # Left pallidum, 8 voxels and 2, joined by a voxel where white matter is the likelier: both are kept.
    scene.block((3, 4, 0), (2, 2, 2), part(13), 13)
    scene.block((3, 4, 2), (1, 1, 1), {13: 0.3125, 2: 0.5625}, 2)
    scene.block((3, 4, 3), (1, 1, 2), part(13), 13)
    # Left hippocampus, 8 voxels and 1, with a voxel of exactly 0.25 between: not joined. Left amygdala, the same
    # with the next number above 0.25: joined.
    scene.block((0, 4, 8), (2, 2, 2), part(17), 17)
    scene.block((0, 4, 10), (1, 1, 1), {17: 0.25, 2: 0.5}, 2)
    scene.block((0, 4, 11), (1, 1, 1), part(17), 2)
    scene.block((6, 6, 8), (2, 2, 2), part(18), 18)
    scene.block((6, 6, 10), (1, 1, 1), {18: np.nextafter(QUARTER, np.float32(1)), 2: 0.5}, 2)
    scene.block((6, 6, 11), (1, 1, 1), part(18), 18)
    # Left cerebellum, whose white matter and cortex are one class: 4 voxels of white matter alone go, and 2 that
    # touch 5 of cortex stay.
    scene.block((3, 8, 0), (2, 2, 1), part(7), 2)
    scene.block((3, 8, 3), (1, 1, 2), part(7), 7)
    scene.block((4, 8, 3), (1, 1, 5), part(8), 8)
    # The fluid round the brain is of the lowest class, which is left alone: both pieces stay.
    scene.block((0, 7, 0), (2, 2, 1), part(24), 24)
    scene.block((0, 7, 3), (1, 1, 2), part(24), 24)
    # Two chances that are equal: the first in the networks' order wins.
    scene.block((7, 4, 6), (1, 1, 1), {2: 0.375, 41: 0.375}, 2)
    scene.block((7, 4, 8), (1, 1, 1), {0: 0.5, 2: 0.5}, 0)


def speck_scene(scene: Scene) -> None:
    """The largest piece is right white matter. What goes becomes nothing."""
    scene.ground((0, 1, 1), (4, 5, 6), {41: 0.7}, 41, noisy=True)
    # Three voxels of left putamen in the blob stay, though six more lie apart from it: those went first.
    scene.block((1, 1, 1), (1, 1, 3), {12: 0.6, 41: 0.3}, 12, noisy=True)
    scene.block((5, 6, 7), (1, 2, 3), {12: 0.8}, 0, noisy=True)
    # A speck of brain stem far off.
    scene.block((5, 0, 0), (1, 1, 2), {16: 0.9}, 0, noisy=True)
    # Left ventral DC, joined to the blob by a voxel whose chances add to 0.375 with none over 0.125: it stays.
    scene.block((1, 6, 1), (1, 1, 1), {28: 0.125, 41: 0.125, 16: 0.125}, 0)
    scene.block((1, 7, 1), (2, 1, 2), {28: 0.8}, 28, noisy=True)
    # Right ventral DC, with a voxel between whose chances add to exactly 0.25: not joined.
    scene.block((3, 6, 4), (1, 1, 1), {60: 0.125, 41: 0.125}, 0)
    scene.block((3, 7, 4), (1, 1, 2), {60: 0.8}, 0, noisy=True)
    # More right white matter, two voxels from the blob inside the grid and joined to it through the padding: it stays.
    scene.block((0, 1, 9), (2, 2, 1), {41: 0.7}, 41, noisy=True)
    scene.fill(scene.where((-1, 1, 1), (1, 1, 9)), {41: 0.6}, noisy=True)


def cortex_scene(scene: Scene, lists: dict, deep: int = 5, high: int = 7, speck=(5, 7, 9), island=None) -> None:
    """Left to right: left cortex, left white matter, right white matter, right cortex, each so deep and so high;
    and a speck of right cortex apart, which goes and takes no area. With an `island`, two voxels of left cortex
    lie there in the right white matter, away from the rest of their class: a second piece, which the standard
    way would lose and the fast way keeps, areas and all. The scene is given the area at each voxel, as the cortex
    network would."""
    cortex = [((0, 1, 1), (1, deep, high)), ((5, 1, 1), (1, deep, high))]
    scene.block((0, 1, 1), (1, deep, high), {3: 0.7}, None, noisy=True)
    scene.block((1, 1, 1), (2, deep, high), {2: 0.7}, 2, noisy=True)
    if island is None:
        scene.block((3, 1, 1), (2, deep, high), {41: 0.7}, 41, noisy=True)
    else:
        scene.ground((3, 1, 1), (2, deep, high), {41: 0.7}, 41, noisy=True)
        scene.block(island, (1, 1, 2), {3: 0.7}, None, noisy=True)
        cortex.append((island, (1, 1, 2)))
    scene.block((5, 1, 1), (1, deep, high), {42: 0.7}, None, noisy=True)
    scene.block(speck, (1, 1, 1), {42: 0.9}, 0, noisy=True)
    # The chances the cortex network gives are made up for every voxel, cortex or not, as the network gives them.
    wild = scene.rng.normal(0, 3, (*scene.taken.shape, len(lists["area_labels"]))).astype(np.float32)
    chances = np.exp(wild - wild.max(axis=-1, keepdims=True))
    chances /= chances.sum(axis=-1, keepdims=True)
    areas = 1 + np.argmax(chances[..., 1:], axis=-1)  # what tools/synthseg-to-onnx (give_areas) makes the network give
    for start, size in cortex:
        where = scene.where(start, size)
        scene.sure.append((where, lists["area_labels"][areas[where]]))
    scene.areas, scene.area_chances = areas, chances


def in_turn(values: np.ndarray) -> np.float32:
    """Single precision numbers added one after another, as a plain loop adds them."""
    total = np.float32(0)
    for value in values:
        total = np.float32(total + value)
    return total


def as_numpy_adds(values: np.ndarray) -> np.float32:
    """The same numbers added as SynthSeg adds a voxel's chances: NumPy's sum along the last axis."""
    return np.sum(values[np.newaxis, np.newaxis, np.newaxis], axis=-1)[0, 0, 0]


def eight_abreast(values: np.ndarray, halved=True, at_eight=True, rest_last=True) -> np.float32:
    """The same sum by the rule written out, which is the one Volsa follows. Under eight numbers are added in turn.
    Up to 128 are added in eight lanes, every eighth number to a lane; the lanes are added in pairs, and what is
    left over is added in turn. A longer list is halved at a multiple of eight, and its halves are added. The
    three switches each break one part of the rule, to see that a list can tell."""
    count = len(values)
    if count > 128 and halved:
        half = count // 2 - (count // 2 % 8 if at_eight else 0)
        parts = [eight_abreast(part, halved, at_eight, rest_last) for part in (values[:half], values[half:])]
        return np.float32(parts[0] + parts[1])
    if count < 8:
        return in_turn(values)
    whole = count - count % 8
    lanes = values[:whole].reshape(-1, 8)
    lane = list(lanes[0])
    for row in lanes[1:]:
        lane = [np.float32(lane[k] + row[k]) for k in range(8)]
    pairs = [np.float32(lane[k] + lane[k + 1]) for k in (0, 2, 4, 6)]
    total = np.float32(np.float32(pairs[0] + pairs[1]) + np.float32(pairs[2] + pairs[3]))
    return in_turn([total, *values[whole:]]) if rest_last else np.float32(in_turn(values[whole:]) + total)


def sums(out: Path) -> None:
    """Lists of made-up single precision numbers, each with its sum as NumPy adds along the last axis of an array,
    which is how SynthSeg adds a voxel's chances: 7, 9, 15, 33, 130 and 255 numbers, on each side of the lengths
    at which NumPy's way of adding changes. The script stops if NumPy no longer adds by the rule Volsa follows."""
    rng = seeded("tidy", "sums")
    # The ways of adding that each list must tell from NumPy's: the numbers added in turn, a long list not halved or
    # halved anywhere, and what is left over added before the lanes. (One number left over is added the same either
    # way, and seven numbers are added in turn by the rule itself.)
    in_a_row = lambda numbers: in_turn(numbers)
    not_halved = lambda numbers: eight_abreast(numbers, halved=False)
    halved_anywhere = lambda numbers: eight_abreast(numbers, at_eight=False)
    rest_first = lambda numbers: eight_abreast(numbers, rest_last=False)
    long = [in_a_row, not_halved, halved_anywhere, rest_first]
    tells = {7: [], 9: [in_a_row], 15: [in_a_row, rest_first], 33: [in_a_row], 130: long, 255: long}
    kept = []
    for count, others in tells.items():
        while True:
            numbers = rng.random(count, dtype=np.float32)
            ours = as_numpy_adds(numbers)
            if ours != eight_abreast(numbers):
                raise SystemExit(f"NumPy does not add {count} numbers by the rule Volsa follows.")
            if all(ours != other(numbers) for other in others):
                break
        kept.append({"numbers": [float(number) for number in numbers], "sum": float(ours)})
    write_json(out / "sums.json", {"what": " ".join(sums.__doc__.split()), "made_with": {"numpy": np.__version__},
                                   "holds": "each number is a 32-bit float: take it as one", "sums": kept})
    print(f"tidy/sums: {len(kept)} lists of {[len(each['numbers']) for each in kept]} numbers")


def near_a_quarter(rng, numpy_says_over: bool) -> np.ndarray:
    """A voxel's 33 chances whose 32 after the first add to more than 0.25 in NumPy's order, but not when added in
    turn and not when added exactly; or the other way round. Nothing, the first, has the other three quarters."""
    while True:
        chances = np.full(STRUCTURES, 0.75, np.float32)
        chances[1:] = rng.random(STRUCTURES - 1, dtype=np.float32)
        chances[1:] *= QUARTER / chances[1:].sum(dtype=np.float32)
        ours, plain, exact = as_numpy_adds(chances[1:]), in_turn(chances[1:]), math.fsum(map(float, chances[1:]))
        if numpy_says_over and ours > QUARTER and plain <= QUARTER and exact <= 0.25:
            return chances
        if not numpy_says_over and ours <= QUARTER and plain > QUARTER and exact > 0.25:
            return chances


def one_step_apart(rng, first: int, second: int, others: list, become_equal: bool, little=0.05, tells=True):
    """A voxel's 33 chances in which the one at `second` is the next number above the one at `first`, and those
    at `others` have a little each. Divided by their sum as NumPy adds it, the two become equal, or stay apart;
    and, when the voxel is to tell the ways of adding apart, divided by their sum added in turn it is the other
    way round. (Two numbers one step apart can become equal only when the sum is over 1.)"""
    while True:
        chances = np.zeros(STRUCTURES, np.float32)
        chances[others] = rng.random(len(others), dtype=np.float32) * np.float32(little)
        chances[first] = rng.uniform(0.26, 0.45)
        chances[second] = np.nextafter(chances[first], np.float32(1))
        shared, plainly = chances / as_numpy_adds(chances), chances / in_turn(chances)
        equal = shared[first] == shared[second]
        if equal == become_equal and (not tells or equal != (plainly[first] == plainly[second])):
            return chances


def close_scene(scene: Scene) -> None:
    """A blob of right white matter, with what is on the edge of the arithmetic in it and beside it."""
    scene.ground((1, 1, 1), (3, 4, 6), {41: 0.75}, 41)
    # Right thalamus beyond a voxel that joins it to the blob only as NumPy adds: it stays. Right caudate beyond a
    # voxel that would join it only if added another way: it goes.
    scene.voxel((1, 5, 1), near_a_quarter(scene.rng, True), 0)
    scene.block((1, 6, 1), (1, 1, 2), {49: 0.75}, 49)
    scene.voxel((3, 5, 4), near_a_quarter(scene.rng, False), 0)
    scene.block((3, 6, 4), (1, 1, 2), {50: 0.75}, 0)
    # Right white matter and, one step above it, right cortex: equal once divided, so the first wins; and not.
    # Divided by a sum added in turn, each would fall the other way. The others with a chance are those the tidying
    # leaves alone here: nothing and the fluid, of the lowest class, and the rest of the blob's own class.
    white, cortex = scene.place[41], scene.place[42]
    others = [scene.place[label] for label in (0, 24, 43, 44)]
    scene.voxel((2, 2, 3), one_step_apart(scene.rng, white, cortex, others, True), 41)
    scene.voxel((2, 2, 5), one_step_apart(scene.rng, white, cortex, others, False), 42)
    # Nothing at all: nought divided by nought is no number, and the first of those wins.
    scene.voxel((2, 3, 2), np.zeros(STRUCTURES, np.float32), 0)
    # No network gives a chance below nought. This voxel's add to nought, so divided they are no numbers where they
    # were nought, and the first of those is left cerebral white matter.
    upside_down = np.zeros(STRUCTURES, np.float32)
    upside_down[0], upside_down[white] = -0.5, 0.5
    scene.voxel((2, 3, 4), upside_down, 2)


def robust_pieces_scene(scene: Scene) -> None:
    """A blob of right white matter. With `fast` only the piece of everything is looked for, and only inside the
    image: no structure loses a piece of its own."""
    scene.ground((0, 0, 0), (4, 6, 8), {41: 0.75}, 41)
    # Left thalamus in two pieces of 8 voxels and 2: both stay. The standard way, the smaller would go.
    scene.block((0, 0, 0), (2, 2, 2), {10: 0.5625, 41: 0.3125}, 10)
    scene.block((0, 0, 4), (1, 1, 2), {10: 0.5625, 41: 0.3125}, 10)
    # More right white matter, joined to the blob only through the padding: it goes. The standard way, it would stay.
    scene.block((5, 0, 7), (1, 2, 1), {41: 0.75}, 0)
    scene.fill(scene.where((3, -1, 7), (3, 1, 1)), {41: 0.75}, noisy=False)
    # A speck of brain stem apart.
    scene.block((5, 5, 0), (1, 1, 1), {16: 0.9}, 0)
    # Left ventral DC, joined to the blob by a voxel whose chances add to 0.375 with none over 0.125: it stays, and
    # the voxel between is nothing, having no chance over 0.2.
    scene.block((4, 3, 2), (1, 1, 1), {28: 0.125, 41: 0.125, 16: 0.125}, 0)
    scene.block((5, 3, 2), (1, 2, 2), {28: 0.75}, 28)
    # Right ventral DC, with a voxel between whose chances add to exactly 0.25: not joined.
    scene.block((4, 5, 5), (1, 1, 1), {60: 0.125, 41: 0.125}, 0)
    scene.block((5, 5, 5), (1, 1, 2), {60: 0.75}, 0)


def robust_marks_scene(scene: Scene) -> None:
    """A blob of right white matter, with voxels in it that turn on the mark of 0.2, and what is on the edge of
    the arithmetic in it and beside it."""
    scene.ground((0, 0, 0), (4, 4, 8), {41: 0.75}, 41)
    others = {41: 0.19, 42: 0.19, 43: 0.19}
    # Left white matter at 0.22, the one chance over 0.2: it wins over nothing at 0.21. A mark of 0.25 would lose it.
    scene.block((1, 1, 1), (1, 1, 1), {0: 0.21, 2: 0.22, **others}, 2)
    # At exactly 0.2, as single precision has it, it goes and nothing is left but nothing; at the next number
    # above, it stays.
    scene.block((1, 1, 3), (1, 1, 1), {0: 0.15, 2: MARK, **others}, 0)
    scene.block((1, 1, 5), (1, 1, 1), {0: 0.15, 2: np.nextafter(MARK, np.float32(1)), **others}, 2)
    # Five chances of 0.19 and little of nothing: no chance is over 0.2, so the voxel is nothing.
    scene.block((2, 2, 1), (1, 1, 1), {0: 0.05, 2: 0.19, 3: 0.19, **others}, 0)
    # As in `close`: right thalamus beyond a voxel that joins it to the blob only as NumPy adds, which stays, and
    # right caudate beyond one that would join it only if added another way, which goes.
    scene.voxel((4, 1, 1), near_a_quarter(scene.rng, True), 0)
    scene.block((5, 1, 1), (1, 1, 2), {49: 0.75}, 49)
    scene.voxel((4, 3, 5), near_a_quarter(scene.rng, False), 0)
    scene.block((5, 3, 5), (1, 1, 2), {50: 0.75}, 0)
    # Right white matter and, one step above it, right cortex: equal once divided, so the first wins; and not.
    # Nothing has the only other chance, since any other under 0.2 would go.
    white, cortex = scene.place[41], scene.place[42]
    scene.voxel((2, 2, 3), one_step_apart(scene.rng, white, cortex, [0], True, little=0.25, tells=False), 41)
    scene.voxel((2, 2, 5), one_step_apart(scene.rng, white, cortex, [0], False, little=0.25, tells=False), 42)


# name: the scene, and how much noise lies under it
SCENES = {"pieces": (pieces_scene, 0.0), "speck": (speck_scene, 0.004), "close": (close_scene, 0.0),
          "cortex": (cortex_scene, 0.004), "robust_pieces": (robust_pieces_scene, 0.0),
          "robust_marks": (robust_marks_scene, 0.0), "robust_cortex": (cortex_scene, 0.004)}


def tidy_case(name: str, lists: dict, out: Path, edit_volumes, postprocess) -> None:
    case = TIDY[name]
    aff = affine(case["steps"], (0.0, 0.0, 0.0))
    order, turned_round = turn_of(aff, edit_volumes)
    whole, before, padded = (np.array(case[key]) for key in ("whole", "before", "padded"))
    cut_from, size = (np.array(part) for part in case["cut"] or ((0, 0, 0), whole))
    fast = bool(case.get("fast"))
    make, noise = SCENES[name]
    scene = Scene(lists, case, seeded("tidy", name), noise=noise)
    if name == "cortex":
        make(scene, lists)
    elif name == "robust_cortex":
        make(scene, lists, deep=4, high=6, speck=(5, 5, 7), island=(4, 2, 3))
    else:
        make(scene)

    # SynthSeg's own tidying, called as its predict() calls it: for the standard model of 2.0, or with `fast` on, as
    # its own program has it whenever the robust networks run. It changes what it is given, so it is given copies.
    parc = None if scene.area_chances is None else scene.area_chances[np.newaxis].copy()
    with np.errstate(invalid="ignore", divide="ignore"):  # nought divided by nought is meant
        labels, _, _ = postprocess(
            post_patch_seg=scene.chances[np.newaxis].copy(), post_patch_parc=parc, shape=[int(s) for s in whole],
            pad_idx=np.concatenate([before, before + size]),
            crop_idx=None if case["cut"] is None else np.concatenate([cut_from, cut_from + size]),
            labels_segmentation=lists["labels"], labels_parcellation=lists["area_labels"], aff=aff, im_res=np.ones(3),
            fast=fast, topology_classes=np.asarray(lists["classes"]), v1=False)
    dims = [0, 0, 0]
    for axis in range(3):
        dims[order[axis]] = int(whole[axis])
    if list(labels.shape) != dims or not np.isin(labels, np.concatenate([lists["labels"], lists["area_labels"]])).all():
        raise SystemExit(f"{name}: SynthSeg's labels are not a grid of {dims} holding its own labels.")
    # The labels as the image has them, by the turn written down: what each block said is held against these.
    image = np.transpose(labels, order)
    for axis in range(3):
        if turned_round[axis]:
            image = np.flip(image, axis=axis)
    inside = tuple(slice(int(f), int(f + s)) for f, s in zip(cut_from, size))
    if np.count_nonzero(image) != np.count_nonzero(image[inside]):
        raise SystemExit(f"{name}: a label lies outside the cut.")
    scene.check(name, image, cut_from)

    out.mkdir(parents=True, exist_ok=True)
    write_floats(out / "chances.f32", scene.chances, "last")
    (out / "labels.i32").write_bytes(labels.astype("<i4").tobytes(order="F"))
    areas = None
    if scene.areas is not None:
        write_bytes(out / "areas.u8", scene.areas)
        areas = {"file": "areas.u8", "holds": AREAS_HOLD}
    import scipy

    write_json(out / "case.json", {
        "what": case["what"], "made_with": {"numpy": np.__version__, "scipy": scipy.__version__}, "fast": fast,
        "chances": {"file": "chances.f32", "holds": CHANCES_HOLD, "padded": [int(s) for s in padded],
                    "structures": STRUCTURES},
        "areas": areas,
        "back": {"order": order, "turned_round": turned_round, "padding_before": [int(b) for b in before],
                 "size": [int(s) for s in size], "cut_from": [int(f) for f in cut_from], "grid_dims": dims},
        "labels": {"file": "labels.i32", "holds": LABELS_HOLD, "dims": dims}})
    print(f"tidy/{name}: {tuple(int(s) for s in padded)} padded gives {tuple(dims)} as stored, "
          f"{int(np.count_nonzero(labels))} voxels labelled with {len(np.unique(labels)) - 1} labels")


def tidy_group(checkout: Path, out: Path) -> None:
    """SynthSeg's tidying of what its networks give: its function postprocess, with its own lists."""
    from convert import read_lists
    from ext.lab2im import edit_volumes
    from SynthSeg.predict_synthseg import postprocess

    lists = read_lists(checkout)
    if len(lists["labels"]) != STRUCTURES:
        raise SystemExit(f"SynthSeg's list has {len(lists['labels'])} structures, not {STRUCTURES}.")
    out.mkdir(parents=True, exist_ok=True)
    write_json(out / "lists.json", {
        "what": "SynthSeg 2.0's own lists, as its predict() prepares them: the label of each structure in the "
                "networks' order, the class its tidying gives each, the label of each cortex area in the cortex "
                "network's order, and the two labels that are cortex",
        "labels": [int(label) for label in lists["labels"]], "classes": [int(c) for c in lists["classes"]],
        "area_labels": [int(label) for label in lists["area_labels"]], "cortex": [3, 42]})
    for name in TIDY:
        tidy_case(name, lists, out / name, edit_volumes, postprocess)
    sums(out)


# ---------------------------------------------------------------- every group, by name

# A later group of cases is a function like prepare_group and a line here.
GROUPS = {"prepare": prepare_group, "box": box_group, "tidy": tidy_group}


def main() -> None:
    if len(sys.argv) < 2 or any(name not in GROUPS for name in sys.argv[2:]):
        raise SystemExit(__doc__ + f"\nThe groups: {', '.join(GROUPS)}.")
    checkout = Path(sys.argv[1]).resolve()
    if not (checkout / "SynthSeg" / "predict_synthseg.py").is_file():
        raise SystemExit(f"{checkout} is not a copy of SynthSeg.")
    use_synthseg(checkout)
    for name in sys.argv[2:] or GROUPS:
        GROUPS[name](checkout, GOLDEN / name)
        written = sum(file.stat().st_size for file in (GOLDEN / name).rglob("*") if file.is_file())
        print(f"{name}: {written / 1024:.0f} KB under {(GOLDEN / name).relative_to(TOOLS.parent).as_posix()}")


if __name__ == "__main__":
    main()
