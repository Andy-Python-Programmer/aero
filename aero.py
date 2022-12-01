#!/usr/bin/env python3

# Copyright (C) 2021-2022 The Aero Project Developers.
#
# This file is part of The Aero Project.
#
# Aero is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# Aero is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with Aero. If not, see <https://www.gnu.org/licenses/>.

import argparse
import json
import os
import platform
import shutil
import subprocess
import sys
import tarfile
import time

from typing import List


def log_info(msg):
    """
    Logs a message with info log level.
    """
    print(f"\033[1m\033[92minfo\033[0m: {msg}")


def log_error(msg):
    """
    Logs a message with error log level.
    """
    print(f"\033[1m\033[91merror\033[0m: {msg}")


# Make sure requests is installed
try:
    import requests
    import xbstrap
except ImportError:
    log_error('Please install required libraires using the following command:')
    log_error(' - python3 -m pip install requests xbstrap')

    sys.exit(0)


OVMF_URL = 'https://github.com/aero-os/ovmf-prebuilt'
LIMINE_URL = 'https://github.com/limine-bootloader/limine'

BUILD_DIR = 'build'
BUNDLED_DIR = 'bundled'
SYSROOT_DIR = 'sysroot'
EXTRA_FILES = 'extra-files'
SYSROOT_CARGO_HOME = os.path.join(SYSROOT_DIR, 'cargo-home')
BASE_FILES_DIR = 'base-files'

LIMINE_TEMPLATE = """
TIMEOUT=0
VERBOSE=yes

:aero
PROTOCOL=limine
KASLR=no
KERNEL_PATH=boot:///aero.elf
CMDLINE=term-background=background theme-background=0x50000000

MODULE_PATH=boot:///term_background.bmp
MODULE_CMDLINE=background
"""


class BuildInfo:
    args: argparse.Namespace
    target_arch: str

    def __init__(self, target_arch: str, args: argparse.Namespace):
        self.target_arch = target_arch
        self.args = args


def get_userland_tool(): return os.path.join(SYSROOT_DIR, "tools")
def get_userland_package(): return os.path.join(SYSROOT_DIR, "packages")

def remove_prefix(string: str, prefix: str):
    if string.startswith(prefix):
        return string[len(prefix):]
    else:
        return string[:]


def parse_args():
    parser = argparse.ArgumentParser(
        description="utility used to build aero kernel and userland")

    check_test = parser.add_mutually_exclusive_group()

    check_test.add_argument('--clean',
                            default=False,
                            action='store_true',
                            help='removes the build artifacts')

    check_test.add_argument('--check',
                            default=False,
                            action='store_true',
                            help='checks if aero builds correctly without packaging and running it')

    check_test.add_argument('--test',
                            default=False,
                            action='store_true',
                            help='runs the aero test suite')

    check_test.add_argument('--document',
                            default=False,
                            action='store_true',
                            help='generates the documentation for the aero kernel')

    parser.add_argument('--debug',
                        default=False,
                        action='store_true',
                        help='builds the kernel and userland in debug mode')

    parser.add_argument('--no-run',
                        default=False,
                        action='store_true',
                        help='doesn\'t run the built image in emulator when applicable')

    parser.add_argument('--only-run',
                        default=False,
                        action='store_true',
                        help='runs aero without rebuilding. ignores any build-related flags')

    parser.add_argument('--bios',
                        type=str,
                        default='legacy',
                        choices=['legacy', 'uefi'],
                        help='run aero using the selected BIOS')

    parser.add_argument('--features',
                        type=lambda x: x.split(','),
                        default=[],
                        help='additional features to build the kernel with')

    parser.add_argument('--target',
                        default='x86_64-aero_os',
                        help='override the target triple the kernel will be built for')

    parser.add_argument('--la57',
                        default=False,
                        action='store_true',
                        help='run emulator with 5 level paging support')

    parser.add_argument('--sysroot',
                        default=False,
                        action='store_true',
                        help='build the full userland sysroot. If disabled, then the sysroot will only contain the aero_shell and the init binaries')

    parser.add_argument('--disable-kvm',
                        default=False,
                        action='store_true',
                        help='disable KVM acceleration even if its available')

    parser.add_argument('remaining',
                        nargs=argparse.REMAINDER,
                        help='additional arguments to pass as the emulator')

    parser.add_argument('--memory',
                        default='9800M',
                        help='amount of memory to allocate to QEMU')

    return parser.parse_args()


