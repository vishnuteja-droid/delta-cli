#!/bin/sh
# CRITERION: C5 generated per-CLI command files are in sync with delta/commands/
#
# A thin wrapper over a command the repository already provides. That is what
# most checks should look like: the value is the binding, not the cleverness.
exec delta/bin/generate-commands --check
