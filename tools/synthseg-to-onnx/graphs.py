"""The changes made to what tf2onnx writes, so that each network is a plain graph that takes
and gives only what Voxa needs. Every function edits a model in place.

A network as tf2onnx leaves it has one input, `[1, x, y, z, channels]` of float32, and one
output of the same kind. After these changes every free side is named `x`, `y` or `z`, so the
program that opens a network can tell the runtime its size by those three names.
"""

import re

import numpy as np
import onnx
from onnx import TensorProto, helper, numpy_helper

SIDES = ["x", "y", "z"]


def use_resize(model: onnx.ModelProto) -> int:
    """Each Keras UpSampling3D comes out of tf2onnx as three rounds of Unsqueeze, Tile and
    Reshape on six-dimensional tensors, with the new size worked out at run time. Replace each
    block with one Resize (nearest, twice the size along x, y and z). Returns how many."""
    graph = model.graph
    by_name = {node.name: node for node in graph.node}
    swapped = 0
    for place, node in enumerate(graph.node):
        if node.op_type != "Transpose" or not re.search(r"_up_\d+/Reshape_2:0$", node.input[0]):
            continue
        if list(node.attribute[0].ints) != [0, 4, 1, 2, 3]:
            raise ValueError("an upsampling block does not end by going back to channels-first")
        scope = node.input[0][: -len("/Reshape_2:0")]
        source = by_name[scope + "/ExpandDims"].input[0]  # the block's input, channels-first
        resize = helper.make_node("Resize", [source, "", "upsample_scales"], list(node.output), name=scope + "/Resize",
                                  mode="nearest", nearest_mode="floor", coordinate_transformation_mode="asymmetric")
        graph.node.remove(node)
        graph.node.insert(place, resize)
        swapped += 1
    graph.initializer.append(numpy_helper.from_array(np.array([1, 1, 2, 2, 2], np.float32), "upsample_scales"))
    prune(model)
    return swapped


def prune(model: onnx.ModelProto) -> None:
    """Drop every node and stored tensor the outputs no longer depend on."""
    graph = model.graph
    producer = {out: node for node in graph.node for out in node.output}
    needed, waiting = set(), [out.name for out in graph.output]
    while waiting:
        node = producer.get(waiting.pop())
        if node is not None and id(node) not in needed:
            needed.add(id(node))
            waiting.extend(node.input)
    for node in [n for n in graph.node if id(n) not in needed]:
        graph.node.remove(node)
    used = {name for node in graph.node for name in node.input}
    for stored in [i for i in graph.initializer if i.name not in used]:
        graph.initializer.remove(stored)


def feed(model: onnx.ModelProto, image: bool, byte: str | None = None, depth: int = 0, box: int | None = None) -> None:
    """Replace the network's one input by what Voxa hands it: the image (`image`, float32,
    one channel), a map of classes (`byte`, one byte a voxel, spread inside the file into
    `depth` channels of 0 and 1), or both joined, the image first. With `box` the three sides
    are fixed at that size."""
    graph = model.graph
    old = graph.input[0].name
    sides = SIDES if box is None else [box] * 3
    del graph.input[:]
    del graph.value_info[:]  # they name the old free sides
    parts, made = [], []
    if image:
        graph.input.append(helper.make_tensor_value_info("image", TensorProto.FLOAT, [1, *sides, 1]))
        parts.append("image")
    if byte:
        graph.input.append(helper.make_tensor_value_info(byte, TensorProto.UINT8, [1, *sides]))
        graph.initializer.append(numpy_helper.from_array(np.array([depth], np.int64), byte + "_depth"))
        graph.initializer.append(numpy_helper.from_array(np.array([0, 1], np.float32), byte + "_values"))
        # 64-bit places, a 64-bit depth and float values: the one mix every provider has a OneHot for.
        made.append(helper.make_node("Cast", [byte], [byte + "_whole"], to=TensorProto.INT64, name=byte + "_cast"))
        made.append(helper.make_node("OneHot", [byte + "_whole", byte + "_depth", byte + "_values"], [byte + "_spread"],
                                     axis=-1, name=byte + "_spread"))
        parts.append(byte + "_spread")
    if parts == ["image"]:
        for node in graph.node:
            for i, name in enumerate(node.input):
                if name == old:
                    node.input[i] = "image"
    else:
        join = "Identity" if len(parts) == 1 else "Concat"
        made.append(helper.make_node(join, parts, [old], name="joined_input", **({} if len(parts) == 1 else {"axis": -1})))
    for place, node in enumerate(made):
        graph.node.insert(place, node)


