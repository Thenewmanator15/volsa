"""Write the made-up picture Voxa's JPEG 2000 decoding is tested on.

A 16 by 16 picture of 16-bit values that climb by 37 from 1000, row by row, stored as a
JPEG 2000 codestream without loss. Nothing in it comes from a scan.

    python tools/fixtures/make_ramp_j2k.py

needs Pillow built with OpenJPEG, and writes crates/voxa-dicom/tests/fixtures/ramp.j2k.
"""

import pathlib

from PIL import Image

SIDE = 16
values = [1000 + 37 * place for place in range(SIDE * SIDE)]
picture = Image.new("I;16", (SIDE, SIDE))
picture.putdata(values)
out = pathlib.Path(__file__).resolve().parents[2] / "crates/voxa-dicom/tests/fixtures/ramp.j2k"
out.parent.mkdir(parents=True, exist_ok=True)
picture.save(out, format="JPEG2000", irreversible=False, no_jp2=True)
print(f"wrote {out} ({out.stat().st_size} bytes)")
