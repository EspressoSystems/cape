use ethers::prelude::abigen;

abigen!(
    TestBN254,
    "/home/lulu/r/translucence/aap-on-ethereum/contracts/artifacts/contracts/TestBN254.sol/TestBN254/abi.json",
    event_derives(serde::Deserialize, serde::Serialize);

    TestRecordsMerkleTree,
    "/home/lulu/r/translucence/aap-on-ethereum/contracts/artifacts/contracts/TestRecordsMerkleTree.sol/TestRecordsMerkleTree/abi.json",
    event_derives(serde::Deserialize, serde::Serialize);

    TestTranscript,
    "/home/lulu/r/translucence/aap-on-ethereum/contracts/artifacts/contracts/TestTranscript.sol/TestTranscript/abi.json",
    event_derives(serde::Deserialize, serde::Serialize);

    // Currently will not compile
    // CAPE,
    // "/home/lulu/r/translucence/aap-on-ethereum/contracts/artifacts/contracts/CAPE.sol/CAPE/abi.json",
    // event_derives(serde::Deserialize, serde::Serialize);

    // TestCAPE,
    // "/home/lulu/r/translucence/aap-on-ethereum/contracts/artifacts/contracts/TestCAPE.sol/TestCAPE/abi.json",
    // event_derives(serde::Deserialize, serde::Serialize);

    Greeter,
    "/home/lulu/r/translucence/aap-on-ethereum/contracts/artifacts/contracts/Greeter.sol/Greeter/abi.json",
    event_derives(serde::Deserialize, serde::Serialize);
);
