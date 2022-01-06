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

  const treeDepth = 26;
  const nRoots = 10;

  await deploy("CAPE", {
    from: deployer,
    args: [treeDepth, nRoots],
    log: true,
    libraries: {
      RescueLib: rescueLib.address,
    },
  });
};
export default func;
func.tags = ["CAPE"];
