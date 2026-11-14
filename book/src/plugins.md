# Plugins: models you add yourself

Volsa comes with one model of its own, for [labelling a brain scan](labelling-a-brain-scan.md).
A plugin lets you add another: a model someone else made, which Volsa runs on your scans on this
computer. Every copy of Volsa can take plugins, and none come with it.

**A plugin is not part of Volsa.** Volsa has not checked what any plugin does, recommends none
and links to none, and never downloads one. Whatever a plugin gives is shown under "Added by you,
not part of Volsa", and, like everything Volsa shows, is not for diagnostic use. Add plugins only
from people you trust: a model is a program of a kind, and Volsa runs it with your own rights.

## What a plugin is

A folder holding the model, as an ONNX file, and a `plugin.json` that says how to prepare a scan
for it and what its answer means. Its maker gives you the folder; its licence is theirs.

## Adding one

1. Choose the **Tools** tab and, under **Added by you**, press **Add a plugin…**. Choose the
   plugin's folder.
2. Volsa checks the folder. If something is wrong, one sentence says what, and nothing is added.
3. A card shows Volsa's own words first, then what the plugin says of itself, then how Volsa will
   prepare a scan for it. Press **Add**, or **Cancel**.
4. Volsa copies the plugin into its own folder and tries it once on a made-up scan, to see what
   it needs.

## Running one

Open a scan and press **Run** in the plugin's entry. A scan of a kind the plugin was not made for
is refused, or, where its description names another contrast, Volsa says so first and runs only
if you press **Run anyway**. The result opens as labels, marked as made by a plugin wherever it
is shown or saved.

**Show result** opens the result kept for a scan again, **Delete result** deletes it, and
**Remove** deletes Volsa's copy of the plugin (results it made are kept).

## Where plugins are kept

In `%APPDATA%\Volsa\config\plugins`, a folder for each plugin added. Removing Volsa as
[Removing Volsa](removing-volsa.md) says removes them too.

## Making a plugin

[docs/plugins.md](https://github.com/Thenewmanator15/volsa/blob/main/docs/plugins.md) says
exactly what a plugin folder holds, every check Volsa makes, and what Volsa does with a scan and
an answer: for people who make plugins, and for anyone who wants to know.
