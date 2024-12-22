{ pkgs ? import (fetchTarball "https://github.com/NixOS/nixpkgs/archive/a9b86fc2290b69375c5542b622088eb6eca2a7c3.tar.gz") {}}:
pkgs.mkShell {
  nativeBuildInputs = with pkgs; [ rustc cargo gcc rustfmt clippy llvmPackages_17.libllvm ];
  buildInputs = with pkgs; [ 
    php
    lua
    ruby
    nodejs-slim
    (pkgs.groovy.override { jdk = pkgs.jdk17; })
    jdk17
  ];
  RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
  shellHook = ''
    export LLVM_COV=$(which llvm-cov)
    export LLVM_PROFDATA=$(which llvm-profdata)
    if [ -z $(cargo --list | grep llvm-cov) ]; then
      cargo install cargo-llvm-cov
    fi
  '';
}