def run_command(args, **kwargs):
    output = subprocess.run(args, **kwargs)

    return output.returncode, output.stdout, output.stderr


def download_bundled():
    if not os.path.exists(BUNDLED_DIR):
        os.makedirs(BUNDLED_DIR)

    ovmf_path = os.path.join(BUNDLED_DIR, 'ovmf')
    limine_path = os.path.join(BUNDLED_DIR, 'limine')

    if not os.path.exists(ovmf_path):
        run_command(['git', 'clone', '--depth', '1', OVMF_URL, ovmf_path])

    if not os.path.exists(limine_path):
        run_command(['git', 'clone', '--branch', 'v4.x-branch-binary',
                    '--depth', '1', LIMINE_URL, limine_path])

    if not os.path.exists(SYSROOT_DIR):
        log_info("building minimal sysroot")
        build_userland_sysroot(True)


def extract_artifacts(stdout):
    result = []
    lines = stdout.splitlines()

    for line in lines:
        info = json.loads(line)
        executable = info['executable'] if 'executable' in info else None

        if executable:
            result.append(info['executable'])

    return result


def build_cargo_workspace(cwd, command, args, cargo="cargo"):
    code, _, _ = run_command([cargo, command, *args], cwd=cwd)

    if code != 0:
        return None

    _, stdout, _ = run_command([cargo, command, *args, '--message-format=json'],
                               stdout=subprocess.PIPE,
                               stderr=subprocess.DEVNULL,
                               cwd=cwd)

    return extract_artifacts(stdout)


def build_kernel(args):
    command = 'build'
    cmd_args = ['--package', 'aero_kernel',
                '--target', f'.cargo/{args.target}.json']

    if not args.debug:
        cmd_args += ['--release']

    if args.test:
        command = 'test'
        cmd_args += ['--no-run']
    elif args.check:
        command = 'check'
    elif args.document:
        command = 'doc'

    if args.features:
        cmd_args += ['--features', ','.join(args.features)]

    return build_cargo_workspace('src', command, cmd_args)


# Helper function for symlink since os.symlink uses path
# relative to the destination directory.
def symlink_rel(src, dst):
    rel_path_src = os.path.relpath(src, os.path.dirname(dst))
    os.symlink(rel_path_src, dst)


def build_userland_sysroot(minimal):
    if not os.path.exists(SYSROOT_DIR):
        os.mkdir(SYSROOT_DIR)

    # FIXME(xbstrap): xbstrap does not copy over the extra-files/rust/config.toml
    # file into the cargo home directory.
    if not os.path.exists(SYSROOT_CARGO_HOME):
        os.mkdir(SYSROOT_CARGO_HOME)

    cargo_sys_cfg = os.path.join(SYSROOT_CARGO_HOME, 'config.toml')
    if not os.path.exists(cargo_sys_cfg):
        cargo_cfg_fd = open(os.path.join(
            EXTRA_FILES, 'rust', 'config.toml'), 'r')
        cargo_cfg = cargo_cfg_fd.read()
        cargo_cfg_fd.close()

        cargo_cfg = cargo_cfg.replace("@SOURCE_ROOT@", os.getcwd())
        cargo_cfg = cargo_cfg.replace(
            "@BUILD_ROOT@", os.path.join(os.getcwd(), SYSROOT_DIR))

        cargo_cfg_fd = open(cargo_sys_cfg, "w+")
        cargo_cfg_fd.write(cargo_cfg)
        cargo_cfg_fd.close()

    blink = os.path.join(SYSROOT_DIR, 'bootstrap.link')

    if not os.path.islink(blink):
        # symlink the bootstrap.yml file in the src root to sysroot/bootstrap.link
        symlink_rel('bootstrap.yml', blink)
    
    def run_xbstrap(args):
        try:
            run_command(['xbstrap', *args], cwd=SYSROOT_DIR)
        except FileNotFoundError:
            run_command([f'{os.environ["HOME"]}/.local/bin/xbstrap', *args], cwd=SYSROOT_DIR)

    if minimal:
        run_xbstrap(['install', '-u', 'bash', 'coreutils'])
    else:
        run_xbstrap(['install', '-u', '--all'])


