#!/usr/bin/env bash
set -euo pipefail

THIS_DIR=`pwd`
PUML_FILES=`ls *.puml global_seq_diag/*.puml`
rm -f *.svg
for file in $PUML_FILES; do
    echo $file
    plantuml -tsvg $file -o "$THIS_DIR/build"
done
