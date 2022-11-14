#!/bin/bash

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

set -x -e

SPATH=$(dirname $(readlink -f "$0"))
AERO_PATH=$(realpath $SPATH/..)

# remove the build directory to ensure the build image
# is rebuilt.
rm -rf $AERO_PATH/build

if [ -z "$1" ]; then
    echo "Usage: $0 <package dir> [<package name>] [--tool]"
    exit 1
fi

if [ -z "$2" ]; then
    PKG_NAME="$1"
else
    if [ "$2" = "--tool" ]; then
        IS_TOOL="-tool"
        PKG_NAME="$1"
    else
        PKG_NAME="$2"
    fi
fi

if [ "$3" = "--tool" ]; then
    IS_TOOL="-tool"
fi

[ -z "$IS_TOOL" ] && rm -rf "$AERO_PATH"/sysroot/pkg-builds/$1
[ -z "$IS_TOOL" ] || rm -rf "$AERO_PATH"/sysroot/tool-builds/$PKG_NAME

if [ -d bundled/$1 ]; then
    rm -rf bundled/$1
fi

if [ -f bundled/"$1".tar.gz ]; then
    rm -rf bundled/"$1".tar.gz
fi

pushd sysroot
xbstrap install -u --all
popd
