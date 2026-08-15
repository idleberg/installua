#!/usr/bin/env python3
"""Generate the opaque payload fixtures the goldens need as inputs.

The rule this file implements: **a fixture whose bytes nothing reads is
generated; a fixture whose content is read is committed.**

  generated  install.ico, uninstall.ico  -- MUI_ICON needs a structurally valid
                                            .ico and does not care what is in it
             Example1.exe, tool.exe      -- `File` payloads, never inspected
  committed  LICENSE.txt                 -- shown on the license page
             manifest.txt                -- parsed by program 3's loop
             notes.txt, README.txt       -- installed and named in output
             payload.bin                 -- its *size* is asserted by the
                                            program 4 runtime check

This is safe because nothing in the five programs depends on what these four
files contain, only that they exist — and because pinning their bytes would buy
nothing anyway. `makensis` embeds each packed file's mtime and git does not
preserve mtimes, so the assembled `.exe` already hashes differently in every
checkout, committed fixtures or not. See examples/README.md.

Idempotent. `examples/assemble.sh` runs it first.
"""

import os
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))


def ico(path, rgba, size=32):
    """A single-image 32x32 BGRA icon — the smallest thing MUI_ICON accepts.

    ICONDIR, one ICONDIRENTRY, then a BITMAPINFOHEADER whose height is doubled
    because an icon carries a colour bitmap and an AND mask stacked together.
    """
    r, g, b, a = rgba
    xor = bytes([b, g, r, a]) * (size * size)
    and_mask = b"\x00" * (size * size // 8)
    bmp = struct.pack(
        "<IiiHHIIiiII",
        40,             # biSize
        size,           # biWidth
        size * 2,       # biHeight — colour bitmap plus AND mask
        1,              # biPlanes
        32,             # biBitCount
        0,              # biCompression
        len(xor) + len(and_mask),
        0, 0, 0, 0,
    )
    image = bmp + xor + and_mask
    blob = struct.pack("<HHH", 0, 1, 1)
    blob += struct.pack("<BBBBHHII", size, size, 0, 0, 1, 32, len(image), 22)
    blob += image
    write(path, blob)


def write(path, data):
    full = os.path.join(HERE, path)
    if os.path.exists(full) and open(full, "rb").read() == data:
        return
    os.makedirs(os.path.dirname(full), exist_ok=True)
    open(full, "wb").write(data)
    print("generated %s (%d bytes)" % (path, len(data)))


def main():
    ico("01-mui-uninstaller/assets/install.ico", (0x20, 0x80, 0xF0, 0xFF))
    ico("01-mui-uninstaller/assets/uninstall.ico", (0xF0, 0x60, 0x20, 0xFF))

    # `File` payloads. Not real executables and never run — program 2's
    # `nsExec` call would fail at install time, which is why program 2 is not
    # one of the two with a runtime check.
    write("01-mui-uninstaller/assets/Example1.exe", b"Example1 payload, not an executable\n")
    write("02-plugins/assets/tool.exe", b"tool payload, not an executable\n")


if __name__ == "__main__":
    main()
    sys.exit(0)
