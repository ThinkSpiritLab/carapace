set -x
cargo build --release
sudo cp ./target/release/carapace /usr/local/bin/
