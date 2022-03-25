import { HardhatRuntimeEnvironment } from "hardhat/types";
import { DeployFunction } from "hardhat-deploy/types";

const func: DeployFunction = async function (hre: HardhatRuntimeEnvironment) {
  const { deployments, getNamedAccounts } = hre;
  const { deploy } = deployments;
  const { deployer } = await getNamedAccounts();

  await deploy("MaliciousToken", { from: deployer, log: true });
};

export default func;
func.id = "deploy_malicious_token"; // id required to prevent reexecution
func.tags = ["CAPE"];
