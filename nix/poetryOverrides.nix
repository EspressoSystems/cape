{ pkgs }:

self: super:

{
  vyper = super.vyper.overridePythonAttrs (old: {
    nativeBuildInputs = old.nativeBuildInputs ++ [ pkgs.git ];
    dontPreferSetupPy = true;
  });
  vvm = super.vvm.overridePythonAttrs (old: {
    nativeBuildInputs = old.nativeBuildInputs ++ [ pkgs.git ];
    dontPreferSetupPy = true;
  });
  black = super.black.overridePythonAttrs (old: {
    postPatch = ''
      substituteInPlace setup.py --replace 'platformdirs>=2' 'platformdirs'
    '';
  });
  mythx-models = super.mythx-models.overridePythonAttrs (old: {
    buildInputs = old.buildInputs ++ [ self.pytest-runner ];
  });
  pythx = super.pythx.overridePythonAttrs (old: {
    buildInputs = old.buildInputs ++ [ self.pytest-runner ];
  });
  py-solc-x = super.py-solc-x.overridePythonAttrs (old: {
    preConfigure = ''
      substituteInPlace setup.py --replace \"setuptools-markdown\" ""
    '';
  });
  eth-brownie = super.eth-brownie.overridePythonAttrs (old: {
    postPatch = ''
      substituteInPlace requirements.txt --replace platformdirs==2.3.0 platformdirs
    '';
  });
}
