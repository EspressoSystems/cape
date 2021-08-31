with import ./nix/nixpkgs.nix {};

mkShell {
  buildInputs = [
    solc          # solidity compiler
    go-ethereum   # blockchain node
    hivemind      # process runner
  ];

  shellHook = ''
    export PATH=$(pwd)/bin:$PATH
  '';
}
