const bls = require("noble-bls12-381");
// if you're using single file, use global variable nobleBls12381

// You can use Uint8Array, or hex string for readability
const privateKey =
  "67d53f170b908cabb9eb326c3c337762d59289a8fec79f7bc9254b584b73265c";
const privateKeys = [
  "18f020b98eb798752a50ed0563b079c125b0db5dd0b1060d1c1b47d4a193e1e4",
  "ed69a8c50cf8c9836be3b67c7eeff416612d45ba39a5c099d48fa668bf558c9c",
  "16ae669f3be7a2121e17d0c68c05a8f3d6bef21ec0f2315f1d7aec12484e4cf5",
];
const message = "64726e3da8";
const messages = ["d2", "0d98", "05caf3"];

(async () => {
  const publicKey = bls.getPublicKey(privateKey);
  const publicKeys = privateKeys.map(bls.getPublicKey);

  const signature = await bls.sign(message, privateKey);
  const isCorrect = await bls.verify(signature, message, publicKey);
  console.log("key", publicKey);
  console.log("signature", signature);
  console.log("is correct:", isCorrect);

  // Sign 1 msg with 3 keys
  const signatures2 = await Promise.all(
    privateKeys.map((p) => bls.sign(message, p))
  );
  const aggPubKey2 = bls.aggregatePublicKeys(publicKeys);
  const aggSignature2 = bls.aggregateSignatures(signatures2);
  const isCorrect2 = await bls.verify(aggSignature2, message, aggPubKey2);
  console.log();
  console.log("signatures are", signatures2);
  console.log("merged to one signature", aggSignature2);
  console.log("is correct:", isCorrect2);

  // Sign 3 msgs with 3 keys
  const signatures3 = await Promise.all(
    privateKeys.map((p, i) => bls.sign(messages[i], p))
  );
  const aggSignature3 = bls.aggregateSignatures(signatures3);
  const isCorrect3 = await bls.verifyBatch(aggSignature3, messages, publicKeys);
  console.log();
  console.log("keys", publicKeys);
  console.log("signatures", signatures3);
  console.log("merged to one signature", aggSignature3);
  console.log("is correct:", isCorrect3);
})();
