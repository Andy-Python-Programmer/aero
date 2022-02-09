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

from typing import List


# Make sure requests is installed
try:
    import requests
    import xbstrap
except ImportError:
    print('Please install required libraires using the following command:')
    print(' - python3 -m pip install requests xbstrap')

    sys.exit(0)

import requests
import xbstrap


OVMF_URL = 'https://github.com/rust-osdev/ovmf-prebuilt/releases/latest/download'
LIMINE_URL = 'https://github.com/limine-bootloader/limine'

BUILD_DIR = 'build'
BUNDLED_DIR = 'bundled'
SYSROOT_DIR = 'sysroot'
EXTRA_FILES = 'extra-files'
SYSROOT_CARGO_HOME = os.path.join(SYSROOT_DIR, 'cargo-home')
BASE_FILES_DIR = 'base-files'
OVMF_FILES = ['OVMF-pure-efi.fd']

LIMINE_TEMPLATE = """
TIMEOUT=0
VERBOSE=yes

:aero
PROTOCOL=stivale2
KERNEL_PATH=boot:///aero.elf
CMDLINE=term-background=background theme-background=0x50000000

MODULE_PATH=boot:///term_background.bmp
MODULE_STRING=background

MODULE_PATH=boot:///initramfs.cpio
MODULE_STRING=initramfs
"""


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
        os.makedirs(ovmf_path)

    for ovmf_file in OVMF_FILES:
        file_path = os.path.join(ovmf_path, ovmf_file)

        if not os.path.exists(file_path):
            with open(file_path, 'wb') as file:
                response = requests.get(f'{OVMF_URL}/{ovmf_file}')

                file.write(response.content)

    if not os.path.exists(limine_path):
        run_command(['git', 'clone', '--branch', 'latest-binary',
                    '--depth', '1', LIMINE_URL, limine_path])


def extract_artifacts(stdout):
    result = []
    lines = stdout.splitlines()

    for line in lines:
        info = json.loads(line)
        executable = info['executable'] if 'executable' in info else None

        if executable:
            result.append(info['executable'])

    return result