def build_userland(args):
    # We need to check if we have host-rust in-order for us to build
    # our rust userland applications in `userland/`.
    host_cargo = os.path.join(SYSROOT_DIR, "tools/host-rust")

    if not os.path.exists(host_cargo):
        log_error("host-rust not built as a part of the sysroot, skipping compilation of `userland/`")
        return []

    HOST_RUST = "host-rust/bin/rustc"
    HOST_GCC = "host-gcc/bin/x86_64-aero-gcc"
    HOST_BINUTILS = "host-binutils/x86_64-aero/bin"
    PACKAGE_MLIBC = "mlibc"

    tool_dir = get_userland_tool()
    pkg_dir = get_userland_package()

    def get_rustc(): return os.path.join('..', tool_dir, HOST_RUST)
    def get_gcc(): return os.path.join('..', tool_dir, HOST_GCC)
    def get_binutils(): return os.path.join("..", tool_dir, HOST_BINUTILS)
    def get_mlibc(): return os.path.join("..", pkg_dir, PACKAGE_MLIBC)

    command = 'build'
    # TODO: handle the unbased architectures.
    cmd_args = ["--target", "x86_64-unknown-aero-system",

                # cargo config
                "--config", f"build.rustc = '{get_rustc()}'",
                "--config", "build.target = 'x86_64-unknown-aero-system'",
                "--config", f"build.rustflags = ['-C', 'link-args=-no-pie -B {get_binutils()} --sysroot {get_mlibc()}', '-lc']",
                "--config", f"target.x86_64-unknown-aero-system.linker = '{get_gcc()}'",

                "-Z", "unstable-options"]

    if not args.debug:
        cmd_args += ['--release']

    if args.check:
        command = 'check'

    if args.test:
        return build_cargo_workspace('userland', 'build', ['--package', 'utest', *cmd_args])
    else:
        return build_cargo_workspace('userland', command, cmd_args)

    # TODO: Userland check
    # elif args.check:
    #     command = 'check'


def generate_docs(args):
    doc_dir = os.path.join('src', 'target', args.target, 'doc')
    out_dir = os.path.join(BUILD_DIR, 'web')

    if os.path.exists(out_dir):
        shutil.rmtree(out_dir)

    shutil.copytree('web', out_dir, dirs_exist_ok=True)
    shutil.copytree(doc_dir, out_dir, dirs_exist_ok=True)


