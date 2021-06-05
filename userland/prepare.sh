# Copyright 2021 The Aero Project Developers. See the COPYRIGHT
# file at the top-level directory of this project.
#
# Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
# http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
# <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
# option. This file may not be copied, modified, or distributed
# except according to those terms.
#
# ## Overview
# This file is responsible for preparing mlibc, gcc and g++! Since this cannot be ported
# to windows, we will straight away do this in a bash script that the aero bootstrap will
# invoke. This would decrease the amount of code we have to write in the bootstrap crate
# as all this is just bash commands!

function __perpare_mlibc  {
    echo "Preparing mlibc..."; 
    git submodule update --init userland/mlibc;

    meson setup 
        --cross=userland/cross-file.ini 
        -Dheaders_only=false -Dstatic=true 
        userland/mlibc userland/build/mlibc
}

function __prerpare_gcc_g++ {
    echo "Preparing gcc and g++"
    git submodule update --init userland/gcc
}

function __userland_prepare {
    __prerpare_gcc_g++

    # We need to download meson as building mlibc requires it :D
    if ! command -v meson &> /dev/null
    then
        echo "Downloading meson..."; sudo apt install meson
    fi

    __perpare_mlibc
}

__userland_prepare
