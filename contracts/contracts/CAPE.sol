//SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

/// @title Configurable Anonymous Payments on Ethereum
/// CAPE provides auditable anonymous payments on Ethereum.
/// @author Translucence Research, Inc.
/// @notice This is a notice.
/// @dev Developers are awesome!

import "hardhat/console.sol";

// Learn more about the ERC20 implementation
// on OpenZeppelin docs: https://docs.openzeppelin.com/contracts/4.x/erc20
import "@openzeppelin/contracts/token/ERC20/ERC20.sol";

import "solidity-bytes-utils/contracts/BytesLib.sol";
import "./libraries/AccumulatingArray.sol";
import "./libraries/EdOnBN254.sol";
import "./libraries/RescueLib.sol";
import "./libraries/VerifyingKeys.sol";
import "./interfaces/IPlonkVerifier.sol";
import "./AssetRegistry.sol";
import "./RecordsMerkleTree.sol";
import "./RootStore.sol";
import "./Queue.sol";

// TODO Remove once functions are implemented
/* solhint-disable no-unused-vars */

contract CAPE is RecordsMerkleTree, RootStore, AssetRegistry, Queue {
    mapping(uint256 => bool) public nullifiers;
    uint64 public blockHeight;
    IPlonkVerifier private _verifier;

    // In order to avoid the contract running out of gas if the queue is too large
    // we set the maximum number of pending deposits record commitments to process
    // when a new block is submitted. This is a temporary solution.
    // See https://github.com/SpectrumXYZ/cape/issues/400
    uint64 public constant MAX_QUEUE_SIZE = 10;

    using AccumulatingArray for AccumulatingArray.Data;

    bytes public constant CAPE_BURN_MAGIC_BYTES = "TRICAPE burn";
    bytes13 public constant DOM_SEP_FOREIGN_ASSET = "FOREIGN_ASSET";
    bytes14 public constant DOM_SEP_DOMESTIC_ASSET = "DOMESTIC_ASSET";
    uint256 public constant AAP_NATIVE_ASSET_CODE = 1;

    event BlockCommitted(uint64 indexed height);
    event Erc20TokensDeposited(bytes roBytes, address erc20TokenAddress, address from);

    struct AuditMemo {
        EdOnBN254.EdOnBN254Point ephemeralKey;
        uint256[] data;
    }

    enum NoteType {
        TRANSFER,
        MINT,
        FREEZE,
        BURN
    }

    struct TransferNote {
        uint256[] inputNullifiers;
        uint256[] outputCommitments;
        IPlonkVerifier.PlonkProof proof;
        AuditMemo auditMemo;
        TransferAuxInfo auxInfo;
    }

    struct BurnNote {
        TransferNote transferNote;
        RecordOpening recordOpening;
    }

    struct MintNote {
        /// nullifier for the input (i.e. transaction fee record)
        uint256 inputNullifier;
        /// output commitment for the fee change
        uint256 chgComm;
        /// output commitment for the minted asset
        uint256 mintComm;
        /// the amount of the minted asset
        uint64 mintAmount;
        /// the asset definition of the asset
        AssetDefinition mintAssetDef;
        /// Internal asset code
        uint256 mintInternalAssetCode;
        /// the validity proof of this note
        IPlonkVerifier.PlonkProof proof;
        /// memo for policy compliance specified for the designated auditor
        AuditMemo auditMemo;
        /// auxiliary information
        MintAuxInfo auxInfo;
    }

    struct FreezeNote {
        uint256[] inputNullifiers;
        uint256[] outputCommitments;
        IPlonkVerifier.PlonkProof proof;
        FreezeAuxInfo auxInfo;
    }

    struct TransferAuxInfo {
        uint256 merkleRoot;
        uint64 fee;
        uint64 validUntil;
        EdOnBN254.EdOnBN254Point txnMemoVerKey;
        bytes extraProofBoundData;
    }

    struct MintAuxInfo {
        uint256 merkleRoot;
        uint64 fee;
        EdOnBN254.EdOnBN254Point txnMemoVerKey;
    }

    struct FreezeAuxInfo {
        uint256 merkleRoot;
        uint64 fee;
        EdOnBN254.EdOnBN254Point txnMemoVerKey;
    }

    struct RecordOpening {
        uint64 amount;
        AssetDefinition assetDef;
        EdOnBN254.EdOnBN254Point userAddr;
        bool freezeFlag;
        uint256 blind;
    }

    struct CapeBlock {
        EdOnBN254.EdOnBN254Point minerAddr;
        NoteType[] noteTypes;
        TransferNote[] transferNotes;
        MintNote[] mintNotes;
        FreezeNote[] freezeNotes;
        BurnNote[] burnNotes;
    }

    constructor(
        uint8 merkleTreeHeight,
        uint64 nRoots,
        address verifierAddr
    ) RecordsMerkleTree(merkleTreeHeight) RootStore(nRoots) Queue() {
        _verifier = IPlonkVerifier(verifierAddr);
    }

    // Checks if a block is empty
    function _isBlockEmpty(CapeBlock memory block) internal returns (bool) {
        bool res = (block.transferNotes.length == 0 &&
            block.burnNotes.length == 0 &&
            block.freezeNotes.length == 0 &&
            block.mintNotes.length == 0);
        return res;
    }

    /// Insert a nullifier into the set of nullifiers.
    /// @dev Reverts if nullifier is already in nullifier set.
    function _insertNullifier(uint256 nullifier) internal {
        // This check is relied upon to prevent double spending of nullifiers
        // within the same note.
        require(!nullifiers[nullifier], "Nullifier already published");
        nullifiers[nullifier] = true;
    }

    /// Check if a nullifier array contains previously published nullifiers.
    /// @dev Does not check if the array contains duplicates.
    function _containsPublished(uint256[] memory newNullifiers) internal view returns (bool) {
        for (uint256 j = 0; j < newNullifiers.length; j++) {
            if (nullifiers[newNullifiers[j]]) {
                return true;
            }
        }
        return false;
    }

    /// Publish an array of nullifiers if none of them have been published before
    /// TODO the text after @ return does not show in docs, only the return type shows.
    /// @return `true` if the nullifiers were published, `false` if one or more nullifiers were published before.
    /// @dev Will revert if not all nullifiers can be published due to duplicates among them.
    /// @dev A block creator must not submit notes with duplicate nullifiers.
    function _publish(uint256[] memory newNullifiers) internal returns (bool) {
        if (!_containsPublished(newNullifiers)) {
            for (uint256 j = 0; j < newNullifiers.length; j++) {
                _insertNullifier(newNullifiers[j]);
            }
            return true;
        }
        return false;
    }

    /// Publish a nullifier if it hasn't been published before
    /// @return `true` if the nullifier was published, `false` if it wasn't
    function _publish(uint256 nullifier) internal returns (bool) {
        if (nullifiers[nullifier]) {
            return false;
        }
        _insertNullifier(nullifier);
        return true;
    }

    /// @notice allows to wrap some erc20 tokens into some CAPE asset defined in the record opening
    /// @param ro record opening that will be inserted in the records merkle tree once the deposit is validated.
    /// @param erc20Address address of the ERC20 token corresponding to the deposit.
    function depositErc20(RecordOpening memory ro, address erc20Address) public {
        ERC20 token = ERC20(erc20Address);
        _checkForeignAssetCode(ro.assetDef.code, erc20Address);
        require(isCapeAssetRegistered(ro.assetDef), "asset definition not registered");

        // We skip the sanity checks mentioned in the rust specification as they are optional.

        uint256 recordCommitment = _deriveRecordCommitment(ro);
        _pushToQueue(recordCommitment);
        bytes32 assetDefHash = keccak256(abi.encode(ro.assetDef));

        token.transferFrom(msg.sender, address(this), ro.amount);
        bytes memory roBytes = abi.encode(ro);
        emit Erc20TokensDeposited(roBytes, erc20Address, msg.sender);
    }

    /// @notice Checks if the asset definition code is correctly derived from the ERC20 address
    ///        of the token and the address of the depositor.
    /// @dev requires "view" to access msg.sender
    function _checkForeignAssetCode(uint256 assetDefinitionCode, address erc20Address)
        internal
        view
    {
        bytes memory description = _computeAssetDescription(erc20Address, msg.sender);
        bytes memory randomBytes = bytes.concat(
            keccak256(bytes.concat(DOM_SEP_FOREIGN_ASSET, description))
        );
        uint256 derivedCode = BN254.fromLeBytesModOrder(randomBytes);
        require(derivedCode == assetDefinitionCode, "Wrong foreign asset code");
    }

    /// @notice Checks if the asset definition code is correctly derived from the internal asset code.
    function _checkDomesticAssetCode(uint256 assetDefinitionCode, uint256 internalAssetCode)
        internal
        view
    {
        bytes memory randomBytes = bytes.concat(
            keccak256(
                bytes.concat(
                    DOM_SEP_DOMESTIC_ASSET,
                    bytes32(BN254.reverseEndianness(internalAssetCode))
                )
            )
        );
        uint256 derivedCode = BN254.fromLeBytesModOrder(randomBytes);
        require(derivedCode == assetDefinitionCode, "Wrong domestic asset code");
    }

    // TODO consider inlining once asset description is finalized. Until then it's useful
    // for testing if this computation matches the rust code.
    function _computeAssetDescription(address erc20Address, address sponsor)
        internal
        pure
        returns (bytes memory)
    {
        return
            bytes.concat("TRICAPE ERC20", bytes20(erc20Address), "sponsored by", bytes20(sponsor));
    }

    /// @notice submit a new block to the CAPE contract. Transactions are validated and the blockchain state is updated. Moreover burn transactions trigger the unwrapping of cape asset records into erc20 tokens.
    /// @param newBlock block to be processed by the CAPE contract.
    /// @param burnedRos record opening of the second outputs of the burn transactions. The information contained in these records opening allow the contract to transfer the erc20 tokens.
    function submitCapeBlock(CapeBlock memory newBlock, RecordOpening[] memory burnedRos) public {
        // Preserve the ordering of the (sub) arrays of notes.
        uint256 transferIdx = 0;
        uint256 mintIdx = 0;
        uint256 freezeIdx = 0;
        uint256 burnIdx = 0;

        uint256 maxSizeCommsArray = _computeMaxCommitments(newBlock) + _getQueueSize();
        AccumulatingArray.Data memory comms = AccumulatingArray.create(maxSizeCommsArray);

        uint256 numNotes = newBlock.noteTypes.length;

        for (uint256 i = 0; i < numNotes; i++) {
            NoteType noteType = newBlock.noteTypes[i];

            if (noteType == NoteType.TRANSFER) {
                TransferNote memory note = newBlock.transferNotes[transferIdx];
                _checkContainsRoot(note.auxInfo.merkleRoot);
                _checkTransfer(note);
                // NOTE: expiry must be checked before publishing the nullifiers
                require(!_isExpired(note), "Expired note");
                require(_publish(note.inputNullifiers), "Duplicated nullifiers");

                comms.add(note.outputCommitments);
                transferIdx += 1;
            } else if (noteType == NoteType.MINT) {
                MintNote memory note = newBlock.mintNotes[mintIdx];
                _checkContainsRoot(note.auxInfo.merkleRoot);
                require(_publish(note.inputNullifier), "Duplicated nullifiers");
                comms.add(note.mintComm);
                comms.add(note.chgComm);
                _checkDomesticAssetCode(note.mintAssetDef.code, note.mintInternalAssetCode);
                mintIdx += 1;
            } else if (noteType == NoteType.FREEZE) {
                FreezeNote memory note = newBlock.freezeNotes[freezeIdx];
                _checkContainsRoot(note.auxInfo.merkleRoot);

                require(_publish(note.inputNullifiers), "Duplicated nullifiers");
                comms.add(note.outputCommitments);

                freezeIdx += 1;
            } else if (noteType == NoteType.BURN) {
                BurnNote memory note = newBlock.burnNotes[burnIdx];
                TransferNote memory transfer = note.transferNote;
                _checkContainsRoot(transfer.auxInfo.merkleRoot);
                _checkBurn(note);

                require(_publish(transfer.inputNullifiers), "Dupl. nullifier(s)");
                // TODO do we need a special logic for how to handle outputs record commitments with BURN notes
                comms.add(transfer.outputCommitments);

                // TODO handle withdrawal (better done at end if call is external
                //      or have other reentrancy protection)

                burnIdx += 1;
            }
        }

        // Batch verify plonk proofs
        IPlonkVerifier.VerifyingKey[] memory vks = new IPlonkVerifier.VerifyingKey[](numNotes);
        uint256[][] memory publicInputs = new uint256[][](numNotes);
        IPlonkVerifier.PlonkProof[] memory proofs = new IPlonkVerifier.PlonkProof[](numNotes);
        bytes[] memory extraMsgs = new bytes[](numNotes);
        transferIdx = 0;
        mintIdx = 0;
        freezeIdx = 0;
        burnIdx = 0;
        uint256 proofIdx = 0;

        for (uint256 i = 0; i < numNotes; i++) {
            if (newBlock.noteTypes[i] == NoteType.TRANSFER) {
                TransferNote memory note = newBlock.transferNotes[transferIdx];
                transferIdx++;
                (
                    vks[proofIdx],
                    publicInputs[proofIdx],
                    proofs[proofIdx],
                    extraMsgs[proofIdx]
                ) = _prepareForProofVerification(note);
                proofIdx++;
            } else if (newBlock.noteTypes[i] == NoteType.MINT) {
                MintNote memory note = newBlock.mintNotes[mintIdx];
                mintIdx++;

                (
                    vks[proofIdx],
                    publicInputs[proofIdx],
                    proofs[proofIdx],
                    extraMsgs[proofIdx]
                ) = _prepareForProofVerification(note);
                proofIdx++;
            } else if (newBlock.noteTypes[i] == NoteType.FREEZE) {
                FreezeNote memory note = newBlock.freezeNotes[freezeIdx];
                freezeIdx++;

                (
                    vks[proofIdx],
                    publicInputs[proofIdx],
                    proofs[proofIdx],
                    extraMsgs[proofIdx]
                ) = _prepareForProofVerification(note);
                proofIdx++;
            } else if (newBlock.noteTypes[i] == NoteType.BURN) {
                BurnNote memory note = newBlock.burnNotes[burnIdx];
                burnIdx++;

                (
                    vks[proofIdx],
                    publicInputs[proofIdx],
                    proofs[proofIdx],
                    extraMsgs[proofIdx]
                ) = _prepareForProofVerification(note);
                proofIdx++;
            } else {
                revert("Cape: unreachable!");
            }
        }
        // Skip the batch plonk verification if the block is empty
        if (numNotes > 0) {
            require(
                _verifier.batchVerify(vks, publicInputs, proofs, extraMsgs),
                "Cape: batch verify failed."
            );
        }

        // Process the pending deposits obtained after calling `depositErc20`
        // There are some pending deposits to process
        uint256 numPendingDeposits = _getQueueSize();

        // See See https://github.com/SpectrumXYZ/cape/issues/400 for why we check that the queue has at most MAX_QUEUE_SIZE elements
        if ((numPendingDeposits > 0) && (numPendingDeposits < MAX_QUEUE_SIZE)) {
            for (uint256 i = 0; i < numPendingDeposits; i++) {
                uint256 rc = _getQueueElem(i);
                comms.add(rc);
            }
            // Empty the queue now the record commitments are ready to be inserted
            _emptyQueue();
        }

        // Only update the merkle tree and add the root if the list of records commitments is non empty
        if (!comms.isEmpty()) {
            _updateRecordsMerkleTree(comms.toArray());
            _addRoot(_rootValue);
        }

        // In all cases (the block is empty or not), the height is incremented.
        blockHeight += 1;
        emit BlockCommitted(blockHeight);
    }

    function _handleWithdrawal() internal {
        // TODO
    }

    /// @dev Compute an upper bound on the number of records to be inserted
    function _computeMaxCommitments(CapeBlock memory newBlock) internal pure returns (uint256) {
        // MintNote always has 2 commitments: mint_comm, chg_comm
        uint256 maxComms = 2 * newBlock.mintNotes.length;
        for (uint256 i = 0; i < newBlock.transferNotes.length; i++) {
            maxComms += newBlock.transferNotes[i].outputCommitments.length;
        }
        for (uint256 i = 0; i < newBlock.burnNotes.length; i++) {
            maxComms += newBlock.burnNotes[i].transferNote.outputCommitments.length;
        }
        for (uint256 i = 0; i < newBlock.freezeNotes.length; i++) {
            maxComms += newBlock.freezeNotes[i].outputCommitments.length;
        }
        return maxComms;
    }

    function _checkTransfer(TransferNote memory note) internal pure {
        // TODO consider moving _checkContainsRoot into _check[NoteType] functions
        require(
            !_containsBurnPrefix(note.auxInfo.extraProofBoundData),
            "Burn prefix in transfer note"
        );
    }

    function _isExpired(TransferNote memory note) internal view returns (bool) {
        return note.auxInfo.validUntil < blockHeight;
    }

    function _checkBurn(BurnNote memory note) internal view {
        bytes memory extra = note.transferNote.auxInfo.extraProofBoundData;
        require(_containsBurnPrefix(extra), "Bad burn tag");
        require(_containsBurnDestination(extra), "Bad burn destination");
        require(_containsBurnRecord(note), "Bad record commitment");
    }

    function _containsBurnPrefix(bytes memory extraProofBoundData) internal pure returns (bool) {
        if (extraProofBoundData.length < 12) {
            return false;
        }
        return BytesLib.equal(BytesLib.slice(extraProofBoundData, 0, 12), CAPE_BURN_MAGIC_BYTES);
    }

    function _containsBurnDestination(bytes memory extraProofBoundData)
        internal
        view
        returns (bool)
    {
        if (extraProofBoundData.length < 32) {
            return false;
        }
        return BytesLib.toAddress(extraProofBoundData, 12) == address(0);
    }

    function _containsBurnRecord(BurnNote memory note) internal view returns (bool) {
        if (note.transferNote.outputCommitments.length < 2) {
            return false;
        }
        uint256 rc = _deriveRecordCommitment(note.recordOpening);
        return rc == note.transferNote.outputCommitments[1];
    }

    function _deriveRecordCommitment(RecordOpening memory ro) internal view returns (uint256 rc) {
        require(ro.assetDef.policy.revealMap < 2**12, "Reveal map exceeds 12 bits");

        // No overflow check, only 12 bits in reveal map
        uint256 revealMapAndFreezeFlag = 2 *
            ro.assetDef.policy.revealMap +
            (ro.freezeFlag ? 1 : 0);

        // blind in front of rest -> 13 elements, pad to 15 (5 x 3)
        uint256[15] memory inputs = [
            ro.blind,
            ro.amount,
            ro.assetDef.code,
            ro.userAddr.x,
            ro.userAddr.y,
            ro.assetDef.policy.auditorPk.x,
            ro.assetDef.policy.auditorPk.y,
            ro.assetDef.policy.credPk.x,
            ro.assetDef.policy.credPk.y,
            ro.assetDef.policy.freezerPk.x,
            ro.assetDef.policy.freezerPk.y,
            revealMapAndFreezeFlag,
            ro.assetDef.policy.revealThreshold,
            0,
            0
        ];

        return RescueLib.commit(inputs);
    }

    // an overloadded function (one for each note type) to prepare all inputs necessary
    // for batch verification of the plonk proof
    function _prepareForProofVerification(TransferNote memory note)
        internal
        view
        returns (
            IPlonkVerifier.VerifyingKey memory vk,
            uint256[] memory publicInput,
            IPlonkVerifier.PlonkProof memory proof,
            bytes memory transcriptInitMsg
        )
    {
        // load the correct (hardcoded) vk
        vk = VerifyingKeys.getVkById(
            VerifyingKeys.getEncodedId(
                uint8(NoteType.TRANSFER),
                uint8(note.inputNullifiers.length),
                uint8(note.outputCommitments.length),
                uint8(_merkleTreeHeight)
            )
        );
        // prepare public inputs
        // 4: root, native_asset_code, valid_until, fee
        // 2: audit_memo.ephemeral_key (x and y)
        publicInput = new uint256[](
            4 +
                note.inputNullifiers.length +
                note.outputCommitments.length +
                2 +
                note.auditMemo.data.length
        );
        publicInput[0] = note.auxInfo.merkleRoot;
        publicInput[1] = AAP_NATIVE_ASSET_CODE;
        publicInput[2] = note.auxInfo.validUntil;
        publicInput[3] = note.auxInfo.fee;
        {
            uint256 idx = 4;
            for (uint256 i = 0; i < note.inputNullifiers.length; i++) {
                publicInput[idx + i] = note.inputNullifiers[i];
            }
            idx += note.inputNullifiers.length;

            for (uint256 i = 0; i < note.outputCommitments.length; i++) {
                publicInput[idx + i] = note.outputCommitments[i];
            }
            idx += note.outputCommitments.length;

            publicInput[idx] = note.auditMemo.ephemeralKey.x;
            publicInput[idx + 1] = note.auditMemo.ephemeralKey.y;
            idx += 2;

            for (uint256 i = 0; i < note.auditMemo.data.length; i++) {
                publicInput[idx + i] = note.auditMemo.data[i];
            }
        }

        // extract out proof
        proof = note.proof;

        // prepare transcript init messages
        transcriptInitMsg = abi.encodePacked(
            EdOnBN254.serialize(note.auxInfo.txnMemoVerKey),
            note.auxInfo.extraProofBoundData
        );
    }

    function _prepareForProofVerification(BurnNote memory note)
        internal
        view
        returns (
            IPlonkVerifier.VerifyingKey memory,
            uint256[] memory,
            IPlonkVerifier.PlonkProof memory,
            bytes memory
        )
    {
        return _prepareForProofVerification(note.transferNote);
    }

    function _prepareForProofVerification(MintNote memory note)
        internal
        view
        returns (
            IPlonkVerifier.VerifyingKey memory vk,
            uint256[] memory publicInput,
            IPlonkVerifier.PlonkProof memory proof,
            bytes memory transcriptInitMsg
        )
    {
        // load the correct (hardcoded) vk
        vk = VerifyingKeys.getVkById(
            VerifyingKeys.getEncodedId(
                uint8(NoteType.MINT),
                1, // num of input
                2, // num of output
                uint8(_merkleTreeHeight)
            )
        );

        // prepare public inputs
        // 9: see below; 8: asset policy; rest: audit memo
        publicInput = new uint256[](9 + 8 + 2 + note.auditMemo.data.length);
        publicInput[0] = note.auxInfo.merkleRoot;
        publicInput[1] = AAP_NATIVE_ASSET_CODE;
        publicInput[2] = note.inputNullifier;
        publicInput[3] = note.auxInfo.fee;
        publicInput[4] = note.mintComm;
        publicInput[5] = note.chgComm;
        publicInput[6] = note.mintAmount;
        publicInput[7] = note.mintAssetDef.code;
        publicInput[8] = note.mintInternalAssetCode;

        publicInput[9] = note.mintAssetDef.policy.revealMap;
        publicInput[10] = note.mintAssetDef.policy.auditorPk.x;
        publicInput[11] = note.mintAssetDef.policy.auditorPk.y;
        publicInput[12] = note.mintAssetDef.policy.credPk.x;
        publicInput[13] = note.mintAssetDef.policy.credPk.y;
        publicInput[14] = note.mintAssetDef.policy.freezerPk.x;
        publicInput[15] = note.mintAssetDef.policy.freezerPk.y;
        publicInput[16] = note.mintAssetDef.policy.revealThreshold;

        {
            publicInput[17] = note.auditMemo.ephemeralKey.x;
            publicInput[18] = note.auditMemo.ephemeralKey.y;

            uint256 idx = 19;
            for (uint256 i = 0; i < note.auditMemo.data.length; i++) {
                publicInput[idx + i] = note.auditMemo.data[i];
            }
        }

        // extract out proof
        proof = note.proof;

        // prepare transcript init messages
        transcriptInitMsg = EdOnBN254.serialize(note.auxInfo.txnMemoVerKey);
    }

    function _prepareForProofVerification(FreezeNote memory note)
        internal
        view
        returns (
            IPlonkVerifier.VerifyingKey memory vk,
            uint256[] memory publicInput,
            IPlonkVerifier.PlonkProof memory proof,
            bytes memory transcriptInitMsg
        )
    {
        // load the correct (hardcoded) vk
        vk = VerifyingKeys.getVkById(
            VerifyingKeys.getEncodedId(
                uint8(NoteType.FREEZE),
                uint8(note.inputNullifiers.length),
                uint8(note.outputCommitments.length),
                uint8(_merkleTreeHeight)
            )
        );

        // prepare public inputs
        publicInput = new uint256[](
            3 + note.inputNullifiers.length + note.outputCommitments.length
        );
        publicInput[0] = note.auxInfo.merkleRoot;
        publicInput[1] = AAP_NATIVE_ASSET_CODE;
        publicInput[2] = note.auxInfo.fee;
        {
            uint256 idx = 3;
            for (uint256 i = 0; i < note.inputNullifiers.length; i++) {
                publicInput[idx + i] = note.inputNullifiers[i];
            }
            idx += note.inputNullifiers.length;

            for (uint256 i = 0; i < note.outputCommitments.length; i++) {
                publicInput[idx + i] = note.outputCommitments[i];
            }
        }

        // extract out proof
        proof = note.proof;

        // prepare transcript init messages
        transcriptInitMsg = EdOnBN254.serialize(note.auxInfo.txnMemoVerKey);
    }
}
