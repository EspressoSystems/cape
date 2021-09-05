{ lib, stdenv, buildGoModule, fetchFromGitHub, libobjc, IOKit }:

let
  # A list of binaries to put into separate outputs
  bins = [
    "geth"
    "clef"
  ];

in buildGoModule rec {
  pname = "go-ethereum";
  version = "1.10.9-dev";

  # src = fetchFromGitHub {
  #   owner = "ethereum";
  #   repo = pname;
  #   rev = "v${version}";
  #   sha256 = "sha256-r4ifLa4CMZvp0MaCkxWo5rWLEnFdX//mYlC08hndXhQ=";
  # };
  src = ./../../go-ethereum;

  runVend = true;

  # vendorSha256 = "sha256-e8aKQMVEEf0BzpdljkOBxznj5P1Go/6EbY9mdhDLarw=";
  vendorSha256 = "sha256-iNdFvLKE6Vn42NLp90xt2GrV/Kk0Ephm486vzzWdu6I=";

  doCheck = false;

  outputs = [ "out" ] ++ bins;

  # Move binaries to separate outputs and symlink them back to $out
  postInstall = lib.concatStringsSep "\n" (
    builtins.map (bin: "mkdir -p \$${bin}/bin && mv $out/bin/${bin} \$${bin}/bin/ && ln -s \$${bin}/bin/${bin} $out/bin/") bins
  );

  subPackages = [
    "cmd/abidump"
    "cmd/abigen"
    "cmd/bootnode"
    "cmd/checkpoint-admin"
    "cmd/clef"
    "cmd/devp2p"
    "cmd/ethkey"
    "cmd/evm"
    "cmd/faucet"
    "cmd/geth"
    "cmd/p2psim"
    "cmd/puppeth"
    "cmd/rlpdump"
    "cmd/utils"
  ];

  # Fix for usb-related segmentation faults on darwin
  propagatedBuildInputs =
    lib.optionals stdenv.isDarwin [ libobjc IOKit ];

  meta = with lib; {
    homepage = "https://geth.ethereum.org/";
    description = "Official golang implementation of the Ethereum protocol";
    license = with licenses; [ lgpl3Plus gpl3Plus ];
    maintainers = with maintainers; [ adisbladis lionello RaghavSood ];
  };
}