def _take_output(model: onnx.ModelProto) -> str:
    graph = model.graph
    old = graph.output[0].name
    del graph.output[:]
    return old


def add_blur(model: onnx.ModelProto, kernel: np.ndarray) -> None:
    """SynthSeg's GaussianBlur(sigma=0.5) of each channel of the answer, as one grouped
    convolution with its 3 x 3 x 3 kernel, zero beyond the edges."""
    graph = model.graph
    channels = [d.dim_value for d in graph.output[0].type.tensor_type.shape.dim][-1]
    old = _take_output(model)
    weights = np.zeros((channels, 1, 3, 3, 3), np.float32)
    weights[:, 0] = kernel
    graph.initializer.append(numpy_helper.from_array(weights, "blur_kernel"))
    graph.node.extend([
        helper.make_node("Transpose", [old], ["blur_in"], perm=[0, 4, 1, 2, 3], name="blur_in"),
        helper.make_node("Conv", ["blur_in", "blur_kernel"], ["blur_out"], group=channels, kernel_shape=[3, 3, 3],
                         pads=[1] * 6, name="blur"),
        helper.make_node("Transpose", ["blur_out"], ["blurred"], perm=[0, 2, 3, 4, 1], name="blur_back"),
    ])
    graph.output.append(helper.make_tensor_value_info("blurred", TensorProto.FLOAT, [1, *SIDES, channels]))


def give_halves(model: onnx.ModelProto, sizes: tuple[int, int]) -> None:
    """Give the probabilities as two outputs split along the channels, `probabilities_a` and
    `probabilities_b`: DirectML hands back no more than about 2 GiB from one output."""
    graph = model.graph
    old = _take_output(model)
    graph.initializer.append(numpy_helper.from_array(np.array(sizes, np.int64), "halves"))
    names = ["probabilities_a", "probabilities_b"]
    graph.node.append(helper.make_node("Split", [old, "halves"], names, axis=-1, name="halves"))
    for name, size in zip(names, sizes):
        graph.output.append(helper.make_tensor_value_info(name, TensorProto.FLOAT, [1, *SIDES, size]))


def give_classes(model: onnx.ModelProto, name: str) -> None:
    """Give, in place of the probabilities, the place of the largest at each voxel, as a byte."""
    graph = model.graph
    old = _take_output(model)
    graph.node.extend([
        helper.make_node("ArgMax", [old], [name + "_place"], axis=-1, keepdims=0, name=name + "_largest"),
        helper.make_node("Cast", [name + "_place"], [name], to=TensorProto.UINT8, name=name + "_byte"),
    ])
    graph.output.append(helper.make_tensor_value_info(name, TensorProto.UINT8, [1, *SIDES]))


def give_areas(model: onnx.ModelProto) -> None:
    """Give 1 plus the place of the largest of the channels after the first: the cortex area
    at each voxel, which is all SynthSeg's tidying uses of this network's answer."""
    graph = model.graph
    old = _take_output(model)
    for name, value in (("areas_from", [1]), ("areas_to", [np.iinfo(np.int32).max]), ("areas_axis", [-1]), ("areas_one", [1])):
        graph.initializer.append(numpy_helper.from_array(np.array(value, np.int64), name))
    graph.node.extend([
        helper.make_node("Slice", [old, "areas_from", "areas_to", "areas_axis"], ["areas_only"], name="areas_only"),
        helper.make_node("ArgMax", ["areas_only"], ["areas_place"], axis=-1, keepdims=0, name="areas_largest"),
        helper.make_node("Add", ["areas_place", "areas_one"], ["areas_whole"], name="areas_from_one"),
        helper.make_node("Cast", ["areas_whole"], ["areas"], to=TensorProto.UINT8, name="areas_byte"),
    ])
    graph.output.append(helper.make_tensor_value_info("areas", TensorProto.UINT8, [1, *SIDES]))


def give_scores(model: onnx.ModelProto, count: int) -> None:
    """Name the scores network's answer `scores`: one number for each class it was given."""
    graph = model.graph
    old = _take_output(model)
    graph.node.append(helper.make_node("Identity", [old], ["scores"], name="scores"))
    graph.output.append(helper.make_tensor_value_info("scores", TensorProto.FLOAT, [1, count]))
