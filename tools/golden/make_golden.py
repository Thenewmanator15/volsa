"""Make the arrays Voxa's tests are held against, with SynthSeg's own Python.

    python make_golden.py SYNTHSEG_CHECKOUT [GROUP ...]

SYNTHSEG_CHECKOUT is a copy of github.com/BBillot/SynthSeg; no weights are needed. Each group
of cases is written under crates/voxa-segment/tests/golden/GROUP/. With no group named, every
group is made. README.md beside this file says what each case holds and in what order.

Every volume here is made up from a seed, so a second run writes the same bytes.

This script ships nothing and is no part of Voxa: it is not built, run or tested by cargo.
"""

import json
import math
import sys
import tempfile
import zlib
from pathlib import Path

import numpy as np

TOOLS = Path(__file__).resolve().parents[1]
GOLDEN = TOOLS.parent / "crates" / "voxa-segment" / "tests" / "golden"

sys.path.insert(0, str(TOOLS / "synthseg-to-onnx"))
from convert import use_synthseg  # noqa: E402  (how SynthSeg's Keras 2 code is made importable)

VOLUME_HOLDS = ("little-endian f32; columns fastest, then rows, then slices: "
                "values[(slice * rows + row) * cols + col]")
IMAGE_HOLDS = ("little-endian f32; the image before padding, as the network's [x, y, z] with z fastest: "
               "values[(x * size_y + y) * size_z + z]")
TO_PATIENT = ("voxel (column, row, slice) to the patient in left-posterior-superior millimetres, as Voxa holds it: "
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
    """SynthSeg's right-anterior-superior transform as Voxa holds one: left-posterior-superior."""
    lps = np.diag([-1.0, -1.0, 1.0, 1.0]) @ aff_ras
    numbers = lambda column: [float(v) + 0.0 for v in column]  # + 0.0: no minus zero in the file
    return {"columns": [numbers(lps[:3, c]) for c in range(3)], "origin": numbers(lps[:3, 3])}


def write_floats(path: Path, array: np.ndarray, fastest: str) -> None:
    """`fastest` is "first" for a volume in Voxa's order, "last" for a network's image."""
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


# ---------------------------------------------------------------- every group, by name

# A later group of cases is a function like prepare_group and a line here.
GROUPS = {"prepare": prepare_group, "box": box_group}


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
