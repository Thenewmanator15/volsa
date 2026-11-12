"""Turn SynthSeg 2.0's files into a model folder that Volsa reads.

Builds SynthSeg's networks as SynthSeg's own build_model does (SynthSeg/predict_synthseg.py,
Copyright 2020 Benjamin Billot, Apache License 2.0), keeping its layer names; translated
and changed for Volsa, as docs/models.md says.

    python convert.py SYNTHSEG_CHECKOUT OUT_FOLDER

SYNTHSEG_CHECKOUT is a copy of github.com/BBillot/SynthSeg with the four 2.0 `.h5` files in
its `models` folder. OUT_FOLDER receives six `.onnx` networks, `model.json`, and a `check`
folder holding a made-up volume and what SynthSeg's own program made of it.

`model.json` is written last, and only if the converted networks give SynthSeg's own answer
(see check.py). A folder without it is not a model folder, and Volsa refuses it.

This script ships nothing and is no part of Volsa: it is not built, run or tested by cargo.
"""

import hashlib
import importlib
import importlib.abc
import importlib.util
import json
import os
import sys
from pathlib import Path

os.environ.setdefault("TF_USE_LEGACY_KERAS", "1")
os.environ.setdefault("TF_CPP_MIN_LOG_LEVEL", "2")

FORMAT = 1
KIND = "synthseg-2.0"
CONVERTER = 2
OPSET = 17
BOX = 224  # the side of the cube SynthSeg's scores network is given
H5 = {"standard": "synthseg_2.0.h5", "robust": "synthseg_robust_2.0.h5", "cortex": "synthseg_parc_2.0.h5",
      "scores": "synthseg_qc_2.0.h5"}
UNET = dict(nb_levels=5, nb_conv_per_level=2, feat_mult=2, activation="elu", batch_norm=-1)


class KerasIsTfKeras(importlib.abc.MetaPathFinder, importlib.abc.Loader):
    """SynthSeg was written for Keras 2. Serve `keras` and `keras.*` from `tf_keras`."""

    def find_spec(self, name, path=None, target=None):
        if name == "keras" or name.startswith("keras."):
            return importlib.util.spec_from_loader(name, self)
        return None

    def create_module(self, spec):
        return importlib.import_module("tf_keras" + spec.name[len("keras"):])

    def exec_module(self, module):
        pass


def use_synthseg(checkout: Path) -> None:
    """Make SynthSeg's own code importable on a current TensorFlow and NumPy."""
    if not any(isinstance(finder, KerasIsTfKeras) for finder in sys.meta_path):
        sys.meta_path.insert(0, KerasIsTfKeras())
    import numpy as np

    for name, kind in (("int", int), ("float", float), ("bool", bool), ("object", object)):
        if name not in np.__dict__:  # aliases NumPy dropped that SynthSeg's code still uses
            setattr(np, name, kind)
    if str(checkout) not in sys.path:
        sys.path.insert(0, str(checkout))


def read_lists(checkout: Path) -> dict:
    """The lists that go with the networks, as SynthSeg's own predict() prepares them."""
    import numpy as np
    from ext.lab2im import utils
    from SynthSeg.predict import get_flip_indices

    data = checkout / "data" / "labels_classes_priors"
    raw, _ = utils.get_list_labels(label_list=str(data / "synthseg_segmentation_labels_2.0.npy"))
    labels, flip, first = get_flip_indices(raw, 19)
    if not np.array_equal(labels, np.unique(raw)):
        raise SystemExit("SynthSeg's label list is not in the order this converter expects.")
    names = np.load(data / "synthseg_segmentation_names_2.0.npy")[first]
    classes = np.load(data / "synthseg_topological_classes_2.0.npy")[first]
    stage = np.unique(utils.get_list_labels(str(data / "synthseg_denoiser_labels_2.0.npy"))[0])
    area_labels, area_first = np.unique(utils.get_list_labels(str(data / "synthseg_parcellation_labels.npy"))[0], return_index=True)
    area_names = np.load(data / "synthseg_parcellation_names.npy")[area_first]
    score_class = utils.get_list_labels(str(data / "synthseg_qc_labels_2.0.npy"))[0][first]
    score_names = np.load(data / "synthseg_qc_names_2.0.npy")[first]
    count = len(np.unique(score_class))
    if sorted(np.unique(score_class).tolist()) != list(range(count)):
        raise SystemExit("SynthSeg's score classes are not 0, 1, 2 and so on.")
    groups = [str(score_names[np.where(score_class == k)[0][0]]) for k in range(1, count)]
    return {
        "labels": labels, "flip": flip, "classes": classes, "stage_classes": len(stage), "area_labels": area_labels,
        "score_class": score_class, "score_groups": groups,
        "structures": [dict(label=int(l), **name_and_side(str(n)), **{"class": int(c)}) for l, n, c in zip(labels, names, classes)],
        "areas": [dict(label=int(l), **name_and_side(str(n))) for l, n in zip(area_labels, area_names)],
    }


