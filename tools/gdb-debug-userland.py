#!/usr/bin/python3

import sys
import os
import subprocess


def gdb_main(program: str):
    print("connecting to the GDB server")
    # connect to gdb-server
    gdb.execute("target remote localhost:1234")

    print("loading the symbol table")
    # load the symbol table
    gdb.execute("symbol-file sysroot/system-root/usr/bin/{}".format(program))

    print("setting a breakpoint at the entry point")
    # set a breakpoint at the entry point of the program
    gdb.execute("b main")


if __name__ == "__main__":
    if os.getenv("IN_GDB") == "yes":
        import gdb

        # Since GDB does not let us pass arguments to the script and we have a big
        # brain, so we pass the arguments to the program as enviornment variables.
        program = os.getenv("AERO_DEBUG_USERLAND_PROGRAM")
        gdb_main(program)

    else:
        os.environ["AERO_DEBUG_USERLAND_PROGRAM"] = sys.argv[1]
        os.environ["IN_GDB"] = "yes"

        process = subprocess.Popen(
            "gdb -tui -q -x tools/gdb-debug-userland.py", shell=True)

        process.wait()
        os.environ["IN_GDB"] = "no"
