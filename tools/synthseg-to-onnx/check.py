"""The converter's own check: SynthSeg's program and the converted networks, on a made-up
volume, on the processor and on every graphics adapter DirectML offers.

`run` writes the `check` folder (the made-up volume and what Python made of it, for Voxa's
own tests) and returns what was measured. It stops the converter if a mark is missed:

- the standard model on the processor gives Python's label at every voxel;
- every other model and device agrees at 99.99% of voxels or more, with Dice of at least
  0.999 for every structure, and scores within 0.001;
- the cortex network's byte answer gives the same labels as SynthSeg's own use of its
  probabilities.

The volume is painted from the sample label map that comes with SynthSeg. It is nobody's scan.
"""

import ctypes
import json
import tempfile
import time
import uuid
from ctypes import wintypes
from pathlib import Path

import numpy as np

SAME_PERCENT = 99.99
DICE = 0.999
SCORES = 0.001
THICK = 4  # slices of the made-up volume averaged into one, for the thick copy


def made_up_head(checkout: Path, seed: int = 7):
    """A head painted from SynthSeg's sample label map: one grey level for each kind of
    tissue, a little blur, a slow shading across it, and noise. Cut to the brain with a
    margin, in the axis order SynthSeg calls standard. Returns the values and the transform."""
    import nibabel as nib
    from scipy import ndimage

    data = checkout / "data"
    source = nib.load(data / "training_label_maps" / "training_seg_01.nii.gz")
    labels = np.asarray(source.dataobj).astype(np.int32)
    paint_labels = np.load(data / "labels_classes_priors" / "generation_labels.npy")
    paint_classes = np.load(data / "labels_classes_priors" / "generation_classes.npy")
    rng = np.random.default_rng(seed)
    greys = rng.uniform(25, 225, size=int(paint_classes.max()) + 1)
    table = np.zeros(int(max(labels.max(), paint_labels.max())) + 1, np.float32)
    for label, kind in zip(paint_labels, paint_classes):
        table[int(label)] = 0.0 if label == 0 else greys[int(kind)]
    image = ndimage.gaussian_filter(table[labels], 0.7)
    shading = ndimage.zoom(rng.uniform(0.85, 1.15, size=(4, 4, 4)), [s / 4 for s in image.shape], order=3)
    image = np.clip(image * shading + rng.normal(0, 6, size=image.shape), 0, None).astype(np.float32)

    brain = np.load(data / "labels_classes_priors" / "synthseg_segmentation_labels_2.0.npy")
    inside = np.argwhere(np.isin(labels, brain[(brain > 0) & (brain != 24)]))
    low, high = np.maximum(inside.min(0) - 4, 0), np.minimum(inside.max(0) + 5, labels.shape)
    image = image[low[0]:high[0], low[1]:high[1], low[2]:high[2]]
    affine = source.affine.copy()
    affine[:3, 3] += affine[:3, :3] @ low
    standard = nib.as_closest_canonical(nib.Nifti1Image(image, affine))
    return np.asarray(standard.dataobj).astype(np.float32), standard.affine.copy()


def thick_copy(volume: np.ndarray, affine: np.ndarray):
    """The same head with slices four times as thick: each the mean of four."""
    count = volume.shape[2] // THICK
    slabs = volume[:, :, : count * THICK].reshape(*volume.shape[:2], count, THICK).mean(-1).astype(np.float32)
    placed = affine.copy()
    placed[:3, 3] += affine[:3, 2] * (THICK - 1) / 2
    placed[:3, 2] *= THICK
    return slabs, placed


def prepared(volume: np.ndarray, affine: np.ndarray):
    """SynthSeg's own preparing of a volume: what the networks see, and how to come back."""
    import nibabel as nib
    from SynthSeg.predict_synthseg import preprocess

    with tempfile.TemporaryDirectory() as folder:
        path = str(Path(folder) / "volume.nii.gz")
        nib.save(nib.Nifti1Image(volume, affine), path)
        image, aff, _, im_res, shape, pad_idx, _ = preprocess(path_image=path, ct=False, crop=None, min_pad=128)
    return image.astype(np.float32), {"aff": aff, "im_res": im_res, "shape": shape, "pad_idx": pad_idx}


