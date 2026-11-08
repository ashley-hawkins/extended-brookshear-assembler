# build-std with optimize_for_size shrinks the wasm by ~100 KB (needs nightly + rust-src)
CARGO_UNSTABLE_BUILD_STD=std,panic_abort \
CARGO_UNSTABLE_BUILD_STD_FEATURES=optimize_for_size \
trunk build --release --public-url './'
bunx wrangler pages deploy dist --project-name brookshear
