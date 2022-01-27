import { HardhatRuntimeEnvironment } from "hardhat/types";
import { DeployFunction } from "hardhat-deploy/types";

const func: DeployFunction = async function (hre: HardhatRuntimeEnvironment) {
  const { deployments, getNamedAccounts } = hre;
  const { deploy } = deployments;
  const { deployer } = await getNamedAccounts();

  let rescueLib = await deploy("RescueLib", {
    from: deployer,
    args: [],
    log: true,
  });
  let edOnBN254 = await deploy("EdOnBN254", {
    from: deployer,
    args: [],
    log: true,
  });
  let accumulatingArray = await deploy("AccumulatingArray", {
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
  const nRoots = 10;

  await deploy("CAPE", {
    from: deployer,
    args: [treeDepth, nRoots, plonkVerifierContract.address],
    log: true,
    libraries: {
      RescueLib: rescueLib.address,
      EdOnBN254: edOnBN254.address,
      AccumulatingArray: accumulatingArray.address,
      VerifyingKeys: verifyingKeys.address,
    },
  });
};

export default func;
func.tags = ["CAPE"];
