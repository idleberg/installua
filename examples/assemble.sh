#!/usr/bin/env bash
# Phase 0, task 3 — assemble all five hand-written `.nsi` under
# `makensis -WX` with an empty warning allowlist.
#
# These files are the oracle. If one of them does not assemble, the expectation
# is wrong, not the compiler.

set -u
cd "$(dirname "$0")"

command -v makensis >/dev/null || {
	echo "SKIP: makensis not installed"
	exit 0
}

# Opaque payload fixtures are generated, not committed. Idempotent.
python3 fixtures.py

echo "makensis $(makensis -VERSION)"

fail=0
for dir in */; do
	[ -f "$dir/expected.nsi" ] || continue
	name="${dir%/}"

	# -WX promotes every warning to an error; the allowlist is empty by
	# construction, since there is no way to spell one.
	out=$( (cd "$dir" && makensis -WX -V2 expected.nsi) 2>&1 )
	rc=$?

	if [ $rc -ne 0 ] || [ -n "$out" ]; then
		printf '\033[31mFAIL\033[0m %s (exit %d)\n' "$name" "$rc"
		printf '%s\n' "$out" | sed 's/^/       /'
		fail=$((fail + 1))
		rm -f "$dir"/*-setup.exe
		continue
	fi

	# Run-to-run determinism in a fixed tree -- this is what catches an `.nsi`
	# that embeds a build timestamp. NOT cross-checkout reproducibility: NSIS
	# packs each file's mtime, so these hashes differ in a fresh clone unless
	# `SetDateSave off` is set. See examples/README.md.
	first=$(shasum -a 256 "$dir"/*-setup.exe | cut -d" " -f1)
	(cd "$dir" && makensis -WX -V0 expected.nsi) >/dev/null 2>&1
	second=$(shasum -a 256 "$dir"/*-setup.exe | cut -d" " -f1)
	rm -f "$dir"/*-setup.exe

	if [ "$first" = "$second" ]; then
		printf '\033[32mPASS\033[0m %-22s %s\n' "$name" "${first:0:16}"
	else
		printf '\033[31mFAIL\033[0m %s assembles but is not reproducible\n' "$name"
		printf '       %s\n       %s\n' "$first" "$second"
		fail=$((fail + 1))
	fi
done

if [ "$fail" -eq 0 ]; then
	printf '\nall five assemble clean\n'
else
	printf '\n%d failed\n' "$fail"
fi
exit "$fail"
