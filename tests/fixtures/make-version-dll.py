#!/usr/bin/env python3
"""Writes `assets/version.dll`: a PE32 DLL whose only content is a version
resource.

`GetDLLVersionLocal` reads the *build* machine at compile time, so its example
needs a real PE carrying `VS_VERSIONINFO` — and nothing shipped with NSIS has
one (every plugin and stub answers *"error reading version info"*). The
alternative was to commit a 39 KB installer built by `makensis`, which is a
runnable executable in a fixtures directory and forty times the size of what the
test reads.

So this generates the smallest honest thing instead: 1 KB, one `.rsrc` section,
no code, no imports, nothing to run. The file version is **1.2.3.4** and the
product version **5.6.7.8**, deliberately different, because `GetDLLVersion`'s
`/ProductVersion` flag reads the second and a fixture where the two agree cannot
tell whether the flag was honoured.

Run from this directory: `python3 make-version-dll.py`.
"""

import struct

FILE_VERSION = (1, 2, 3, 4)
PRODUCT_VERSION = (5, 6, 7, 8)

SECTION_ALIGN = 0x1000
FILE_ALIGN = 0x200
RSRC_RVA = 0x1000


def pad(data: bytes, to: int) -> bytes:
    """Zero-fill up to a multiple of `to`."""
    return data + b"\0" * (-len(data) % to)


def utf16(text: str) -> bytes:
    return text.encode("utf-16-le") + b"\0\0"


def versioned(numbers) -> tuple[int, int]:
    """A four-part version as the two 32-bit halves NSIS binds to two registers."""
    major, minor, build, revision = numbers
    return (major << 16) | minor, (build << 16) | revision


def fixed_file_info() -> bytes:
    """`VS_FIXEDFILEINFO`: the 52 bytes `GetDLLVersionLocal` actually reads."""
    file_ms, file_ls = versioned(FILE_VERSION)
    product_ms, product_ls = versioned(PRODUCT_VERSION)
    return struct.pack(
        "<13I",
        0xFEEF04BD,  # dwSignature
        0x00010000,  # dwStrucVersion 1.0
        file_ms,
        file_ls,
        product_ms,
        product_ls,
        0x3F,  # dwFileFlagsMask
        0,  # dwFileFlags: no debug, no prerelease
        0x00040004,  # VOS_NT_WINDOWS32
        0x00000002,  # VFT_DLL
        0,  # dwFileSubtype
        0,  # dwFileDateMS
        0,  # dwFileDateLS
    )


def node(key: str, value: bytes, kind: int, children: bytes = b"") -> bytes:
    """One `VS_VERSIONINFO` node: length, value length, type, key, then both.

    Every node is 4-aligned twice over — after the key and after the value —
    which is the whole of what makes this format fiddly.
    """
    head = utf16(key)
    body = pad(struct.pack("<HHH", 0, len(value) if kind == 0 else len(value) // 2, kind) + head, 4)
    body += pad(value, 4) + children
    return struct.pack("<H", len(body)) + body[2:]


def version_info() -> bytes:
    strings = b"".join(
        node(key, utf16(text), 1)
        for key, text in [
            ("CompanyName", "Installua"),
            ("FileDescription", "GetDLLVersionLocal fixture"),
            ("FileVersion", ".".join(str(part) for part in FILE_VERSION)),
            ("ProductName", "Installua test fixture"),
            ("ProductVersion", ".".join(str(part) for part in PRODUCT_VERSION)),
        ]
    )
    # 040904B0: US English, Unicode — the pair `VarFileInfo` repeats as numbers.
    table = node("040904B0", b"", 1, strings)
    string_info = node("StringFileInfo", b"", 1, table)
    var_info = node("VarFileInfo", b"", 1, node("Translation", struct.pack("<HH", 0x0409, 0x04B0), 0))
    return node("VS_VERSION_INFO", fixed_file_info(), 0, string_info + var_info)


def resource_directory(payload_rva: int, payload_size: int) -> bytes:
    """Three levels — type, name, language — each one entry wide.

    A directory that branched would say nothing more: the reader walks to
    `RT_VERSION`, takes the first name and the first language, and stops.
    """
    def header(entry_id: int, offset: int, is_directory: bool) -> bytes:
        table = struct.pack("<IIHHHH", 0, 0, 0, 0, 0, 1)
        high = 0x80000000 if is_directory else 0
        return table + struct.pack("<II", entry_id, offset | high)

    level = 16 + 8  # a 16-byte table plus the one entry that follows it
    directory = header(16, level, True)  # RT_VERSION
    directory += header(1, 2 * level, True)  # name 1
    directory += header(0x0409, 3 * level, False)  # US English, to the data entry
    directory += struct.pack("<IIII", payload_rva, payload_size, 0, 0)
    return directory


def main() -> None:
    payload = version_info()
    # The data entry sits at `levels + 16`; the blob follows it, 4-aligned.
    entry_end = 3 * (16 + 8) + 16
    payload_offset = entry_end + (-entry_end % 4)
    directory = resource_directory(RSRC_RVA + payload_offset, len(payload))
    rsrc = pad(directory, 4) + payload
    rsrc_size = len(rsrc)
    rsrc = pad(rsrc, FILE_ALIGN)

    headers_size = 0x200
    dos = b"MZ" + b"\0" * 58 + struct.pack("<I", 0x40)
    coff = struct.pack(
        "<HHIIIHH",
        0x014C,  # i386
        1,  # one section
        0,  # TimeDateStamp: zero, so the file is byte-identical every run
        0,
        0,
        0xE0,  # SizeOfOptionalHeader
        0x2102,  # EXECUTABLE_IMAGE | 32BIT_MACHINE | DLL
    )
    optional = struct.pack(
        "<HBBIIIIIII",
        0x010B,  # PE32
        0, 0,  # linker version
        0, 0, 0,  # sizes of code, initialised and uninitialised data
        0,  # AddressOfEntryPoint: none, because there is no code
        0, 0,  # bases of code and data
        0x10000000,  # ImageBase
    )
    optional += struct.pack(
        "<IIHHHHHHIIIIHHIIIIII",
        SECTION_ALIGN,
        FILE_ALIGN,
        4, 0,  # OS version
        0, 0,  # image version
        4, 0,  # subsystem version
        0,  # Win32VersionValue
        RSRC_RVA + SECTION_ALIGN,  # SizeOfImage
        headers_size,
        0,  # CheckSum
        3,  # IMAGE_SUBSYSTEM_WINDOWS_CUI
        0,  # DllCharacteristics
        0x100000, 0x1000,  # stack reserve, commit
        0x100000, 0x1000,  # heap reserve, commit
        0,  # LoaderFlags
        16,  # NumberOfRvaAndSizes
    )
    directories = [(0, 0)] * 16
    directories[2] = (RSRC_RVA, rsrc_size)  # IMAGE_DIRECTORY_ENTRY_RESOURCE
    optional += b"".join(struct.pack("<II", rva, size) for rva, size in directories)

    section = struct.pack(
        "<8sIIIIIIHHI",
        b".rsrc\0\0\0",
        rsrc_size,
        RSRC_RVA,
        len(rsrc),
        headers_size,
        0, 0, 0, 0,
        0x40000040,  # INITIALIZED_DATA | MEM_READ
    )

    image = pad(dos + b"PE\0\0" + coff + optional + section, headers_size) + rsrc
    with open("assets/version.dll", "wb") as out:
        out.write(image)
    print(f"assets/version.dll: {len(image)} bytes")


if __name__ == "__main__":
    main()
