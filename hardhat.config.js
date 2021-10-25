require("@nomiclabs/hardhat-waffle");
require("hardhat-gas-reporter");
const {
  TASK_COMPILE_SOLIDITY_GET_SOLC_BUILD,
} = require("hardhat/builtin-tasks/task-names");

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

    goerli: {
      url: process.env.GOERLI_URL,
      gasPrice: 2_000_000_000,
      gas: 25_000_000,
      accounts: { mnemonic: process.env.GOERLI_MNEMONIC },
    },

    localhost: {
      url: `http://localhost:${process.env.RPC_PORT || 8545}`,
      timeout: 120000, // when running against hardhat, some tests are very slow
    },

    arbitrum: {
      url: `https://rinkeby.arbitrum.io/rpc`,
      gasPrice: 1_000_000_000,
      gas: 25_000_000,
      accounts: { mnemonic: process.env.RINKEBY_MNEMONIC },
    },

    // Network config from
    // https://github.com/OffchainLabs/arbitrum/blob/b89d2d626f7e78f3c24624ba23c2fd8d2bad42ac/packages/arb-bridge-eth/hardhat.config.ts#L337-L349
    arbitrum_dev: {
      url: "http://127.0.0.1:8547",
      // url: 'https://kovan3.arbitrum.io/rpc',
      gas: 999999999999999,
      accounts: {
        mnemonic:
          "jar deny prosper gasp flush glass core corn alarm treat leg smart",
        path: "m/44'/60'/0'/0",
        initialIndex: 0,
        count: 10,
      },
      timeout: 100000,
    },
  },
  solidity: {
    version: process.env.SOLC_VERSION,
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
    onlyCalledMethods: false,
  },
  mocha: {
    timeout: 300000,
  },
};
