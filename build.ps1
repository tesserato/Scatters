$outputFolder = "./outputs"

Remove-Item -r $outputFolder
cargo check
write-host ""
cargo run -- --help
write-host ""
cargo run -- "./sample" --debug --output-dir $outputFolder -l 99999999999999 #--no-autoscale-y
