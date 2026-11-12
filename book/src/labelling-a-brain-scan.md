# Labelling a brain scan

Volsa shows two kinds of labels, and says which is which wherever it draws them. Labels made by
another program are read from a file and shown as made elsewhere; Volsa does not vouch for
them. Labels of the structures of a brain, and of the areas of its cortex, Volsa makes itself
on this computer when asked, with SynthSeg 2.0's networks run through ONNX Runtime and, on
Windows, DirectML. A label made by a model is a guess that looks certain, so each one comes
with the model's own check of its work, which is said first when it fails.

To label a scan, open it, choose the **Tools** tab and press **Label this scan**. That needs
three things:

1. **ONNX Runtime**, which comes in the zip, in a `runtime` folder beside `volsa.exe`, and uses
   a graphics card through the DirectML that Windows has. Volsa carries no DirectML of its own.
2. **Microsoft's free Visual C++ 2015-2022 x64 redistributable**, which ONNX Runtime needs
   (Volsa itself does not). Many computers already have it; Volsa says so when it is missing.
   Microsoft offers it, as the X64 one, at
   <https://learn.microsoft.com/cpp/windows/latest-supported-vc-redist>.
3. **A model folder**, which Volsa does not come with and never downloads. You make it once,
   from your own copy of SynthSeg's files, as a `models` folder beside `volsa.exe` (or a folder
   named under **Options**). [The files labelling needs](setting-up-labelling.md), which also
   comes in the zip as `MODELS.md`, says where the files come from, how to make the model
   folder, how much memory a run needs, and why.

Without them Volsa starts and does everything else, and the button says what is missing.

Labels made by a model are not clinical results; see the
[intended-use page](intended-use.md).
Labels Volsa made are kept to be shown again unless you switch that off; see
[What Volsa keeps on your computer](what-volsa-keeps-on-your-computer.md).
