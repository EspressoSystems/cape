import { HardhatRuntimeEnvironment } from "hardhat/types";
import { DeployFunction } from "hardhat-deploy/types";
import { utils, BigNumber } from "ethers";

function fromEnv(env_var: string, fallback: string): string {
  return process.env[env_var] || fallback;
}

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

  // Derived from USERPUBKEY~Gqoj9n3Ukd79jKV6L3q083AxJ9OiqP_z4nvVU_Gh-i8gAAAAAAAAAIhPWzWc8XSDsOQsoyBApAkt-EozfMGsMXzb1Ba5g2hP9w
  let faucetManagerEncKey = "0x884f5b359cf17483b0e42ca32040a4092df84a337cc1ac317cdbd416b983684f";
  let faucetManagerAddress = {
    x: BigNumber.from("0x2ffaa1f153d57be2f3ffa8a2d3273170f3b47a2f7aa58cfdde91d47df623aa1a"),
    y: BigNumber.from("0x0869c1246d9577b7b406785c210911e10093bb13f5d054a3c4d41bc9a64dd50d"),
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