def python_answer(checkout: Path, lists: dict, image: np.ndarray, robust: bool) -> dict:
    """SynthSeg's own graph, with cortex areas and scores, as its predict() builds it."""
    from SynthSeg.predict_synthseg import build_model

    models = checkout / "models"
    net = build_model(path_model_segmentation=str(models / ("synthseg_robust_2.0.h5" if robust else "synthseg_2.0.h5")),
                      path_model_parcellation=str(models / "synthseg_parc_2.0.h5"),
                      path_model_qc=str(models / "synthseg_qc_2.0.h5"), input_shape_qc=224,
                      labels_segmentation=lists["labels"], labels_denoiser=np.arange(lists["stage_classes"]),
                      labels_parcellation=lists["area_labels"], labels_qc=lists["score_class"], sigma_smoothing=0.5,
                      flip_indices=None if robust else lists["flip"], robust=robust, do_parcellation=True, do_qc=True)
    started = time.perf_counter()
    probabilities, areas, scores = net.predict([image, np.array(image.shape[1:-1])[None]], verbose=0)
    return {"probabilities": probabilities.astype(np.float32), "area_probabilities": areas.astype(np.float32),
            "scores": np.around(np.clip(np.squeeze(scores)[1:], 0, 1), 4), "seconds": time.perf_counter() - started}


def tidied(lists: dict, back: dict, probabilities: np.ndarray, robust: bool, area_probabilities=None,
           areas=None) -> np.ndarray:
    """SynthSeg's own tidying, as its program asks for it: the program forces `fast` for the
    robust model, and so does this. The cortex comes either from the area probabilities, as
    SynthSeg uses them, or from a byte map of areas, as Voxa will."""
    from ext.lab2im import edit_volumes
    from SynthSeg.predict_synthseg import postprocess

    labels, _, _ = postprocess(post_patch_seg=probabilities.copy(),
                               post_patch_parc=None if area_probabilities is None else area_probabilities.copy(),
                               shape=back["shape"], pad_idx=back["pad_idx"], crop_idx=None, labels_segmentation=lists["labels"],
                               labels_parcellation=lists["area_labels"], aff=back["aff"], im_res=back["im_res"], fast=robust,
                               topology_classes=lists["classes"], v1=False)
    if areas is not None:
        there = edit_volumes.crop_volume_with_idx(np.squeeze(areas), back["pad_idx"], n_dims=3)
        there = edit_volumes.align_volume_to_ref(there, np.eye(4), aff_ref=back["aff"], n_dims=3)
        cortex = (labels == 3) | (labels == 42)
        labels[cortex] = lists["area_labels"][there[cortex]]
    return labels.astype(np.int32)


def scores_box(top: np.ndarray, side: int) -> np.ndarray:
    """SynthSeg's MakeShape layer: a cube of `side` centred on the voxels whose top structure
    is neither place 0 nor place 24, cut or padded with zeros to fit."""
    shape = np.array(top.shape)
    where = np.argwhere((top != 0) & (top != 24))
    if len(where) == 0:
        low, high = np.zeros(3, int), np.minimum(shape, side)
    else:
        low, high = np.maximum(where.min(0), 0), np.minimum(where.max(0) + 1, shape)
    extent = high - low
    low = low - np.ceil((side - extent) / 2).astype(int)
    high = high + np.floor((side - extent) / 2).astype(int)
    a, b = np.maximum(low, 0), np.minimum(high, shape)
    cut = top[a[0]:b[0], a[1]:b[1], a[2]:b[2]]
    return np.pad(cut, list(zip(np.abs(np.minimum(low, 0)), np.maximum(high - shape, 0))))


