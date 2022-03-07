#!/usr/bin/env bash
set -euo pipefail

rm -f *.png
for file in *.puml; do
    echo $file
    plantuml $file
done
