with import ./nix/nixpkgs.nix {};

#let
#  geth = go-ethereum.overrideAttrs (old: rec {
#     src = fetchgit {
#        url = "https://gitlab.com/translucence/go-ethereum";
#        rev = "ddf77c130afd14a5cabe63c5ed7ed1c56cb5aeb8";
#        sha256 = "05riajfg5wcc40g95r6bdwfya4a0lwlricv6gp6b9wisn6wkphmw";
#  };
#
#  buildPhase = ''
#            '';
#});

mkShell {
  buildInputs = [
    go            # go language
    solc          # solidity compiler
    hivemind      # process runner
    nodejs-12_x   # nodejs
  ];

  shellHook = ''
    export PATH=$(pwd)/bin:$PATH
    export HARDHAT_NETWORK=localhost
  '';
}
