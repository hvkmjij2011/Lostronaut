#!/bin/bash
# Runs the debug build with test env vars, captures the real window (incl. egui) and quits.
# usage: tools/shot.sh <out.png> <seconds> [ENV=VAL ...]
# The window never takes keyboard focus (LOSTRANOUT_AUTOSTART / _CONTINUE imply that).
# Set LOSTRANOUT_TITLE to something unique when several instances may run at once.
out=$1; delay=$2; shift 2
title="LR-$$"
home=$(mktemp -d)
for kv in "$@"; do case "$kv" in LOSTRANOUT_HOME=*) home="${kv#LOSTRANOUT_HOME=}";; esac; done
bin="${LOSTRANOUT_BIN:-${CARGO_TARGET_DIR:-/home/hvkmjij/Masaüstü/Lostronaut/target}/debug/lostranout}"
env LOSTRANOUT_HOME="$home" LOSTRANOUT_TITLE="$title" "$@" "$bin" > "${out%.png}.log" 2>&1 &
pid=$!
sleep "$delay"
import -window "$title" "$out" 2>&1 | tail -1
kill $pid 2>/dev/null; wait $pid 2>/dev/null
grep -E 'panic|ERROR|\[bot\]' "${out%.png}.log" | tail -${LOG_LINES:-6}
