#!/usr/bin/env bash

#  deps.sh
#  dependency installer script
#
#  note: should be replaced as part of build system rework

set -e

RED='\033[1;31m'
GREEN='\033[1;32m'
NC='\033[0m' # no colour

function log_info() { echo -e "$PREFIX ${GREEN}info${NC}: $1"; }
function log_error() { echo -e "$PREFIX ${RED}error${NC}: $1"; }

PREFIX='deps.sh>'

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

log_info "\`$PLATFORM\` system detected"
log_info "installing packages from \`$DEPSFILE\` with \`$PKGMAN\`..."

if [ "$EUID" -ne 0 ]; then
	log_error "please run as root"
	exit
fi

while read -r line; do
	# DEPSFILE comments are made with `#`
	pkg=$(echo -n -e $line)
	if [[ "$pkg" != *"#"* && -n "$pkg" ]]; then
		echo -n "installing $pkg... "
		# TODO: handle potential errors in installation commands
		if [[ "$VERBOSE" == "true" ]]; then
			$PKGPREFIX $PKGMAN $PKGINSTALL $pkg
		else
			$PKGPREFIX $PKGMAN $PKGINSTALL $pkg &>/dev/null
		fi

		if grep -q "$pkg" <<< "$($PKGMAN $PKGQUERY)"; then
			echo -e "${GREEN}ok${NC}"
		else
			echo -e "${RED}FAILED${NC}"
		fi
	fi
done <$DEPSFILE

log_info "$PREFIX completed"
