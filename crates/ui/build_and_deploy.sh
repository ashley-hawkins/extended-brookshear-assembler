#!/bin/sh
set -e
CARGO_UNSTABLE_BUILD_STD=std,panic_abort \
CARGO_UNSTABLE_BUILD_STD_FEATURES=optimize_for_size \
trunk build --release --public-url './'
./deploy.sh
