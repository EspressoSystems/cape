use ethers::prelude::abigen;

abigen!(
    TestBN254,
    "../artifacts/contracts/TestBN254.sol/TestBN254/abi.json",
    event_derives(serde::Deserialize, serde::Serialize);

    TestRecordsMerkleTree,
    "../artifacts/contracts/TestRecordsMerkleTree.sol/TestRecordsMerkleTree/abi.json",
    event_derives(serde::Deserialize, serde::Serialize);

    TestTranscript,
    "../artifacts/contracts/TestTranscript.sol/TestTranscript/abi.json",
    event_derives(serde::Deserialize, serde::Serialize);

    ReadCAPTx,
    "../artifacts/contracts/ReadCAPTx.sol/ReadCAPTx/abi.json",
    event_derives(serde::Deserialize, serde::Serialize);

    Greeter,
    "../artifacts/contracts/Greeter.sol/Greeter/abi.json",
    event_derives(serde::Deserialize, serde::Serialize);
);
