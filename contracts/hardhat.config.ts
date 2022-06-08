// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

import "@nomiclabs/hardhat-ethers";
import "@nomiclabs/hardhat-waffle";
import "@typechain/hardhat";
import "@nomiclabs/hardhat-etherscan";
import "hardhat-deploy";
import "hardhat-gas-reporter";
import "hardhat-contract-sizer";
import {
  TASK_NODE_SERVER_CREATED,
  TASK_COMPILE_SOLIDITY_GET_SOLC_BUILD,
} from "hardhat/builtin-tasks/task-names";
import { HardhatUserConfig, subtask, task } from "hardhat/config";

// This is a sample Hardhat task. To learn how to create your own go to
// https://hardhat.org/guides/create-task.html
task("accounts", "Prints the list of accounts", async (_taskArgs, hre) => {
  const accounts = await hre.ethers.getSigners();

  for (const account of accounts) {
    console.log(account.address);
  }
});

task(TASK_NODE_SERVER_CREATED, async (taskArgs: any) => {
  // Increase from 5 seconds to 5 minutes to prevent tests with longer pauses from failing.
  taskArgs.server._httpServer.keepAliveTimeout = 5 * 60 * 1000;
});

// Use the compiler downloaded with nix if the version matches
// Based on: https://github.com/fvictorio/hardhat-examples/tree/master/custom-solc
subtask(TASK_COMPILE_SOLIDITY_GET_SOLC_BUILD, async (args: any, _hre, runSuper) => {
  if (args.solcVersion === process.env.SOLC_VERSION) {
    const compilerPath = process.env.SOLC_PATH;

    return {
      compilerPath,
      isSolcJs: false, // native solc
      version: args.solcVersion,
      // for extra information in the build-info files, otherwise not important
      longVersion: `${args.solcVersion}-dummy-long-version`,
    };
  }

  console.warn("Warning: Using compiler downloaded by hardhat");
  return runSuper(); // Fall back to running the default subtask
});

const config: HardhatUserConfig = {
  defaultNetwork: "localhost",
  etherscan: {
    apiKey: process.env.ETHERSCAN_API_KEY,
  },
  namedAccounts: {
    deployer: 0,
    tokenOwner: 1,
  },
  networks: {
    hardhat: {
      allowUnlimitedContractSize: true,
      accounts: {
        mnemonic: process.env.TEST_MNEMONIC,
      },
      // Avoid: "InvalidInputError: Transaction gas limit is 31061912 and exceeds block gas limit of 30000000"
      gas: 25_000_000,
    },
    rinkeby: {
      url: process.env.RINKEBY_URL,
      gasPrice: 2_000_000_000,
      accounts: { mnemonic: process.env.RINKEBY_MNEMONIC },
    },
    goerli: {
      url: process.env.GOERLI_URL,
      gasPrice: 2_000_000_000,
      accounts: { mnemonic: process.env.GOERLI_MNEMONIC },
    },
    localhost: {
      url: `http://localhost:${process.env.RPC_PORT || 8545}`,
      timeout: 120000, // when running against hardhat, some tests are very slow
    },
  },
  solidity: {
    version: process.env.SOLC_VERSION ? process.env.SOLC_VERSION : "0.8.0",
    settings: {
      optimizer: {
        enabled: true,
        runs: Number(process.env.SOLC_OPTIMIZER_RUNS),
      },
    },
  },
  gasReporter: {
    enabled: process.env.REPORT_GAS ? true : false,
    showMethodSig: true,
    currency: "USD",
    gasPrice: 205,
    onlyCalledMethods: true,
  },
  mocha: {
    timeout: 300000,
  },
  typechain: {
    target: "ethers-v5",
    alwaysGenerateOverloads: false,
  },
  // Mainnet only allows contracts up to 24kB in size.
  // Fail hardhat compile task if CAPE exceeds the limit but allow the
  // test contracts to exceed the limit.
  contractSizer: {
    runOnCompile: true,
    strict: true,
    only: ["/CAPE"],
  },
};

export default config;
