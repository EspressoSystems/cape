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
    lint-solidity = {
      enable = true;
      files = "^contracts/contracts/";
      entry = "lint-solidity";
      types = [ "solidity" ];
    };
    check-format = {
      enable = true;
      entry = "treefmt --fail-on-change";
    };
    # The hook "clippy" that ships with nix-precommit-hooks is outdated.
    cargo-clippy = {
      enable = true;
      description = "Lint Rust code.";
      entry = "cargo-clippy --workspace -- -D warnings";
      files = "\\.rs$";
      pass_filenames = false;
    };
  };
}
