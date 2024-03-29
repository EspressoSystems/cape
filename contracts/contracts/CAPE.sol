// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

pragma solidity ^0.8.0;

/// @title Configurable Anonymous Payments for Ethereum
/// CAPE provides auditable anonymous payments on Ethereum.
/// @author Espresso Systems <hello@espressosys.com>

import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@rari-capital/solmate/src/utils/SafeTransferLib.sol";

import "solidity-bytes-utils/contracts/BytesLib.sol";
import "./libraries/AccumulatingArray.sol";
import "./libraries/EdOnBN254.sol";
import "./libraries/RescueLib.sol";
import "./libraries/VerifyingKeys.sol";
import "./interfaces/IPlonkVerifier.sol";
import "./interfaces/IRecordsMerkleTree.sol";
import "./AssetRegistry.sol";
import "./RootStore.sol";

contract CAPE is RootStore, AssetRegistry, ReentrancyGuard {
    using AccumulatingArray for AccumulatingArray.Data;

    mapping(uint256 => bool) public nullifiers;
    uint64 public blockHeight;
    IPlonkVerifier private _verifier;
    IRecordsMerkleTree internal _recordsMerkleTree;
    uint256[] public pendingDeposits;

    // NOTE: used for faucet in testnet only, will be removed for mainnet
    address public deployer;
    bool public faucetInitialized;

    bytes public constant CAPE_BURN_MAGIC_BYTES = "EsSCAPE burn";
    uint256 public constant CAPE_BURN_MAGIC_BYTES_SIZE = 12;
    // In order to avoid the contract running out of gas if the queue is too large
    // we set the maximum number of pending deposits record commitments to process
    // when a new block is submitted. This is a temporary solution.
    // See https://github.com/EspressoSystems/cape/issues/400
    uint256 public constant MAX_NUM_PENDING_DEPOSIT = 10;

    event FaucetInitialized(bytes roBytes);

    event BlockCommitted(
        uint64 indexed height,
        uint256[] depositCommitments,
        // What follows is a `CapeBlock` struct split up into fields.
        // This may no longer be necessary once
        // https://github.com/gakonst/ethers-rs/issues/1220
        // is fixed.
        bytes minerAddr,
        bytes noteTypes,
        bytes transferNotes,
        bytes mintNotes,
        bytes freezeNotes,
        bytes burnNotes
    );

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
        uint128 mintAmount;
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
        uint128 fee;
        uint64 validUntil;
        EdOnBN254.EdOnBN254Point txnMemoVerKey;
        bytes extraProofBoundData;
    }

    struct MintAuxInfo {
        uint256 merkleRoot;
        uint128 fee;
        EdOnBN254.EdOnBN254Point txnMemoVerKey;
    }

    struct FreezeAuxInfo {
        uint256 merkleRoot;
        uint128 fee;
        EdOnBN254.EdOnBN254Point txnMemoVerKey;
    }

    struct RecordOpening {
        uint128 amount;
        AssetDefinition assetDef;
        EdOnBN254.EdOnBN254Point userAddr;
        bytes32 encKey;
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

    /// @notice CAPE contract constructor method.
    /// @param nRoots number of the most recent roots of the records merkle tree to be stored
    /// @param verifierAddr address of the Plonk Verifier contract
    constructor(
        uint64 nRoots,
        address verifierAddr,
        address recordsMerkleTreeAddr
    ) RootStore(nRoots) {
        _verifier = IPlonkVerifier(verifierAddr);
        _recordsMerkleTree = IRecordsMerkleTree(recordsMerkleTreeAddr);

        // NOTE: used for faucet in testnet only, will be removed for mainnet
        deployer = msg.sender;
    }

    /// @notice Allocate native token faucet to a manager. For testnet only.
    /// @param faucetManagerAddress address of public key of faucet manager for CAP native token (testnet only!)
    /// @param faucetManagerEncKey public key of faucet manager for CAP native token (testnet only!)
    function faucetSetupForTestnet(
        EdOnBN254.EdOnBN254Point memory faucetManagerAddress,
        bytes32 faucetManagerEncKey
    ) external {
        // faucet can only be set up once by the manager
        require(msg.sender == deployer, "Only invocable by deployer");
        require(!faucetInitialized, "Faucet already set up");

        // allocate maximum possible amount of native CAP token to faucet manager on testnet
        // max amount len is set to 63 bits: https://github.com/EspressoSystems/cap/blob/main/src/constants.rs#L50-L51
        RecordOpening memory ro = RecordOpening(
            type(uint128).max / 2,
            nativeDomesticAsset(),
            faucetManagerAddress,
            faucetManagerEncKey,
            false,
            0 // arbitrary blind factor
        );
        uint256[] memory recordCommitments = new uint256[](1);
        recordCommitments[0] = _deriveRecordCommitment(ro);

        // Insert the record into record accumulator.
        //
        // This is a call to our own contract, not an arbitrary external contract.
        // slither-disable-next-line reentrancy-no-eth
        _recordsMerkleTree.updateRecordsMerkleTree(recordCommitments);
        // slither-disable-next-line reentrancy-benign
        _addRoot(_recordsMerkleTree.getRootValue());

        // slither-disable-next-line reentrancy-events
        emit FaucetInitialized(abi.encode(ro));
        faucetInitialized = true;
    }

    /// @notice Publish an array of nullifiers.
    /// @dev Requires all nullifiers to be unique and unpublished.
    /// @dev A block creator must not submit notes with duplicate nullifiers.
    /// @param newNullifiers list of nullifiers to publish
    function _publish(uint256[] memory newNullifiers) internal {
        for (uint256 j = 0; j < newNullifiers.length; j++) {
            _publish(newNullifiers[j]);
        }
    }

    /// @notice Publish a nullifier if it hasn't been published before.
    /// @dev Reverts if the nullifier is already published.
    /// @param nullifier nullifier to publish
    function _publish(uint256 nullifier) internal {
        require(!nullifiers[nullifier], "Nullifier already published");
        nullifiers[nullifier] = true;
    }

    /// @notice Wraps ERC-20 tokens into a CAPE asset defined in the record opening.
    /// @param ro record opening that will be inserted in the records merkle tree once the deposit is validated
    /// @param erc20Address address of the ERC-20 token corresponding to the deposit
    function depositErc20(RecordOpening memory ro, address erc20Address) external nonReentrant {
        require(isCapeAssetRegistered(ro.assetDef), "Asset definition not registered");
        require(lookup(ro.assetDef) == erc20Address, "Wrong ERC20 address");

        // We skip the sanity checks mentioned in the rust specification as they are optional.
        if (pendingDeposits.length >= MAX_NUM_PENDING_DEPOSIT) {
            revert("Pending deposits queue is full");
        }
        pendingDeposits.push(_deriveRecordCommitment(ro));

        SafeTransferLib.safeTransferFrom(
            ERC20(erc20Address),
            msg.sender,
            address(this),
            ro.amount
        );

        emit Erc20TokensDeposited(abi.encode(ro), erc20Address, msg.sender);
    }

    /// @notice Submit a new block with extra data to the CAPE contract.
    /// @param newBlock block to be processed by the CAPE contract
    /// @param {bytes} extraData data to be stored in calldata; this data is ignored by the contract function
    function submitCapeBlockWithMemos(
        CapeBlock memory newBlock,
        bytes calldata /* extraData */
    ) external {
        submitCapeBlock(newBlock);
    }

    /// @notice Submit a new block to the CAPE contract.
    /// @dev Transactions are validated and the blockchain state is updated. Moreover *BURN* transactions trigger the unwrapping of cape asset records into erc20 tokens.
    /// @param newBlock block to be processed by the CAPE contract.
    function submitCapeBlock(CapeBlock memory newBlock) public nonReentrant {
        AccumulatingArray.Data memory commitments = AccumulatingArray.create(
            _computeNumCommitments(newBlock) + pendingDeposits.length
        );

        uint256 numNotes = newBlock.noteTypes.length;

        // Batch verify plonk proofs
        IPlonkVerifier.VerifyingKey[] memory vks = new IPlonkVerifier.VerifyingKey[](numNotes);
        uint256[][] memory publicInputs = new uint256[][](numNotes);
        IPlonkVerifier.PlonkProof[] memory proofs = new IPlonkVerifier.PlonkProof[](numNotes);
        bytes[] memory extraMsgs = new bytes[](numNotes);

        // Preserve the ordering of the (sub) arrays of notes.
        uint256 transferIdx = 0;
        uint256 mintIdx = 0;
        uint256 freezeIdx = 0;
        uint256 burnIdx = 0;

        // We require either the block or the pending deposits queue to be non empty. That is we expect the block submission to trigger some change in the blockchain state.
        // The reason is that, due to race conditions, it is possible to have the relayer send an empty block while the pending deposits queue is still empty.
        // If we do not reject the block, the `blockHeight` contract variable will be incremented, yet the set of records merkle tree roots will be unchanged.
        // On the other side, the wallet assumes that the blockHeight is equal to the number of roots and thus, in the case of a block submission that only increments `blockHeight`,
        // the wallet and the contract states become inconsistent.
        require(!((numNotes == 0) && (pendingDeposits.length == 0)), "Block must be non-empty");

        for (uint256 i = 0; i < numNotes; i++) {
            NoteType noteType = newBlock.noteTypes[i];

            if (noteType == NoteType.TRANSFER) {
                TransferNote memory note = newBlock.transferNotes[transferIdx];
                transferIdx += 1;

                _checkContainsRoot(note.auxInfo.merkleRoot);
                _checkTransfer(note);
                require(!_isExpired(note), "Expired note");

                _publish(note.inputNullifiers);

                commitments.add(note.outputCommitments);

                (vks[i], publicInputs[i], proofs[i], extraMsgs[i]) = _prepareForProofVerification(
                    note
                );
            } else if (noteType == NoteType.MINT) {
                MintNote memory note = newBlock.mintNotes[mintIdx];
                mintIdx += 1;

                _checkContainsRoot(note.auxInfo.merkleRoot);
                _checkDomesticAssetCode(note.mintAssetDef.code, note.mintInternalAssetCode);

                _publish(note.inputNullifier);

                commitments.add(note.chgComm);
                commitments.add(note.mintComm);

                (vks[i], publicInputs[i], proofs[i], extraMsgs[i]) = _prepareForProofVerification(
                    note
                );
            } else if (noteType == NoteType.FREEZE) {
                FreezeNote memory note = newBlock.freezeNotes[freezeIdx];
                freezeIdx += 1;

                _checkContainsRoot(note.auxInfo.merkleRoot);

                _publish(note.inputNullifiers);

                commitments.add(note.outputCommitments);

                (vks[i], publicInputs[i], proofs[i], extraMsgs[i]) = _prepareForProofVerification(
                    note
                );
            } else if (noteType == NoteType.BURN) {
                BurnNote memory note = newBlock.burnNotes[burnIdx];
                burnIdx += 1;

                _checkContainsRoot(note.transferNote.auxInfo.merkleRoot);
                _checkBurn(note);

                _publish(note.transferNote.inputNullifiers);

                // Insert all the output commitments to the records merkle tree except from the second one (corresponding to the burned output)
                for (uint256 j = 0; j < note.transferNote.outputCommitments.length; j++) {
                    if (j != 1) {
                        commitments.add(note.transferNote.outputCommitments[j]);
                    }
                }

                (vks[i], publicInputs[i], proofs[i], extraMsgs[i]) = _prepareForProofVerification(
                    note
                );

                // Send the tokens
                _handleWithdrawal(note);
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
        for (uint256 i = 0; i < pendingDeposits.length; i++) {
            commitments.add(pendingDeposits[i]);
        }

        // Only update the merkle tree and add the root if the list of records commitments is non empty
        if (!commitments.isEmpty()) {
            // This is a call to our own contract, not an arbitrary external contract.
            // slither-disable-next-line reentrancy-no-eth
            _recordsMerkleTree.updateRecordsMerkleTree(commitments.items);
            // slither-disable-next-line reentrancy-benign
            _addRoot(_recordsMerkleTree.getRootValue());
        }

        // In all cases (the block is empty or not), the height is incremented.
        blockHeight += 1;

        // Inform clients about the new block and the processed deposits.
        // slither-disable-next-line reentrancy-events
        _emitBlockEvent(newBlock);

        // Empty the queue now that the record commitments have been inserted
        delete pendingDeposits;
    }

    /// @notice This function only exists to avoid a stack too deep compilation error.
    function _emitBlockEvent(CapeBlock memory newBlock) internal {
        emit BlockCommitted(
            blockHeight,
            pendingDeposits,
            abi.encode(newBlock.minerAddr),
            abi.encode(newBlock.noteTypes),
            abi.encode(newBlock.transferNotes),
            abi.encode(newBlock.mintNotes),
            abi.encode(newBlock.freezeNotes),
            abi.encode(newBlock.burnNotes)
        );
    }

    /// @dev send the ERC-20 tokens equivalent to the asset records being burnt. Recall that the burned record opening is contained inside the note.
    /// @param note note of type *BURN*
    function _handleWithdrawal(BurnNote memory note) internal {
        address ercTokenAddress = lookup(note.recordOpening.assetDef);

        // Extract recipient address
        address recipientAddress = BytesLib.toAddress(
            note.transferNote.auxInfo.extraProofBoundData,
            CAPE_BURN_MAGIC_BYTES_SIZE
        );
        SafeTransferLib.safeTransfer(
            ERC20(ercTokenAddress),
            recipientAddress,
            note.recordOpening.amount
        );
    }

    /// @dev Compute an upper bound on the number of records to be inserted
    function _computeNumCommitments(CapeBlock memory newBlock) internal pure returns (uint256) {
        // MintNote always has 2 commitments: mint_comm, chg_comm
        uint256 numComms = 2 * newBlock.mintNotes.length;
        for (uint256 i = 0; i < newBlock.transferNotes.length; i++) {
            numComms += newBlock.transferNotes[i].outputCommitments.length;
        }
        for (uint256 i = 0; i < newBlock.burnNotes.length; i++) {
            // Subtract one for the burn record commitment that is not inserted.
            // The function _containsBurnRecord checks that there are at least 2 output commitments.
            numComms += newBlock.burnNotes[i].transferNote.outputCommitments.length - 1;
        }
        for (uint256 i = 0; i < newBlock.freezeNotes.length; i++) {
            numComms += newBlock.freezeNotes[i].outputCommitments.length;
        }
        return numComms;
    }

    /// @dev Verify if a note is of type *TRANSFER*.
    /// @param note note which could be of type *TRANSFER* or *BURN*
    function _checkTransfer(TransferNote memory note) internal pure {
        require(
            !_containsBurnPrefix(note.auxInfo.extraProofBoundData),
            "Burn prefix in transfer note"
        );
    }

    /// @dev Check if a note has expired.
    /// @param note note for which we want to check its timestamp against the current block height
    function _isExpired(TransferNote memory note) internal view returns (bool) {
        return note.auxInfo.validUntil < blockHeight;
    }

    /// @dev Check if a burn note is well formed.
    /// @param note note of type *BURN*
    function _checkBurn(BurnNote memory note) internal view {
        bytes memory extra = note.transferNote.auxInfo.extraProofBoundData;
        require(_containsBurnPrefix(extra), "Bad burn tag");
        require(_containsBurnRecord(note), "Bad record commitment");
    }

    /// @dev Checks if a sequence of bytes contains hardcoded prefix.
    /// @param byteSeq sequence of bytes
    function _containsBurnPrefix(bytes memory byteSeq) internal pure returns (bool) {
        if (byteSeq.length < CAPE_BURN_MAGIC_BYTES_SIZE) {
            return false;
        }
        return
            BytesLib.equal(
                BytesLib.slice(byteSeq, 0, CAPE_BURN_MAGIC_BYTES_SIZE),
                CAPE_BURN_MAGIC_BYTES
            );
    }

    /// @dev Check if the burned record opening and the record commitment in position 1 are consistent.
    /// @param note note of type *BURN*
    function _containsBurnRecord(BurnNote memory note) internal view returns (bool) {
        if (note.transferNote.outputCommitments.length < 2) {
            return false;
        }
        uint256 rc = _deriveRecordCommitment(note.recordOpening);
        return rc == note.transferNote.outputCommitments[1];
    }

    /// @dev Compute the commitment of a record opening.
    /// @param ro record opening
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

    /// @dev An overloaded function (one for each note type) to prepare all inputs necessary for batch verification of the plonk proof.
    /// @param note note of type *TRANSFER*
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
        // slither-disable-next-line calls-loop
        vk = VerifyingKeys.getVkById(
            VerifyingKeys.getEncodedId(
                uint8(NoteType.TRANSFER),
                uint8(note.inputNullifiers.length),
                uint8(note.outputCommitments.length),
                uint8(_recordsMerkleTree.getHeight())
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
        publicInput[1] = CAP_NATIVE_ASSET_CODE;
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

    /// @dev An overloaded function (one for each note type) to prepare all inputs necessary for batch verification of the plonk proof.
    /// @param note note of type *BURN*
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

    /// @dev An overloaded function (one for each note type) to prepare all inputs necessary for batch verification of the plonk proof.
    /// @param note note of type *MINT*
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
        // slither-disable-next-line calls-loop
        vk = VerifyingKeys.getVkById(
            VerifyingKeys.getEncodedId(
                uint8(NoteType.MINT),
                1, // num of input
                2, // num of output
                uint8(_recordsMerkleTree.getHeight())
            )
        );

        // prepare public inputs
        // 9: see below; 8: asset policy; rest: audit memo
        publicInput = new uint256[](9 + 8 + 2 + note.auditMemo.data.length);
        publicInput[0] = note.auxInfo.merkleRoot;
        publicInput[1] = CAP_NATIVE_ASSET_CODE;
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

    /// @dev An overloaded function (one for each note type) to prepare all inputs necessary for batch verification of the plonk proof.
    /// @param note note of type *FREEZE*
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
        // slither-disable-next-line calls-loop
        vk = VerifyingKeys.getVkById(
            VerifyingKeys.getEncodedId(
                uint8(NoteType.FREEZE),
                uint8(note.inputNullifiers.length),
                uint8(note.outputCommitments.length),
                uint8(_recordsMerkleTree.getHeight())
            )
        );

        // prepare public inputs
        publicInput = new uint256[](
            3 + note.inputNullifiers.length + note.outputCommitments.length
        );
        publicInput[0] = note.auxInfo.merkleRoot;
        publicInput[1] = CAP_NATIVE_ASSET_CODE;
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

    function getRootValue() external view returns (uint256) {
        return _recordsMerkleTree.getRootValue();
    }
}
