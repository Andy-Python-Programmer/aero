#!/usr/bin/env bash

#  deps.sh
#  dependency installer script
#
#  note: should be replaced as part of build system rework

PREFIX='deps.sh>'
PLATFORM='unknown'
PKGMAN='unknown'
if [[ "$(uname)" == 'Linux' ]]; then
# TODO: only supports arch-based distros at the moment
PLATFORM='linux'
PKGMAN='pacman -S'
elif [[ "$(uname)" == 'Darwin' ]]; then
PLATFORM='darwin'
PKGMAN='brew install'
else
	echo "$PREFIX unsupported OS"
	# TODO: support more operating systems
fi

DEPSDIR='./tools/deps'
DEPSFILE="$DEPSDIR/deps_$PLATFORM"

echo "$PREFIX \`$PLATFORM\` system detected"
echo "$PREFIX installing packages from \`$DEPSFILE\` with \`$PKGMAN\`..."

while read -r line; do
	# DEPSFILE comments are made with `#`
	pkg=$(echo -n -e $line)
	if [[ "$pkg" != *"#"* ]]; then
		echo -n "installing $pkg... "
		# TODO: handle potential errors in installation commands
		$PKGMAN $pkg &>/dev/null
		echo "done" 
	fi
done <$DEPSFILE

echo "$PREFIX completed"
