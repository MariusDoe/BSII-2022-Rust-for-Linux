#!/bin/sh

set -x
REPO_DIR=$(dirname $0)
MOD_DIR=$(realpath $REPO_DIR/_mod_inst)
BUILD_DIR=$(realpath $REPO_DIR/bs2build/)

RUSTUP_TOOLCHAIN="nightly-2021-05-29" make O="$BUILD_DIR" CC=clang LLVM=1 ARCH=x86_64 INSTALL_MOD_PATH="$MOD_DIR" $@
