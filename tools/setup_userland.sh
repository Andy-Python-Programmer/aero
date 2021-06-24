#!/bin/bash

SPATH=$(dirname $(readlink -f "$0"))

AERO_PATH=$(realpath $SPATH/..)
AERO_SYSROOT=$AERO_PATH/sysroot/aero
AERO_SYSROOT_BUILD=$AERO_PATH/sysroot/build
AERO_BUNDLED=$AERO_PATH/bundled

AERO_CROSS=$AERO_PATH/sysroot/cross
AERO_TRIPLE=x86_64-aero

set -x -e

# This function is responsible for building and assembling the mlibc headers.
function setup_mlibc {
    if [ ! -d $AERO_SYSROOT/usr/include ]; then # Avoid wasting time to re-install the headers in the prefix location.
        meson setup --cross-file $SPATH/cross-file.ini --prefix $AERO_SYSROOT/usr -Dheaders_only=true -Dstatic=true $AERO_SYSROOT_BUILD/mlibc $AERO_BUNDLED/mlibc
        meson install -C $AERO_SYSROOT_BUILD/mlibc
    fi
}

# This function is responsible for building and assembling libgcc.
function setup_gcc {
    mkdir -p $AERO_SYSROOT_BUILD/gcc

    # The first step of compiling GCC for the Aero target is to download and extract the
    # prerequisite dependencies that GCC requires. We use the helper script `download_prerequisites`
    # to download them. The script requires to run in the root directory of GCC itself so we push the src
    # directory then run the script and pop the directory.
    pushd . 
    cd $AERO_BUNDLED/gcc 
    ./contrib/download_prerequisites
    popd

    # After we are done downloading all of the prerequisite dependencies, we can build GCC. We use the helper
    # configure command from GCC itself, which makes the build process of building GCC much simpler. The configure
    # script requires the current directory to be the build directory so, we push into the GCC build directory here.
    pushd .
    cd $AERO_SYSROOT_BUILD/gcc

    # Run the configure script and only enable C and C++ languages and set enable-threads to posix. See the documentation of the
    # configure script in bundled/gcc/configure for more information.
    $AERO_BUNDLED/gcc/configure --target=$AERO_TRIPLE --prefix="$AERO_CROSS" --with-sysroot=$AERO_SYSROOT --enable-languages=c,c++ --enable-threads=posix
    popd

    # Do the actual compilation of GCC by executing MAKE and compiling libgcc. If you run out of memory, try setting the
    # job number `-j` to an amount lower then 4.
    make -C $AERO_SYSROOT_BUILD/gcc -j4 all-gcc all-target-libgcc
    make -C $AERO_SYSROOT_BUILD/gcc install-gcc install-target-libgcc
}

setup_mlibc
