#!/usr/bin/env bash

set -euo pipefail

# Entry point for the Safenet Validator Docker image.
#
# Arguments:
# - STORAGE_FILE: The file name of the SQLite database used for storing
#                 validator state. This is the same environment variable that
#                 gets passed to the validator executable.
# - STORAGE_BACKUP: A printf-style format string specifying the file name for
#                   backing up any existing storage file. The `%s` in the
#                   format string will be replaced with a timestamp.

if [[ -f "${STORAGE_FILE:-}" && -n "${STORAGE_BACKUP:-}" ]]; then
	timestamp="$(date +%s)"
	backup_file="$(printf "$STORAGE_BACKUP" "$timestamp")"
	cp "$STORAGE_FILE" "$backup_file"
fi

node validator/dist/validator.js
