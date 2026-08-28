#!/bin/sh
# CRITERION: C1 ~/.delta/ does not exist after install, and nothing recreates it
set -eu

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
fakehome="$tmp/home"; mkdir -p "$fakehome"

HOME="$fakehome" delta/bin/install >/dev/null
test ! -e "$fakehome/.delta" || { echo "install created ~/.delta"; exit 1; }

# Running verify afterwards - the thing that used to write the version note -
# must not create it either.
delta/bin/verify example-verify-exit-codes >/dev/null 2>&1 || true
test ! -e "$fakehome/.delta" || { echo "running verify created ~/.delta"; exit 1; }