def onnx_answer(out: Path, lists: dict, image: np.ndarray, robust: bool, device) -> dict:
    """The converted networks, with the steps between them done here as Voxa will do them."""
    import onnxruntime as ort

    seconds = 0.0

    def run(role: str, *feeds: dict) -> list:
        nonlocal seconds
        options = ort.SessionOptions()
        options.log_severity_level = 3
        if role != "scores":
            for name, size in zip("xyz", image.shape[1:4]):
                options.add_free_dimension_override_by_name(name, int(size))
        if device == "processor":
            providers = ["CPUExecutionProvider"]
        else:
            options.enable_mem_pattern = False
            options.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
            providers = [("DmlExecutionProvider", {"device_id": int(device)}), "CPUExecutionProvider"]
        session = ort.InferenceSession(str(out / f"{role}.onnx"), sess_options=options, providers=providers)
        names = [o.name for o in session.get_outputs()]
        answers = []
        for one in feeds:
            started = time.perf_counter()
            if device != "processor" and len(names) > 1:  # kept on the card, fetched one at a time
                binding = session.io_binding()
                for name, value in one.items():
                    binding.bind_cpu_input(name, np.ascontiguousarray(value))
                for name in names:
                    binding.bind_output(name, "dml")
                session.run_with_iobinding(binding)
                got = binding.copy_outputs_to_cpu()
            else:
                got = session.run(None, {name: np.ascontiguousarray(value) for name, value in one.items()})
            seconds += time.perf_counter() - started
            answers.append(got[0] if len(got) == 1 else np.concatenate(got, -1))
        return answers

    if robust:
        classes = run("robust_1", {"image": image})[0]
        cleaned = run("robust_2", {"classes": classes})[0]
        probabilities = run("robust_3", {"image": image, "classes": cleaned})[0]
    else:
        straight, mirrored = run("standard", {"image": image}, {"image": image[:, ::-1]})
        probabilities = 0.5 * (straight + mirrored[:, ::-1][..., lists["flip"]])
    top = probabilities.argmax(-1).astype(np.uint8)
    cortex = np.isin(lists["labels"][top], (3, 42)).astype(np.uint8)
    areas = run("cortex", {"image": image, "cortex": cortex})[0]
    classes = lists["score_class"][scores_box(top[0], 224)].astype(np.uint8)[None]
    scores = np.around(np.clip(np.squeeze(run("scores", {"classes": classes})[0])[1:], 0, 1), 4)
    return {"probabilities": probabilities, "areas": areas, "scores": scores, "seconds": seconds}


def agreement(python: np.ndarray, ours: np.ndarray) -> dict:
    same = python == ours
    dice = []
    for label in np.union1d(np.unique(python), np.unique(ours)):
        if label:
            a, b = python == label, ours == label
            dice.append(2 * np.count_nonzero(a & b) / (np.count_nonzero(a) + np.count_nonzero(b)))
    return {"voxels": int(same.size), "differ": int((~same).sum()), "same_percent": round(float(100 * same.mean()), 5),
            "structures": len(dice), "dice_least": round(float(min(dice)), 5), "dice_mean": round(float(np.mean(dice)), 6)}


def adapters() -> list:
    """(number, name) of each graphics adapter in the system's own order, software ones and
    second listings of the same card left out. Empty where there is no such list."""
    try:
        dxgi = ctypes.WinDLL("dxgi")
    except (AttributeError, OSError):
        return []

    class Guid(ctypes.Structure):
        _fields_ = [("a", wintypes.DWORD), ("b", wintypes.WORD), ("c", wintypes.WORD), ("d", ctypes.c_ubyte * 8)]

    class Described(ctypes.Structure):
        _fields_ = [("name", wintypes.WCHAR * 128), ("ids", wintypes.UINT * 4), ("memory", ctypes.c_size_t * 3),
                    ("luid", wintypes.DWORD * 2), ("flags", wintypes.UINT)]

    def method(thing, place, *kinds):
        table = ctypes.cast(ctypes.cast(thing, ctypes.POINTER(ctypes.c_void_p))[0], ctypes.POINTER(ctypes.c_void_p))
        return ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p, *kinds)(table[place])

    wanted = uuid.UUID("770aae78-f26f-4dba-a829-253c83d1b387")  # IDXGIFactory1
    guid = Guid(wanted.time_low, wanted.time_mid, wanted.time_hi_version, (ctypes.c_ubyte * 8)(*wanted.bytes[8:]))
    factory = ctypes.c_void_p()
    dxgi.CreateDXGIFactory1.argtypes = [ctypes.POINTER(Guid), ctypes.POINTER(ctypes.c_void_p)]
    dxgi.CreateDXGIFactory1.restype = ctypes.c_long
    if dxgi.CreateDXGIFactory1(ctypes.byref(guid), ctypes.byref(factory)) != 0:
        return []
    found, seen = [], set()
    for number in range(16):
        adapter = ctypes.c_void_p()
        if method(factory, 12, wintypes.UINT, ctypes.POINTER(ctypes.c_void_p))(factory, number, ctypes.byref(adapter)) != 0:
            break
        described = Described()
        method(adapter, 10, ctypes.POINTER(Described))(adapter, ctypes.byref(described))
        if not described.flags & 2 and described.name not in seen:
            found.append((number, described.name))
            seen.add(described.name)
        method(adapter, 2)(adapter)
    method(factory, 2)(factory)
    return found


