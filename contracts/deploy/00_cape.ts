// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

import { HardhatRuntimeEnvironment } from "hardhat/types";
import { DeployFunction } from "hardhat-deploy/types";
import { BigNumber } from "ethers";

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

  // To change, update change FAUCET_MANAGER_ENCRYPTION_KEY in rust/src/cape/faucet.rs
  //
  // cargo run --bin faucet-gen-typescript
  //
  // and copy/paste the output.

  // Derived from USERPUBKEY~muN7VKxj1GbJ4D6rU6gANdvwD05oPKy_XmhkBxSByq0gAAAAAAAAAIRN-Rik8czFiToI8Ft5fsIf9HAEtWHDsOHh-ZBJZl1KxQ
  let faucetManagerEncKey = "0x844df918a4f1ccc5893a08f05b797ec21ff47004b561c3b0e1e1f99049665d4a";
  let faucetManagerAddress = {
    x: BigNumber.from("0x2dca81140764685ebfac3c684e0ff0db3500a853ab3ee0c966d463ac547be39a"),
    y: BigNumber.from("0x228cf79945e37cfbb3f43f150b977639a12c900c949e23ed1dcd250578314393"),
  };

  // Override values with environment variable if set.
  const env_enc_key = process.env["CAPE_FAUCET_MANAGER_ENC_KEY"];
  if (env_enc_key) {
    console.log(`Using CAPE_FAUCET_MANAGER_ENC_KEY=${env_enc_key}`);
    faucetManagerEncKey = env_enc_key;
  }

  const env_address_x = process.env["CAPE_FAUCET_MANAGER_ADDRESS_X"];
  const env_address_y = process.env["CAPE_FAUCET_MANAGER_ADDRESS_Y"];
  if (env_address_x && env_address_y) {
    console.log(`Using CAPE_FAUCET_MANAGER_ADDRESS_X=${env_address_x}`);
    console.log(`Using CAPE_FAUCET_MANAGER_ADDRESS_Y=${env_address_y}`);
    faucetManagerAddress = {
      x: BigNumber.from(env_address_x),
      y: BigNumber.from(env_address_y),
    };
  }

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
      log: true,
      from: deployer,
    },
    "faucetSetupForTestnet",
    faucetManagerAddress,
    faucetManagerEncKey
  );
};

export default func;
func.tags = ["CAPE"];
