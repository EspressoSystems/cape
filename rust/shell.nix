with import <nixpkgs> { };
mkShell {

  nativeBuildInputs = with pkgs; [ rustc cargo gcc ];
  buildInputs = with pkgs; [
    rustfmt clippy

    # Add some extra dependencies from `pkgs`
    openssl
    pkgconfig openssl binutils-unwrapped
    cargo-udeps

    solc
  ];

  RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
  RUST_BACKTRACE = 1;
}
