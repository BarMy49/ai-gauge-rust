import ctypes
import struct
import sys
from pathlib import Path

data = Path(sys.argv[1]).read_bytes()
pe = struct.unpack_from('<I', data, 0x3c)[0]
section_count = struct.unpack_from('<H', data, pe+6)[0]
optional_size = struct.unpack_from('<H', data, pe+20)[0]
optional = pe+24
sections = [struct.unpack_from('<8sIIIIIIHHI', data, optional+optional_size+40*i) for i in range(section_count)]
def offset(rva):
    for _, virtual_size, virtual, raw_size, raw, *_ in sections:
        if virtual <= rva < virtual + max(virtual_size, raw_size):
            return rva - virtual + raw
    raise ValueError(hex(rva))
def string(rva):
    start = offset(rva)
    return data[start:data.index(b'\0', start)]
imports = offset(struct.unpack_from('<I', data, optional+120)[0])
kernel = ctypes.WinDLL('kernel32', use_last_error=True)
kernel.GetProcAddress.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
kernel.GetProcAddress.restype = ctypes.c_void_p
for entry in range(imports, len(data), 20):
    thunk, _, _, name, first = struct.unpack_from('<IIIII', data, entry)
    if name == 0:
        break
    dll = string(name).decode()
    try:
        library = ctypes.WinDLL(dll)
    except OSError as error:
        print('CANNOT LOAD', dll, error)
        continue
    count = 0
    for pos in range(offset(thunk or first), len(data), 8):
        value = struct.unpack_from('<Q', data, pos)[0]
        if not value:
            break
        symbol = ctypes.cast(value & 0xffff, ctypes.c_char_p) if value >> 63 else string(value+2)
        if not kernel.GetProcAddress(library._handle, symbol):
            print('MISSING', dll, symbol)
        count += 1
    print(dll, count, 'imports checked')