def prepare_iso(args, kernel_bin, user_bins):
    log_info("preparing ISO")

    if not os.path.exists(BUILD_DIR):
        os.makedirs(BUILD_DIR)

    iso_path = os.path.join(BUILD_DIR, 'aero.iso')
    iso_root = os.path.join(BUILD_DIR, 'iso_root')
    limine_path = os.path.join(BUNDLED_DIR, 'limine')

    if os.path.exists(iso_root):
        shutil.rmtree(iso_root)

    os.makedirs(iso_root)

    shutil.copy(kernel_bin, os.path.join(iso_root, 'aero.elf'))
    shutil.copy(os.path.join('src', '.cargo', 'term_background.bmp'), iso_root)
    shutil.copy(os.path.join(limine_path, 'limine.sys'), iso_root)
    shutil.copy(os.path.join(limine_path, 'limine-cd.bin'), iso_root)
    shutil.copy(os.path.join(limine_path, 'limine-cd-efi.bin'), iso_root)

    efi_boot = os.path.join(iso_root, "EFI", "BOOT")
    os.makedirs(efi_boot)

    shutil.copy(os.path.join(limine_path, 'BOOTAA64.EFI'), efi_boot)
    shutil.copy(os.path.join(limine_path, 'BOOTX64.EFI'), efi_boot)

    sysroot_dir = os.path.join(SYSROOT_DIR, 'system-root')
    for file in user_bins:
        bin_name = os.path.basename(file)
        dest_dir = os.path.join(sysroot_dir, "usr", "bin")
        os.makedirs(dest_dir, exist_ok=True)
        shutil.copy(file, os.path.join(dest_dir, bin_name))

    with open(os.path.join(iso_root, 'limine.cfg'), 'w') as limine_cfg:
        limine_cfg.write(LIMINE_TEMPLATE)

    code, _, xorriso_stderr = run_command([
        'xorriso', '-as', 'mkisofs', '-b', 'limine-cd.bin', '-no-emul-boot', '-boot-load-size', '4',
        '-boot-info-table', '--efi-boot', 'limine-cd-efi.bin', '-efi-boot-part',
        '--efi-boot-image', '--protective-msdos-label', iso_root, '-o', iso_path
    ], stdout=subprocess.PIPE, stderr=subprocess.PIPE)

    if code != 0:
        log_error('failed to create the ISO image')
        log_error(xorriso_stderr.decode('utf-8'))

        return None

    limine_deploy = os.path.join(limine_path, 'limine-deploy')

    if not os.path.exists(limine_deploy):
        code, _, limine_build_stderr = run_command(['make', '-C', limine_path],
                                                   stdout=subprocess.PIPE,
                                                   stderr=subprocess.PIPE)
        if code != 0:
            log_error('failed to build `limine-deploy`')
            log_error(limine_build_stderr.decode('utf8'))
            exit(1)

    code, _, limine_deploy_stderr = run_command([limine_deploy, iso_path],
                                                stdout=subprocess.PIPE,
                                                stderr=subprocess.PIPE)

    if code != 0:
        log_error('failed to install Limine')
        log_error(limine_deploy_stderr)

        return None

    # create the disk image
    disk_path = os.path.join(BUILD_DIR, 'disk.img')

    if not os.path.exists(disk_path):
        log_info('creating disk image')
        os.system('bash ./tools/mkimage.sh')

    return iso_path


def run_in_emulator(build_info: BuildInfo, iso_path):
    is_kvm_available = is_kvm_supported()
    args = build_info.args

    qemu_args = ['-cdrom', iso_path,
                 '-m', args.memory,
                 '-smp', '1',
                 '-serial', 'stdio',
                 '-drive', 'file=build/disk.img,if=none,id=NVME1,format=raw', '-device', 'nvme,drive=NVME1,serial=nvme',
                 # Specify the boot order (where `d` is the first CD-ROM drive)
                 '--boot', 'd']

    if args.bios == 'uefi':
        qemu_args += ['-bios',
                      f'bundled/ovmf/ovmf-{build_info.target_arch}/OVMF.fd']

    cmdline = args.remaining

    if '--' in cmdline:
        cmdline.remove('--')

    if cmdline:
        qemu_args += cmdline

    if is_kvm_available and not args.disable_kvm:
        log_info("running with KVM acceleration enabled")

        if platform.system() == 'Darwin':
            qemu_args += ['-accel', 'hvf', '-cpu',
                          'qemu64,+la57' if args.la57 else 'qemu64']
        else:
            qemu_args += ['-enable-kvm', '-cpu',
                          'host,+la57' if args.la57 else 'host']
    else:
        if build_info.target_arch == "aarch64":
            qemu_args += ['-device', 'ramfb',
                          '-M', 'virt', '-cpu', 'cortex-a72']
        elif build_info.target_arch == "x86_64":
            qemu_args += ["-cpu", "qemu64,+la57" if args.la57 else "qemu64"]
        else:
            log_error("unknown target architecture")
            exit(1)

    qemu_binary = f'qemu-system-{build_info.target_arch}'
    run_command([qemu_binary, *qemu_args])


