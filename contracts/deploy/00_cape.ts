import { HardhatRuntimeEnvironment } from "hardhat/types";
import { DeployFunction } from "hardhat-deploy/types";
import { utils, BigNumber } from "ethers";

const func: DeployFunction = async function (hre: HardhatRuntimeEnvironment) {
  const { deployments, getNamedAccounts } = hre;
  const { deploy, execute } = deployments;
  const { deployer } = await getNamedAccounts();

  let rescueLib = await deploy("RescueLib", {
    from: deployer,
    args: [],
    log: true,
  });
  let verifyingKeys = await deploy("VerifyingKeys", {
    from: deployer,
    args: [],
    log: true,
  });

  let plonkVerifierContract = await deploy("PlonkVerifier", {
    from: deployer,
    args: [],
    log: true,
  });

  const treeDepth = 24;
  const nRoots = 1000;

  // To update, update `pub_key` in `./rust/src/bin/faucet.rs` and run
  //
  //     cargo run --bin faucet -p cap-rust-sandbox
  //
  // and copy/paste the output.
  const faucetManagerEncKey = "0x844df918a4f1ccc5893a08f05b797ec21ff47004b561c3b0e1e1f99049665d4a";
  const faucetManagerAddress = {
    x: BigNumber.from("0x2DCA81140764685EBFAC3C684E0FF0DB3500A853AB3EE0C966D463AC547BE39A"),
    y: BigNumber.from("0x228CF79945E37CFBB3F43F150B977639A12C900C949E23ED1DCD250578314393"),
  };

  await deploy("CAPE", {
    from: deployer,
    args: [treeDepth, nRoots, plonkVerifierContract.address],
    log: true,
    libraries: {
      RescueLib: rescueLib.address,
      VerifyingKeys: verifyingKeys.address,
    },
  });
  await execute(
    "CAPE",
    {
      from: deployer,
    },
    "faucetSetupForTestnet",
    faucetManagerAddress,
    faucetManagerEncKey
  );
};

export default func;
func.tags = ["CAPE"];