def name_and_side(name: str) -> dict:
    """SynthSeg's names carry the side in front; Volsa keeps the two apart."""
    lower = name.lower()
    for prefix, side in (("left ", "left"), ("right ", "right"), ("ctx-lh-", "left"), ("ctx-rh-", "right")):
        if lower.startswith(prefix):
            return {"name": name[len(prefix):], "side": side}
    return {"name": lower if lower == "background" else name, "side": "midline"}


def every_layer_loaded(model, h5: Path) -> None:
    """Loading by name skips silently a layer the file does not have. That must not happen."""
    import h5py

    with h5py.File(h5, "r") as file:
        group = file["model_weights"] if "model_weights" in file else file
        named = {n.decode() if isinstance(n, bytes) else n for n in group.attrs["layer_names"]}
    missing = [layer.name for layer in model.layers if layer.weights and layer.name not in named]
    if missing:
        raise SystemExit(f"{h5.name} has no weights for: {missing}")


def build(checkout: Path, lists: dict) -> dict:
    """Each network standing alone, built as SynthSeg's build_model builds it and loaded by
    layer name from its `.h5`. Returns role -> (Keras model, channels it takes)."""
    import keras.layers as KL
    import tensorflow as tf
    from ext.neuron import models as nrn
    from keras.models import Model

    files = {role: checkout / "models" / name for role, name in H5.items()}
    structures, stage, areas = len(lists["structures"]), lists["stage_classes"], len(lists["areas"])
    scores = len(lists["score_groups"]) + 1

    def unet(channels, answers, name, h5, conv_size=3, features=24, **more):
        net = nrn.unet(input_shape=[None, None, None, channels], nb_labels=answers, conv_size=conv_size,
                       nb_features=features, name=name, **UNET, **more)
        net.load_weights(str(h5), by_name=True)
        every_layer_loaded(net, h5)
        return net, channels

    check = nrn.conv_enc(input_shape=[None, None, None, scores], nb_levels=4, nb_conv_per_level=2, conv_size=5,
                         nb_features=24, feat_mult=2, activation="relu", batch_norm=-1, use_residuals=True, name="qc")
    last = {"padding": "same", "activation": "relu", "data_format": "channels_last"}
    t = KL.MaxPool3D(pool_size=(2, 2, 2), name="qc_maxpool_3", padding="same")(check.outputs[0])
    t = KL.Conv3D(scores, kernel_size=5, **last, name="qc_final_conv_0")(t)
    t = KL.Conv3D(scores, kernel_size=5, **last, name="qc_final_conv_1")(t)
    t = KL.Lambda(lambda x: tf.reduce_mean(x, axis=[1, 2, 3]), name="qc_final_pred")(t)
    check = Model(check.inputs, t)
    check.load_weights(str(files["scores"]), by_name=True)
    every_layer_loaded(check, files["scores"])

    return {
        "standard": unet(1, structures, "unet", files["standard"]),
        "robust_1": unet(1, stage, "unet", files["robust"]),
        "robust_2": unet(stage, stage, "l2l", files["robust"], conv_size=5, features=16, skip_n_concatenations=2),
        "robust_3": unet(1 + stage, structures, "unet2", files["robust"]),
        "cortex": unet(3, areas, "unet_parc", files["cortex"]),
        "scores": (check, scores),
    }