def get_sysctl(name: str) -> str:
    """
    Shell out to sysctl(1)

    Returns the value as a string.
    Non-leaf nodes will return the value for each sub-node separated by newline characters.
    """
    status, stdout, stderr = run_command(["sysctl", "-n", name],
                                         stdout=subprocess.PIPE,
                                         stderr=subprocess.PIPE)
    if status != 0:
        print("`sysctl` failed: ", end="")
        print(stderr.decode())

    return stdout.strip().decode()


def is_kvm_supported() -> bool:
    """
    Returns True if KVM is supported on this machine
    """

    platform = sys.platform

    if platform == "darwin":
        # Check for VMX support
        cpu_features = get_sysctl("machdep.cpu.features")
        vmx_support = "VMX" in cpu_features.split(' ')

        # Check for HVF support
        hv_support = get_sysctl("kern.hv_support") == "1"

        return hv_support and vmx_support

    if platform == "linux":
        kvm_path = "/dev/kvm"

        # Check if the `/dev/kvm` device exists.
        if not os.path.exists(kvm_path):
            return False

        # Read out the cpuinfo from `/proc/cpuinfo`
        fd = open("/proc/cpuinfo")
        cpuinfo = fd.read()

        # Parse the cpuinfo
        cpuinfo_array = cpuinfo.split("\n\n")
        processors_info = []

        for cpu in cpuinfo_array:
            ret = {}
            for line in cpu.split("\n"):
                try:
                    name, value = line.split(":")

                    name = name.strip()
                    value = value.strip()

                    ret[name] = value
                except ValueError:
                    pass

            processors_info.append(ret)

        for processor in processors_info:
            if processor["processor"] == "0":
                # KVM acceleration can be used
                if "vmx" in processor["flags"]:
                    return True
                # KVM acceleration cannot be used
                else:
                    return False

        fd.close()

    # KVM is not avaliable on Windows
    return False


def main():
    t0 = time.time()
    args = parse_args()

    # arch-aero_os
    target_arch = args.target.split('-')[0]
    build_info = BuildInfo(target_arch, args)

    if build_info.target_arch == "aarch64" and not args.bios == "uefi":
        log_error("aarch64 requires UEFI (help: run again with `--bios=uefi`)")
        return

    download_bundled()

    if args.only_run:
        iso_path = os.path.join(BUILD_DIR, 'aero.iso')

        if not os.path.exists(iso_path):
            user_bins = build_userland(args)
            kernel_bin = build_kernel(args)

            if not kernel_bin or args.check:
                return

            kernel_bin = kernel_bin[0]
            iso_path = prepare_iso(args, kernel_bin, user_bins)
        run_in_emulator(build_info, iso_path)
    elif args.clean:
        src_target = os.path.join('src', 'target', args.target)
        userland_target = os.path.join('userland', 'target')

        if os.path.exists(src_target):
            shutil.rmtree(src_target)

        if os.path.exists(userland_target):
            shutil.rmtree(userland_target)
    elif args.sysroot:
        build_userland_sysroot(False)
    elif args.document:
        build_kernel(args)

        generate_docs(args)
    else:
        user_bins = build_userland(args)
        kernel_bin = build_kernel(args)

        if not kernel_bin or args.check:
            return

        kernel_bin = kernel_bin[0]
        iso_path = prepare_iso(args, kernel_bin, user_bins)

        t1 = time.time()
        log_info(f"build completed in {t1 - t0:.2f} seconds")
        if not args.no_run:
            run_in_emulator(build_info, iso_path)


if __name__ == '__main__':
    try:
        main()
    except KeyboardInterrupt:
        pass
