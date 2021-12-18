import { HardhatRuntimeEnvironment } from "hardhat/types";
import { DeployFunction } from "hardhat-deploy/types";

const func: DeployFunction = async function (hre: HardhatRuntimeEnvironment) {
  const { deployments, getNamedAccounts } = hre;
  const { deploy } = deployments;
  const { deployer } = await getNamedAccounts();
  const treeDepth = 26;
  const nRoots = 10;
  await deploy("CAPE", {
    from: deployer,
    args: [treeDepth, nRoots],
    log: true,
  });
};
export default func;
func.tags = ["CAPE"];
