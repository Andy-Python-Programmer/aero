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

PKGMAN='unknown'
PKGPREFIX=''
PKGINSTALL=''
PKGQUERY=''

PARENTDIR=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )

PLATFORM=$(uname)
DEPSDIR="$PARENTDIR/deps"
DEPSFILE="$DEPSDIR/deps_${PLATFORM,,}"

. $DEPSFILE

log_info "\`$PLATFORM\` system detected"
log_info "installing packages from \`$DEPSFILE\` with \`$PKGMAN\`..."

if [[ !("$PLATFORM" == "Darwin") && ("$EUID" -ne 0) ]]; then
	log_error "please run as root"
	exit
fi

for pkg in "${packages[@]}"; do
	echo -n "installing $pkg... "

	# TODO: handle potential errors in installation commands
	if [[ "$VERBOSE" == "true" ]]; then
		install_package $pkg
	else
		install_package $pkg &>/dev/null
	fi

	if query_package $pkg; then
		echo -e "${GREEN}ok${NC}"
	else
		echo -e "${RED}FAILED${NC}"
	fi
done

log_info "completed"