def run(checkout: Path, out: Path, lists: dict) -> dict:
    volume, affine = made_up_head(checkout)
    image, back = prepared(volume, affine)
    slabs, slabs_affine = thick_copy(volume, affine)
    slabs_image, _ = prepared(slabs, slabs_affine)
    print(f"made-up volume {volume.shape}; the networks see {image.shape[1:4]}; thick copy {slabs.shape} becomes {slabs_image.shape[1:4]}")

    devices = ["processor"] + [number for number, _ in adapters()]
    names = {"processor": "processor", **{number: name for number, name in adapters()}}
    checked, failed, python = {}, [], {}
    for robust in (False, True):
        model = "robust" if robust else "standard"
        theirs = python_answer(checkout, lists, image, robust)
        with_areas = tidied(lists, back, theirs["probabilities"], robust, area_probabilities=theirs["area_probabilities"])
        without = tidied(lists, back, theirs["probabilities"], robust)
        by_bytes = tidied(lists, back, theirs["probabilities"], robust,
                          areas=theirs["area_probabilities"][..., 1:].argmax(-1) + 1)
        if not np.array_equal(with_areas, by_bytes):
            failed.append(f"{model}: areas as bytes give other labels than SynthSeg's own use of the probabilities")
        python[model] = {"with": with_areas, "without": without, "scores": theirs["scores"]}
        print(f"Python, {model} model with cortex areas and scores: {theirs['seconds']:.1f} s")
        checked[model] = {}
        for device in devices:
            ours = onnx_answer(out, lists, image, robust, device)
            labels = tidied(lists, back, ours["probabilities"], robust, areas=ours["areas"])
            result = agreement(with_areas, labels)
            result["scores_differ_by"] = round(float(np.abs(ours["scores"] - theirs["scores"]).max()), 5)
            result["seconds"] = round(ours["seconds"], 1)
            checked[model][names[device]] = result
            print(f"  {model} on {names[device]}: {result}")
            if device == "processor" and not robust:
                plain = tidied(lists, back, ours["probabilities"], robust)
                if not np.array_equal(plain, without):
                    failed.append(f"standard on the processor: {int((plain != without).sum())} voxels differ from Python")
            if result["same_percent"] < SAME_PERCENT or result["dice_least"] < DICE or result["scores_differ_by"] > SCORES:
                failed.append(f"{model} on {names[device]}: {result}")
    if failed:
        raise SystemExit("The converted networks do not give SynthSeg's own answer, so no model.json was written:\n  " + "\n  ".join(failed))

    folder = out / "check"
    folder.mkdir(exist_ok=True)
    columns_fastest = lambda array: np.ascontiguousarray(array.transpose(2, 1, 0))  # noqa: E731
    columns_fastest(volume).astype("<f4").tofile(folder / "volume.f32")
    columns_fastest(slabs).astype("<f4").tofile(folder / "thick.f32")
    image[0, ..., 0].astype("<f4").tofile(folder / "image.f32")
    slabs_image[0, ..., 0].astype("<f4").tofile(folder / "thick_image.f32")
    columns_fastest(python["standard"]["without"]).astype("<u2").tofile(folder / "standard.u16")
    columns_fastest(python["robust"]["with"]).astype("<u2").tofile(folder / "robust.u16")
    case = {"volume": {"dims": list(volume.shape), "affine": affine.tolist()},
            "thick": {"dims": list(slabs.shape), "affine": slabs_affine.tolist()},
            "image": {"dims": list(image.shape[1:4])}, "thick_image": {"dims": list(slabs_image.shape[1:4])},
            "standard": {"scores": python["standard"]["scores"].tolist()}, "robust": {"scores": python["robust"]["scores"].tolist()}}
    (folder / "case.json").write_text(json.dumps(case, indent=1), encoding="utf-8")
    return checked
