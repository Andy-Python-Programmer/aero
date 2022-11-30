#!/usr/bin/env bash

#  deps.sh
#  dependency installer script
#
#  note: should be replaced as part of build system rework

PREFIX='deps.sh>'

# setuid binary detection - supports 'sudo' and 'opendoas'
SUID_BINARY='unknown'
if command -v sudo; then
	SUID_BINARY=$(which sudo)
else
	echo "$PREFIX sudo not found, attempting to use opendoas"
	SUID_BINARY=$(which doas)
fi

# platform/os detection - supports macOS and arch-based distros 
PLATFORM='unknown'
PKGMAN='unknown'
PKGPREFIX=''
PKGINSTALL=''
PKGQUERY=''
if [[ "$(uname)" == 'Linux' ]]; then
# TODO: only supports arch-based distros at the moment
PLATFORM='linux'
PKGMAN="pacman"
PKGPREFIX="$SUID_BINARY"
PKGINSTALL="-S --noconfirm"
PKGQUERY="-Q"
elif [[ "$(uname)" == 'Darwin' ]]; then
PLATFORM='darwin'
PKGMAN='brew'
PKGINSTALL='install'
PKGQUERY='list'
else
	echo "$PREFIX unsupported OS"
	# TODO: support more operating systems
fi

PARENTDIR=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
DEPSDIR="$PARENTDIR/deps"
DEPSFILE="$DEPSDIR/deps_$PLATFORM"

echo "$PREFIX \`$PLATFORM\` system detected"
echo "$PREFIX installing packages from \`$DEPSFILE\` with \`$PKGMAN\`..."

while read -r line; do
	# DEPSFILE comments are made with `#`
	pkg=$(echo -n -e $line)
	if [[ "$pkg" != *"#"* ]]; then
		echo -n "installing $pkg... "
		# TODO: handle potential errors in installation commands
		if [[ "$VERBOSE" == "true" ]]; then
			$PKGPREFIX $PKGMAN $PKGINSTALL $pkg
		else
			$PKGPREFIX $PKGMAN $PKGINSTALL $pkg &>/dev/null
		fi

		if grep -q "$pkg" <<< "$($PKGMAN $PKGQUERY)"; then
			echo done
		else
			echo "FAILED"
		fi
	fi
done <$DEPSFILE

echo "$PREFIX completed"
