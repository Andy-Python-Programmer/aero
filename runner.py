import sys
import os


def main(argc, argv):
    kernel_bin = os.path.join(os.path.split(
        os.path.normpath(argv[1]))[0], "bootimage-aero.bin")

    args = ""

    if argc >= 3:
        for arg in argv[2::]:
            args += (f"{arg} ")

    command = ("qemu-system-x86_64 -drive format=raw,file=" +
               kernel_bin + " " + args)

    print(f"Running {command}")
    os.system(command)


if __name__ == "__main__":
    main(len(sys.argv), sys.argv)
