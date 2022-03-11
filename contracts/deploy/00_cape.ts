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

  const pubKey =
    "USERPUBKEY~muN7VKxj1GbJ4D6rU6gANdvwD05oPKy_XmhkBxSByq0gAAAAAAAAAIRN-Rik8czFiToI8Ft5fsIf9HAEtWHDsOHh-ZBJZl1KxQ";
  // To update, update in `./rust/src/bin/faucet.rs` and run
  //
  //     cargo run --bin faucet -p cap-rust-sandbox
  //
  // To get the values below.
  // ```
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
    // TODO This is probably wrong. How construct the serialized pub key correctly (?)
    utils.toUtf8Bytes(pubKey)
  );
};

export default func;
func.tags = ["CAPE"];
