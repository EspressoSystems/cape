with import ../nix/nixpkgs.nix { };

mkShell {

  buildInputs = [

    rustfmt
    clippy

    pkgconfig
    openssl

    rustc
    cargo
    cargo-edit
    cargo-watch

    jq

    entr
  ] ++ lib.optionals stdenv.isDarwin [
    # required to compile ethers-rs
    darwin.apple_sdk.frameworks.Security
    darwin.apple_sdk.frameworks.CoreFoundation

    # https://github.com/NixOS/nixpkgs/issues/126182
    libiconv
  ] ++ lib.optionals stdenv.isLinux [
    lld # a faster linker, does not work out of the box on OSX
  ];

  RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
  RUST_BACKTRACE = 1;
  RUSTFLAGS = if stdenv.isLinux then "-C link-arg=-fuse-ld=lld" else "";

  shellHook = ''
    export PATH=$(pwd)/bin:$PATH
    export RUST_LOG=info

    # Needed with the ldd linker
    export LD_LIBRARY_PATH=$LD_LIBRARY_PATH:${openssl.out}/lib
  '';
}
