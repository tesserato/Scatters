# rustup component add rustfmt, clippy
cargo fmt
cargo clippy --fix --allow-dirty --allow-staged -- -D warnings
