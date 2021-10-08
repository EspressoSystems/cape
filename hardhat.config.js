require("@nomiclabs/hardhat-waffle");
require("hardhat-gas-reporter");
const {
  TASK_COMPILE_SOLIDITY_GET_SOLC_BUILD,
} = require("hardhat/builtin-tasks/task-names");

const Common = require("@ethereumjs/common").default;
const forCustomChain = Common.forCustomChain;
Common.forCustomChain = (...args) => {
  const common = forCustomChain(...args);
  common._eips = [2537];
  return common;
};

// This is a sample Hardhat task. To learn how to create your own go to
// https://hardhat.org/guides/create-task.html
task("accounts", "Prints the list of accounts", async (taskArgs, hre) => {
  const accounts = await hre.ethers.getSigners();

  for (const account of accounts) {
    console.log(account.address);
  }
});

// Use the compiler downloaded with nix if the version matches
// Based on: https://github.com/fvictorio/hardhat-examples/tree/master/custom-solc
subtask(TASK_COMPILE_SOLIDITY_GET_SOLC_BUILD, async (args, hre, runSuper) => {
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

// You need to export an object to set up your config
// Go to https://hardhat.org/config/ to learn more

/**
 * @type import('hardhat/config').HardhatUserConfig
 */
module.exports = {
  defaultNetwork: "localhost",
  networks: {
    hardhat: {
      accounts: {
        mnemonic: process.env.TEST_MNEMONIC,
      },
      // Avoid: "InvalidInputError: Transaction gas limit is 31061912 and exceeds block gas limit of 30000000"
      gas: 25_000_000,
    },
    rinkeby: {
      url: process.env.RINKEBY_URL,
      gasPrice: 2_000_000_000,
      gas: 25_000_000,
      accounts: { mnemonic: process.env.RINKEBY_MNEMONIC },
    },
    local: {
      url: "http://localhost:8545",
    },
    arbitrum: {
      url: `https://rinkeby.arbitrum.io/rpc`,
      gasPrice: 1_000_000_000,
      gas: 25_000_000,
      accounts: { mnemonic: process.env.RINKEBY_MNEMONIC },
    },
  },
  solidity: {
    version: process.env.SOLC_VERSION,
    settings: {
      optimizer: {
        enabled: true,
        runs: 1000,
      },
    },
  },
  gasReporter: {
    enabled: process.env.REPORT_GAS ? true : false,
    showMethodSig: true,
    onlyCalledMethods: false,
  },
  mocha: {
    timeout: 120000,
  },
};
