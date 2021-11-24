{ pkgs, ... }:

let
  nix-pre-commit-hooks = import (pkgs.fetchFromGitHub {
    owner = "cachix";
    repo = "pre-commit-hooks.nix";
    rev = "50cfce93606c020b9e69dce24f039b39c34a4c2d";
    sha256 = "KZoMUmLgJVYnmohhJ/ENeiH8fCN7rY3VyG/4UpDNEWA=";
  });
in
nix-pre-commit-hooks.run {
  src = ./.;
  hooks = {
    lint-solidity = {
      enable = true;
      files = "^contracts/";
      entry = "lint-solidity";
      types = [ "solidity" ];
    };
    check-format = {
      enable = true;
      entry = "treefmt --fail-on-change";
    };
  };
}
