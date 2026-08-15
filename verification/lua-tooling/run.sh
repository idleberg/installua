#!/usr/bin/env bash
# Re-run the §1 tooling verification. Every claim §1 makes about
# lua-language-server, selene and stylua is checked here; RESULTS.md records
# what came back. Phase 0 artifact — this becomes a real test in Phase 1.
#
# Exit status is 0 when every check behaves as RESULTS.md says it does,
# including the ones that record a *failed* claim.

set -u
cd "$(dirname "$0")"

pass=0
fail=0
note() { printf '  %s\n' "$*"; }
ok() { printf '\033[32mPASS\033[0m %s\n' "$1"; pass=$((pass + 1)); }
no() { printf '\033[31mFAIL\033[0m %s\n' "$1"; fail=$((fail + 1)); }

# selene: see RESULTS.md finding S1 — no released binary can parse Lua 5.4.
SELENE="${SELENE:-selene}"

need() {
	command -v "$1" >/dev/null 2>&1 || {
		printf '\033[33mSKIP\033[0m %s not installed\n' "$1"
		return 1
	}
}

################################################################################
printf '\n=== lua-language-server ===\n'
################################################################################
if need lua-language-server; then
	rm -rf .out
	lua-language-server --check . --checklevel=Information \
		--logpath=.out --check_out_path=.out/check.json >/dev/null 2>&1

	python3 - <<'PY'
import json, os, sys

path = ".out/check.json"
got = {}
if os.path.exists(path):
    raw = json.load(open(path))
    if isinstance(raw, dict):
        for uri, items in raw.items():
            name = uri.split("/")[-1]
            for it in items:
                got.setdefault(name, set()).add(
                    (it["range"]["start"]["line"] + 1, it.get("code"))
                )

expected = {
    "bad.lua": {
        (11, "assign-type-mismatch"),   # alias member checked inside a string literal
        (14, "undefined-global"),       # `detailprint` casing
        (15, "missing-parameter"),      # arity
        (17, "undefined-global"),       # require
        (18, "undefined-global"),       # coroutine
        (19, "undefined-global"),       # io
        (20, "undefined-global"),       # os
        (21, "undefined-global"),       # debug
    },
}

fails = 0
if got.get("good.lua"):
    print("FAIL good.lua is not clean: %s" % sorted(got["good.lua"]))
    fails += 1
else:
    print("PASS good.lua is clean")

missing = expected["bad.lua"] - got.get("bad.lua", set())
extra = got.get("bad.lua", set()) - expected["bad.lua"]
if missing or extra:
    print("FAIL bad.lua diagnostics differ")
    if missing:
        print("       missing: %s" % sorted(missing))
    if extra:
        print("       extra:   %s" % sorted(extra))
    fails += 1
else:
    print("PASS bad.lua produces exactly the expected diagnostics")

# Finding L1: the unknown field on line 7 is NOT reported, and that is the
# recorded behaviour. If it ever starts being reported, RESULTS.md is stale.
if any(line == 7 for line, _ in got.get("bad.lua", set())):
    print("FAIL finding L1 is stale: LuaLS now reports the unknown field")
    fails += 1
else:
    print("PASS finding L1 holds: unknown field in a table constructor is silent")

sys.exit(1 if fails else 0)
PY
	if [ $? -eq 0 ]; then ok "diagnostics"; else no "diagnostics"; fi

	if python3 completion_probe.py >/tmp/installua-completion.log 2>&1; then
		ok "completion inside string literals (---@alias)"
	else
		no "completion inside string literals (---@alias)"
		sed 's/^/  /' /tmp/installua-completion.log
	fi
fi

################################################################################
printf '\n=== selene ===\n'
################################################################################
python3 gen_selene_std.py
if need "$SELENE"; then
	if "$SELENE" good.lua 2>&1 | grep -q "^0 errors"; then
		ok "good.lua is clean"
	else
		no "good.lua is clean"
		note "if this says 'lua version lua54 … feature is not enabled', see finding S1"
	fi

	out=$("$SELENE" bad.lua 2>&1)
	for want in detailprint "requires 2 parameters" require string.gsub math.floor "try:"; do
		if printf '%s' "$out" | grep -q -- "$want"; then
			ok "bad.lua flags $want"
		else
			no "bad.lua flags $want"
		fi
	done

	if "$SELENE" escapes.lua 2>&1 | grep -q bad_string_escape; then
		ok "escapes.lua: \`\\P\` caught (§13 backslash hazard, editor half)"
	else
		no "escapes.lua: \`\\P\` caught"
	fi
fi

################################################################################
printf '\n=== stylua ===\n'
################################################################################
if need stylua; then
	# Finding T1: the default config rewrites `attributes { … }` into
	# `attributes({ … })`, so a stylua.toml is not optional.
	if stylua --config-path /dev/null - <good.lua 2>/dev/null | grep -q 'attributes({'; then
		ok "finding T1 holds: default config breaks block syntax"
	else
		no "finding T1 is stale: default config no longer adds call parentheses"
	fi

	if stylua --config-path stylua.toml - <good.lua | diff -q good.lua - >/dev/null; then
		ok "good.lua is stylua-stable under stylua.toml"
	else
		no "good.lua is stylua-stable under stylua.toml"
		stylua --config-path stylua.toml - <good.lua | diff good.lua - | sed 's/^/  /'
	fi
fi

printf '\n%d passed, %d failed\n' "$pass" "$fail"
[ "$fail" -eq 0 ]
