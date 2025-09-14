if (Test-Path "./target/doc") {
    Remove-Item "./target/doc" -Recurse
}
cargo doc --no-deps --open