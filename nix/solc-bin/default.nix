{ stdenv, lib, fetchurl, autoPatchelfHook }:

stdenv.mkDerivation rec {
  version = "0.7.6";
  pname = "solc-bin-${version}";
  system = "x86_64-linux";

  # See https://solc-bin.ethereum.org/linux-amd64/list.json
  src = fetchurl {
    url = "https://solc-bin.ethereum.org/linux-amd64/solc-linux-amd64-v${version}+commit.7338295f";
    sha256 = "1fx6b14jvk7c1097j7fhznfxny6xa7cnlhnbfkdg9wkv8a2ylsdx";
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
