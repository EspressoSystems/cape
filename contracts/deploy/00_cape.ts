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

  // Faucet manager wallet public key: USERPUBKEY~RQtOX72Af9P9nxNKyZZXjQEN1j3FC02tyg8cXgHinasgAAAAAAAAAHvbhTwmXsaZJ6pSsjJQeDNpoD1Dp65tt5njNUbJkxNugQ
  // address generation code:
  // ```rust
  // use ark_std::str::FromStr;
  // let result = "USERPUBKEY~RQtOX72Af9P9nxNKyZZXjQEN1j3FC02tyg8cXgHinasgAAAAAAAAAHvbhTwmXsaZJ6pSsjJQeDNpoD1Dp65tt5njNUbJkxNugQ";
  // let pk = UserPubKey::from_str(&result).unwrap_or_default();
  // ark_std::eprintln!(
  //     "x: {}, y: {}",
  //     pk.address().internal().x,
  //     pk.address().internal().y
  // );
  // ```
  const faucetManager = {
    x: BigNumber.from("0x2B9DE2015E1C0FCAAD4D0BC53DD60D018D5796C94A139FFDD37F80BD5F4E0B45"),
    y: BigNumber.from("0x22E38BAE576E313A11DE2BE09569830F3F7BF414592BA32812D82857947400CC"),
  };

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
      from: deployer,
    },
    "faucetSetupForTestnet",
    faucetManager
  );
};

export default func;
func.tags = ["CAPE"];
