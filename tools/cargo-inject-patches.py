#!/usr/bin/env python3
# Copied over from https://github.com/managarm/bootstrap-managarm/scripts/cargo-inject-patches.py

import argparse
import os
import pathlib
import subprocess

patched_libs = {
    "backtrace": "0.3.64",
    "calloop": "0.9.3",
    "libc": "0.2.149",
    "getrandom": "0.2.9",
    "libloading": "0.7.3",
    "mio": ["0.6.23", "0.8.3"],
    "nix": "0.24.3",
    "num_cpus": "1.15.0",
    "users": "0.11.0",
    "winit": "0.26.1",
    "glutin": "0.28.0",
    "glutin_glx_sys": "0.1.7",
    "glutin_egl_sys": "0.1.5",
    "shared_library": "0.1.9",
    "rustix": "0.38.13"
}

parser = argparse.ArgumentParser(description="Inject patched Rust libraries into Cargo lockfiles")
parser.add_argument("manifest", type=pathlib.Path, help="path to Cargo.toml")
manifest = parser.parse_args().manifest

# First, delete the existing lockfile to work around https://github.com/rust-lang/cargo/issues/9470
# lockfile = os.path.join(os.path.dirname(manifest), "Cargo.lock")
# if os.path.exists(lockfile):
#     print("cargo-inject-patches: workaround cargo bug by removing existing lockfile...")
#     os.remove(lockfile)

for lib, versions in patched_libs.items():
    if not isinstance(versions, list):
        versions = [versions]

    for version in versions:
        cmd = [
            "cargo",
            "update",
            "--manifest-path",
            manifest,
            "--package",
            lib,
            "--precise",
            version,
        ]

        output = subprocess.run(cmd)
        if "did not match any packages" in str(output.stderr):
            print(f"cargo-inject-patches: Injecting {lib} v{version} failed, patch not used")
        else:
            print(f"cargo-inject-patches: Injected {lib} v{version}")
