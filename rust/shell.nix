with import ../nix/nixpkgs.nix { };
let
  mySolc = callPackage ../nix/solc-bin { version = "0.8.4"; };
in
mkShell {

  buildInputs = [

    rustfmt
    clippy

    pkgconfig
    openssl

    rustc
    lld # faster linking
    cargo
    cargo-edit
    cargo-watch

    jq

    mySolc

    entr
  ];

  RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
  RUST_BACKTRACE = 1;
  RUSTFLAGS="-C link-arg=-fuse-ld=lld";

  shellHook = ''
    export PATH=$(pwd)/bin:$PATH
    export RUST_LOG=info

    # Needed with the ldd linker
    export LD_LIBRARY_PATH=$LD_LIBRARY_PATH:${openssl.out}/lib
  '';
}
