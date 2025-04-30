#!/bin/bash

set -xeuo pipefail

# Colorful output.
function greenprint {
    echo -e "\033[1;32m[$(date -Isecond)] ${1}\033[0m"
}

check_version() {
    # WARNING: exits on error
    from=$1
    to=$2

    if git --no-pager diff "${from}...${to}" | grep '^diff --git' | grep 'runtime/src/lib.rs'; then
        greenprint "PASS: lib.rs was modified!"
    else
        greenprint "FAIL: lib.rs was not modified!"
        exit 1
    fi
}

#### main part

SPEC_VERSION=$(grep spec_version: runtime/src/lib.rs | cut -f2 -d: | tr -d " ,")
IMPL_VERSION=$(grep impl_version: runtime/src/lib.rs | cut -f2 -d: | tr -d " ,")

FROM=$(git rev-parse "${1:-origin/main}")
TO=$(git rev-parse "${2:-HEAD}")

greenprint "DEBUG: Inspecting range $FROM..$TO"

if [ -z "$FROM" ]; then
    echo "ERROR: FROM is empty. Exiting..."
    exit 2
fi

if [ -z "$TO" ]; then
    echo "ERROR: TO is empty. Exiting..."
    exit 2
fi

if git --no-pager diff --name-only "${FROM}"..."${TO}" | grep -e '^runtime'; then
    greenprint "INFO: runtime/src/ has been modified"
    check_version "${FROM}" "${TO}"
fi