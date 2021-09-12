with import ./nix/nixpkgs.nix { };

let
  geth = callPackage ./nix/go-ethereum {
    inherit (darwin) libobjc;
    inherit (darwin.apple_sdk.frameworks) IOKit;
  };
  mySolc = callPackage ./nix/solc-bin { };
in
mkShell
{
  buildInputs = [
    geth
    nodePackages.pnpm
    mySolc
    hivemind # process runner
    nodejs-12_x # nodejs
  ];
  # export SOLCX_BINARY_PATH=${solcWithVersion}/bin
  shellHook = ''
    export SOLC_VERSION=${mySolc.version}
    export SOLC_PATH=${mySolc}/bin/solc
    export PATH=$(pwd)/bin:$(pwd)/node_modules/.bin:$PATH
  '';
}
