{ stdenv, lib, fetchurl, autoPatchelfHook, version }:

stdenv.mkDerivation {
  version = version;
  pname = "solc-bin";

  src =
    let
      platform_id = if stdenv.isLinux then "linux-amd64" else "macosx-amd64";
      build_list = lib.importJSON (./. + "/list-${platform_id}.json");
      build = lib.findSingle (x: x.version == version)
        (throw "version not found")
        (throw "found multiple matching versions")
        build_list.builds;
    in
    fetchurl {
      url = "https://binaries.soliditylang.org/${platform_id}/${build.path}";
      sha256 = lib.removePrefix "0x" build.sha256;
    };

  nativeBuildInputs = lib.optionals stdenv.isLinux [ autoPatchelfHook ];

  dontUnpack = true;

  installPhase = ''
    install -Dm755 $src $out/bin/solc

    # Also expose solc-vA.B.C for tools that rely on the version scheme
    ln -s $out/bin/{solc,solc-v${version}}
  '';

  meta = with lib; {
    description = "Solidity compiler prebuild binary";
    homepage = https://github.com/ethereum/solidity;
    license = licenses.gpl3;
    maintainers = with stdenv.lib.maintainers; [ ];
    platforms = [ "x86_64-linux" "x86_64-darwin" "aarch64-darwin" ];
  };
}
