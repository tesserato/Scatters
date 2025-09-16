$outputFolder = "./outputs"

Remove-Item -r $outputFolder
cargo check
write-host ""
cargo run -- --help
write-host ""
cargo run -- "./sample" --debug --output-dir $outputFolder -l 9000 #--no-autoscale-y