def to_onnx(model, channels: int):
    """One network through tf2onnx, as it comes."""
    import tensorflow as tf
    import tf2onnx

    spec = [tf.TensorSpec([1, None, None, None, channels], tf.float32, name="in0")]
    proto, _ = tf2onnx.convert.from_keras(model, input_signature=spec, opset=OPSET)
    return proto


def write_networks(checkout: Path, out: Path, lists: dict) -> None:
    """Convert the six networks and make each take and give what Volsa needs."""
    import numpy as np
    import onnx
    from ext.lab2im import edit_tensors

    import graphs

    structures, stage = len(lists["structures"]), lists["stage_classes"]
    half = (structures + 1) // 2
    blur = [np.squeeze(np.asarray(one)).astype(np.float32)
            for one in edit_tensors.gaussian_kernel([0.5] * 3, separable=True)]
    for role, (model, channels) in build(checkout, lists).items():
        proto = to_onnx(model, channels)
        if role != "scores" and graphs.use_resize(proto) != 4:
            raise SystemExit(f"{role}: expected four upsampling blocks in what tf2onnx wrote.")
        if role == "standard":
            graphs.feed(proto, image=True)
            graphs.add_blur(proto, blur)
            graphs.give_halves(proto, (half, structures - half))
        elif role == "robust_1":
            graphs.feed(proto, image=True)
            graphs.give_classes(proto, "classes")
        elif role == "robust_2":
            graphs.feed(proto, image=False, byte="classes", depth=stage)
            graphs.give_classes(proto, "cleaned")
        elif role == "robust_3":
            graphs.feed(proto, image=True, byte="classes", depth=stage)
            graphs.give_halves(proto, (half, structures - half))
        elif role == "cortex":
            graphs.feed(proto, image=True, byte="cortex", depth=2)
            graphs.give_areas(proto)
        else:
            graphs.feed(proto, image=False, byte="classes", depth=channels, box=BOX)
            graphs.give_scores(proto, channels)
        onnx.checker.check_model(proto)
        onnx.save(proto, str(out / f"{role}.onnx"))
        print(f"wrote {role}.onnx: {(out / f'{role}.onnx').stat().st_size / 2**20:.1f} MB, {len(proto.graph.node)} nodes")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as file:
        for block in iter(lambda: file.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    checkout, out = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
    missing = [name for name in H5.values() if not (checkout / "models" / name).is_file()]
    if missing:
        raise SystemExit(f"Not found in {checkout / 'models'}: {missing}. docs/models.md says where they come from.")
    out.mkdir(parents=True, exist_ok=True)
    (out / "model.json").unlink(missing_ok=True)  # an old list must not vouch for new files
    use_synthseg(checkout)
    import onnx
    import tensorflow as tf
    import tf2onnx

    import check

    lists = read_lists(checkout)
    write_networks(checkout, out, lists)
    checked = check.run(checkout, out, lists)  # stops here, with the reason, if a mark is missed
    roles = ["standard", "robust_1", "robust_2", "robust_3", "cortex", "scores"]
    listing = {
        "format": FORMAT,
        "kind": KIND,
        "files": {r: {"name": f"{r}.onnx", "bytes": (out / f"{r}.onnx").stat().st_size, "sha256": sha256(out / f"{r}.onnx")} for r in roles},
        "structures": lists["structures"],
        "flip": [int(i) for i in lists["flip"]],
        "stage_classes": lists["stage_classes"],
        "areas": lists["areas"],
        "cortex": [3, 42],
        "score_class": [int(c) for c in lists["score_class"]],
        "score_groups": lists["score_groups"],
        "box": BOX,
        "made": {"converter": CONVERTER, "tensorflow": tf.__version__, "tf2onnx": tf2onnx.__version__, "onnx": onnx.__version__,
                 "opset": OPSET, "source": {name: sha256(checkout / "models" / name) for name in H5.values()}},
        "checked": checked,
    }
    (out / "model.json").write_text(json.dumps(listing, indent=1), encoding="utf-8")
    print(f"model.json written: the folder {out} is ready for Volsa.")


if __name__ == "__main__":
    main()
