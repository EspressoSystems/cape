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

  // Faucet manager wallet public key: USERPUBKEY~uc8DIUZc8j7P01_QiXGYjg_KDDIpuJ1y3bfWjQ3ByyggAAAAAAAAAFezuiSZRdTqt2V8lesbMyY6-QUKuLG-QXIAXZnYUoAbyw
  // address generation code:
  // ```rust
  // use ark_std::str::FromStr;
  // let result = "USERPUBKEY~uc8DIUZc8j7P01_QiXGYjg_KDDIpuJ1y3bfWjQ3ByyggAAAAAAAAAFezuiSZRdTqt2V8lesbMyY6-QUKuLG-QXIAXZnYUoAbyw";
  // let pk = UserPubKey::from_str(&result).unwrap_or_default();
  // ark_std::eprintln!(
  //     "x: {}, y: {}",
  //     pk.address().internal().x,
  //     pk.address().internal().y
  // );
  // ```
  const faucetManager = {
    x: BigNumber.from("0x28CBC10D8DD6B7DD729DB829320CCA0F8E987189D05FD3CF3EF25C462103CFB9"),
    y: BigNumber.from("0x0B9F465C2530B75A937FDF0B5AD4EF76D0A939E9B5F5F36C4A9BF6279EBC19F0"),
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