def build_cargo_workspace(cwd, command, args):
    code, _, _ = run_command(['cargo', command, *args], cwd=cwd)

    if code != 0:
        return None

    _, stdout, _ = run_command(['cargo', command, *args, '--message-format=json'],
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


def build_userland_sysroot(args):
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

    os.chdir(SYSROOT_DIR)

    args = {
        "update": True,
        "all": True,
        "dry_run": False,
        "check": False,
        "recursive": False,
        "paranoid": False,
        "reset": False,
        "hard_reset": False,
        "only_wanted": False,
        "keep_going": False,

        "progress_file": None,  # file that receives machine-ready progress notifications
        "reconfigure": False,
        "rebuild": False
    }

    namespace = argparse.Namespace(**args)
    xbstrap.do_install(namespace)


def build_userland(args):
    command = 'build'
    cmd_args = []

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
    shutil.copy(os.path.join(limine_path, 'limine-eltorito-efi.bin'), iso_root)

    initramfs_root = os.path.join(BUILD_DIR, 'initramfs_root')

    initramfs_bin = os.path.join(initramfs_root, 'bin')
    initramfs_lib = os.path.join(initramfs_root, 'usr', 'lib')
    initramfs_include = os.path.join(initramfs_root, 'usr', 'include')

    if os.path.exists(initramfs_root):
        shutil.rmtree(initramfs_root)

    os.makedirs(initramfs_root)
    os.makedirs(initramfs_bin)
    os.makedirs(initramfs_lib)

    def find(path) -> List[str]:
        _, find_output, _ = run_command(['find', '.', '-type', 'f'],
                                        cwd=path,
                                        stdout=subprocess.PIPE)

        files_without_dot = filter(
            lambda x: x != '.', find_output.decode('utf-8').splitlines())
        files_without_prefix = map(
            lambda x: remove_prefix(x, './'), files_without_dot)
        files = list(files_without_prefix)

        return files

    def cp(src, dest):
        files = find(src)

        for line in files:
            file = os.path.join(src, line)
            dest_file = os.path.join(dest, line)

            os.makedirs(os.path.dirname(dest_file), exist_ok=True)
            shutil.copy(file, dest_file)

    bin_src = os.path.join(SYSROOT_DIR, 'system-root/usr/bin')
    lib_src = os.path.join(SYSROOT_DIR, 'system-root/usr/lib')
    inc_src = os.path.join(SYSROOT_DIR, 'system-root/usr/include')

    if os.path.exists(bin_src):
        cp(bin_src, initramfs_bin)

    if os.path.exists(lib_src):
        cp(lib_src, initramfs_lib)

    if os.path.exists(inc_src):
        cp(inc_src, initramfs_include)

    cp(BASE_FILES_DIR, initramfs_root)

    for file in user_bins:
        bin_name = os.path.basename(file)

        shutil.copy(file, os.path.join(initramfs_bin, bin_name))

    files = find(initramfs_root)

    with open(os.path.join(iso_root, 'initramfs.cpio'), 'wb') as initramfs:
        cpio_input = '\n'.join(files)
        code, _, _ = run_command(['cpio', '-o', '-v'],
                                 cwd=initramfs_root,
                                 stdout=initramfs,
                                 stderr=subprocess.PIPE,
                                 input=cpio_input.encode('utf-8'))

    with open(os.path.join(iso_root, 'limine.cfg'), 'w') as limine_cfg:
        limine_cfg.write(LIMINE_TEMPLATE)

    code, _, xorriso_stderr = run_command([
        'xorriso', '-as', 'mkisofs', '-b', 'limine-cd.bin', '-no-emul-boot', '-boot-load-size', '4',
        '-boot-info-table', '--efi-boot', 'limine-eltorito-efi.bin', '-efi-boot-part',
        '--efi-boot-image', '--protective-msdos-label', iso_root, '-o', iso_path
    ], stdout=subprocess.PIPE, stderr=subprocess.PIPE)

    if code != 0:
        print('Failed to create the ISO image')
        print(xorriso_stderr)

        return None

    limine_install = None

    if platform.system() == 'Windows':
        limine_install = 'limine-install-win32.exe'
    elif platform.system() == 'Linux':
        limine_install = 'limine-install-linux-x86_64'
    elif platform.system() == 'Darwin':
        limine_install = 'limine-install'
        # Limine doesn't provide pre-built binaries, so we have to build from source
        code, _, limine_build_stderr = run_command(['make', '-C', limine_path],
                                                   stdout=subprocess.PIPE,
                                                   stderr=subprocess.PIPE)
        if code != 0:
            print('Failed to build `limine-install`')
            print(limine_build_stderr.decode('utf8'))
            exit(1)

    limine_install = os.path.join(limine_path, limine_install)

    code, _, limine_install_stderr = run_command([limine_install, iso_path],
                                                 stdout=subprocess.PIPE,
                                                 stderr=subprocess.PIPE)

    if code != 0:
        print('Failed to install Limine')
        print(limine_install_stderr)

        return None

    return iso_path


def run_in_emulator(args, iso_path):
    is_kvm_available = is_kvm_supported()

    qemu_args = ['-cdrom', iso_path,
                 '-M', 'q35',
                 '-m', '5G',
                 '-smp', '5',
                 '-serial', 'stdio']

    if args.bios == 'uefi':
        qemu_args += ['-bios', 'bundled/ovmf/OVMF-pure-efi.fd']

    cmdline = args.remaining

    if '--' in cmdline:
        cmdline.remove('--')

    if cmdline:
        qemu_args += cmdline

    if is_kvm_available and not args.disable_kvm:
        print("Running with KVM acceleration enabled")

        if platform.system() == 'Darwin':
            qemu_args += ['-accel', 'hvf']
        else:
            qemu_args += ['-enable-kvm']
        qemu_args += ['-cpu', 'host,+la57' if args.la57 else 'host']
    else:
        qemu_args += ["-cpu", "qemu64,+la57" if args.la57 else "qemu64"]

    run_command(['qemu-system-x86_64', *qemu_args])


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
    args = parse_args()

    download_bundled()

    if args.clean:
        src_target = os.path.join('src', 'target', args.target)
        userland_target = os.path.join('userland', 'target')

        if os.path.exists(src_target):
            shutil.rmtree(src_target)

        if os.path.exists(userland_target):
            shutil.rmtree(userland_target)
    elif args.sysroot:
        build_userland_sysroot(args)
    elif args.document:
        build_kernel(args)

        generate_docs(args)
    else:
        user_bins = build_userland(args)

        if not user_bins:
            return

        kernel_bin = build_kernel(args)

        if not kernel_bin or args.check:
            return

        kernel_bin = kernel_bin[0]
        iso_path = prepare_iso(args, kernel_bin, user_bins)

        if not args.no_run:
            run_in_emulator(args, iso_path)


if __name__ == '__main__':
    try:
        main()
    except KeyboardInterrupt:
        pass
