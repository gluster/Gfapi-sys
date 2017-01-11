#!/bin/bash

curl https://sh.rustup.rs -y -sSf | sh
cargo build
sudo target/debug/main
