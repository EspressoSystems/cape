{ pkgs, ... }:

let
  nix-pre-commit-hooks = import (pkgs.fetchFromGitHub {
    owner = "cachix";
    repo = "pre-commit-hooks.nix";
    rev = "3ed0e618cebc1ff291c27b749cf7568959cac028";
    sha256 = "0zni3zpz544p7bs7a87wjhd6wb7jmicx0sf2s5nrqapnxa97zcs4";
  });
in
nix-pre-commit-hooks.run {
  src = ./.;
  hooks = {
    treefmt = {
      enable = true;
      entry = "treefmt";
    };
    lint-solidity = {
      enable = true;
      entry = "lint-solidity";
    };
  };
}
