# Contributing
Contributions are absolutely, positively welcome and encouraged!

- [Debugging the Aero kernel](#debugging-the-aero-kernel)
    * [Prerequisites](##prerequisites)
    * [Debugging using GDB](##using-gdb)
    * [Debugging using LLDB](##using-lldb)

# Debugging the Aero kernel

## Prerequisites
- Debugger
    * [LLDB](https://lldb.llvm.org/) (recommended)
    * [GDB](https://www.gnu.org/software/gdb/)

To debug the Aero kernel run:
```shell
$ cargo run -- -s -S
```

Passing the `-s` flag to qemu will set up qemu to listen to at port `1234` for a GDB client to connect to it.

## Using GDB
If you are using GDB use the following commands to start debugging:
```shell
$ gdb
(gdb) target remote localhost:1234
(gdb)
```

## Using LLDB
If you are using LLDB use the following commands to start debugging:
```shell
$ lldb
(lldb) gdb-remote localhost:1234
(lldb)
````

Check out the docs for your debugger for information about how to use the debugger.