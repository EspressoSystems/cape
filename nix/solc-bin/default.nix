{ stdenv, lib, fetchurl, autoPatchelfHook }:

stdenv.mkDerivation rec {
  version = "0.8.4";
  pname = "solc-bin";
  system = "x86_64-linux";

  # See https://solc-bin.ethereum.org/linux-amd64/list.json
  src = fetchurl {
    url = "https://solc-bin.ethereum.org/linux-amd64/solc-linux-amd64-v${version}+commit.c7e474f2";
    sha256 = "1y571l0ngzdwf14afrdg20niyhhlhsgr9258mbrxr68qy755q4gp";
  };

  nativeBuildInputs = [
    autoPatchelfHook
  ];

  dontUnpack = true;

  installPhase = ''
    install -Dm755 $src $out/bin/solc
  '';

  meta = with lib; {
    description = "Solidity compiler prebuild binary";
    homepage = https://github.com/ethereum/solidity;
    license = licenses.gpl3;
    maintainers = with stdenv.lib.maintainers; [ ];
    platforms = [ "x86_64-linux" ];
  };
}
