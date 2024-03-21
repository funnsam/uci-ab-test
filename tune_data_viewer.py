import sys
import struct

fn = sys.argv[1]

if fn.endswith(".flt"):
    f = open(fn, "rb")

    chunk = f.read(4)
    while chunk:
        print(struct.unpack('f', chunk)[0])
        chunk = f.read(4)
else:
    f = open(fn, "rb")

    chunk = f.read(4)
    while chunk:
        print(struct.unpack('<i', chunk)[0])
        chunk = f.read(4)
