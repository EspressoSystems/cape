// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

import { HardhatRuntimeEnvironment } from "hardhat/types";
import { DeployFunction } from "hardhat-deploy/types";

const func: DeployFunction = async function (hre: HardhatRuntimeEnvironment) {
  const { deployments, getNamedAccounts } = hre;
  const { deploy, log, read, execute } = deployments;
  const { deployer } = await getNamedAccounts();

  const deployToken = async (name: string) => {
    await deploy(name, { from: deployer, log: true });
    let decimals = await read(name, "decimals");
    log(`Deployed with ${decimals} decimals`);
  };

  await deployToken("WETH");
  await deployToken("DAI");
  await deployToken("USDC");
};

export default func;
func.id = "deploy_token"; // id required to prevent re-execution
func.tags = ["Token"];
func.skip = async (hre: HardhatRuntimeEnvironment) => hre.network.tags.public;
