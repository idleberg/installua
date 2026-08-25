#!/usr/bin/env bash
# PLAN Phase 0, task 1b — reproduce the large-string plugin probe.
#
# Needs wine and network access: the vanilla and strlen_8192 Windows builds of
# NSIS 3.12 are downloaded, because the hazard is *by construction* about two
# `makensis` builds disagreeing and macOS Homebrew only ships one.
#
# Everything lands in a scratch directory; nothing is installed.

set -eu
cd "$(dirname "$0")"
HERE="$(pwd)"
WORK="${WORK:-${TMPDIR:-/tmp}/installua-largestr}"
BASE="https://downloads.sourceforge.net/project/nsis/NSIS%203/3.12"

command -v wine >/dev/null || {
	echo "SKIP: wine not installed"
	exit 0
}

mkdir -p "$WORK"
cd "$WORK"

[ -f std.zip ] || curl -sSL -o std.zip "$BASE/nsis-3.12.zip"
[ -f ls.zip ] || curl -sSL -o ls.zip "$BASE/nsis-3.12-strlen_8192.zip"
[ -d std ] || unzip -q std.zip -d std
[ -d ls ] || unzip -q ls.zip -d ls

# `big` is the standard distribution — Plugins and all, compiled against 1024 —
# with only makensis.exe and the Stubs swapped for the 8192 ones. That is
# exactly the configuration under test.
if [ ! -d big ]; then
	cp -r std/nsis-3.12 big
	cp ls/Bin/makensis.exe big/
	cp -r ls/Stubs/. big/Stubs/
fi

winpath() { printf 'Z:%s' "$(printf '%s' "$1" | tr '/' '\\')"; }
export WINEDEBUG=-all

build() { # build <nsisdir> <makensis> <outdir> <script>
	mkdir -p "$3"
	PROBE_OUT="$(winpath "$WORK/$3")\\probe.exe" NSISDIR="$(winpath "$WORK/$1")" \
		wine "$2" -V2 -WX "$(winpath "$4")" 2>&1 |
		grep -viE '^[[:space:]]|mvk|vulkan' || true
}

run() { # run <outdir>
	rm -f "$1/probe-result.txt"
	(cd "$1" && timeout 300 wine probe.exe >/dev/null 2>&1 || true)
	printf '\n===== %s =====\n' "$1"
	cat "$1/probe-result.txt"
}

sed 's/\${While} \$0 < 50/${While} $0 < 205/' "$HERE/probe.nsi" >probe7000.nsi

build std/nsis-3.12 std/nsis-3.12/makensis.exe out1024 "$HERE/probe.nsi"
build big big/makensis.exe out8192 "$HERE/probe.nsi"
build big big/makensis.exe out7000 "$WORK/probe7000.nsi"

run out1024
run out8192
run out7000

printf '\nCompare against %s/result-*.txt\n' "$HERE"
