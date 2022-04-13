# Copyright (c) 2022 Espresso Systems (espressosys.com)
# This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
#
# This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
# This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
# You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

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
