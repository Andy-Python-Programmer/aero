Contributions are absolutely, positively welcome and encouraged!

## Debugging the aero kernel

## Prerequisites
- Debugger
    * [LLDB](https://lldb.llvm.org/) (recommended)
    * [GDB](https://www.gnu.org/software/gdb/)

To debug the aero kernel run:
```shell
$ cargo run -- -s
```

Passing the `-s` flag to qemu will set up qemu to listen to at port `1234` for a GDB client to connect to it.

## Using GDB
If you are using GDB use the following commands to start debugging:
```shell
$ gdb
(gdb) target remote localhost:1234
(gdb)
```

## Use LLDB
If you are using LLDB use the following commands to start debugging:
```shell
$ lldb
(lldb) gdb-remote localhost:1234
(lldb)
````

Check out the docs for your debugger for information about how to use the debugger.