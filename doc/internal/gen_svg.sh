#!/usr/bin/env bash
set -euo pipefail

rm -f *.svg
for file in *.puml; do
    echo $file
    plantuml -tsvg $file
done
