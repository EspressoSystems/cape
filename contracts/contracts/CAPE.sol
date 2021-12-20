//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

/// @title Configurable Anonymous Payments on Ethereum
/// CAPE provides auditable anonymous payments on Ethereum.
/// @author Translucence Research, Inc.
/// @notice This is a notice.
/// @dev Developers are awesome!

import "hardhat/console.sol";
import "solidity-bytes-utils/contracts/BytesLib.sol";
import "./RecordsMerkleTree.sol";

// TODO Remove once functions are implemented
/* solhint-disable no-unused-vars */

contract CAPE is RecordsMerkleTree {
    mapping(uint256 => bool) public nullifiers;

    // The last N roots are managed with a circular buffer
    uint256[] internal _roots;
    uint64 internal _nRoots;
    uint64 internal _rootsWriteIndex;

    bytes public constant CAPE_BURN_MAGIC_BYTES = "TRICAPE burn";

    struct PlonkProof {
        // TODO
        uint256 dummy;
    }

    struct EdOnBn254Point {
        uint256 x;
        uint256 y;
    }

    struct AuditMemo {
        EdOnBn254Point ephemeralKey;
        uint256[] data;
    }

    enum NoteType {
        TRANSFER,
        MINT,
        FREEZE,
        BURN
    }

    struct TransferNote {
        uint256[] inputsNullifiers;
        uint256[] outputCommitments;
        PlonkProof proof;
        AuditMemo auditMemo;
        AuxInfo auxInfo;
    }

    struct BurnNote {
        TransferNote transferNote;
        RecordOpening recordOpening;
    }

    struct MintNote {
        /// nullifier for the input (i.e. transaction fee record)
        uint256 nullifier;
        /// output commitment for the fee change
        uint256 chgComm;
        /// output commitment for the minted asset
        uint256 mintComm;
        /// the amount of the minted asset
        uint64 mintAmount;
        /// the asset definition of the asset
        AssetDefinition mintAssedDef;
        /// the validity proof of this note
        PlonkProof proof;
        /// memo for policy compliance specified for the designated auditor
        AuditMemo auditMemo;
        /// auxiliary information
        MintAuxInfo auxInfo;
    }

    struct FreezeNote {
        uint256[] inputNullifiers;
        uint256[] outputCommitments;
        PlonkProof proof;
        FreezeAuxInfo auxInfo;
    }

    struct AuxInfo {
        uint256 merkleRoot;
        uint64 fee;
        uint64 validUntil;
        EdOnBn254Point txnMemoVerKey;
        bytes extraProofBoundData;
    }

    struct MintAuxInfo {
        uint256 merkleRoot;
        uint64 fee;
        EdOnBn254Point txnMemoVerKey;
    }

    struct FreezeAuxInfo {
        uint256 merkleRoot;
        uint64 fee;
        EdOnBn254Point txnMemoVerKey;
    }

    struct UserPubKey {
        EdOnBn254Point address_; // "address" is a keyword in solidity
        EdOnBn254Point encKey;
    }

    struct AssetDefinition {
        uint256 code;
        AssetPolicy policy;
    }

    struct AssetPolicy {
        uint64 revealThreshold;
        bool[12] revealMap; // ATTRS_LEN (8) + 3 + 1
        EdOnBn254Point auditorPk;
        EdOnBn254Point credPk;
        EdOnBn254Point freezerPk;
    }

    struct RecordOpening {
        bool field;
        // TODO (Philippe will take care of it)
    }

    struct CapeBlock {
        UserPubKey miner; // TODO
        uint64 blockHeight; // TODO
        NoteType[] noteTypes;
        TransferNote[] transferNotes;
        MintNote[] mintNotes;
        FreezeNote[] freezeNotes;
        BurnNote[] burnNotes; // TODO
    }

    constructor(uint8 height, uint64 nRoots) RecordsMerkleTree(height) {
        // Set up the circular buffer for handling the last N roots
        _roots = new uint256[](nRoots);
        _nRoots = nRoots;

        require(_nRoots > 1, "A least 2 roots required");
        _rootsWriteIndex = 1; // The first root value is 0 when the tree is empty
    }

    /// Update the list of roots
    function _updateRootsList(uint256 lastRoot) private {
        _roots[_rootsWriteIndex] = lastRoot;
        _rootsWriteIndex = (_rootsWriteIndex + 1) % _nRoots;
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
    function _containsPublished(uint256[] memory newNullifiers)
        internal
        view
        returns (bool)
    {
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

    /// Trim the array of records commitment from the 0 values at the end
    function _trimArrayRightZeroes(uint256[] memory input)
        private
        returns (uint256[] memory)
    {
        // Compute new size of array after trimming
        uint256 size = 0;
        for (uint256 i = 0; i < input.length; i++) {
            if (input[i] != 0) {
                size += 1;
            }
        }
        // Allocate memory for the output array
        uint256[] memory output = new uint256[](size);

        // Copy values into new array
        for (uint256 i = 0; i < size; i++) {
            output[i] = input[i];
        }

        return output;
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

    /// @notice Check if an asset is already registered
    /// @param erc20Address erc20 token address corresponding to the asset type.
    /// @param newAsset asset type.
    /// @return true if the asset type is registered, false otherwise
    function isCapeAssetRegistered(
        address erc20Address,
        AssetDefinition memory newAsset
    ) public returns (bool) {
        return true;
    }

    /// @notice create a new asset type associated to some erc20 token and register it in the contract so that it can be used later for wrapping.
    /// @param erc20Address erc20 token address of corresponding to the asset type.
    /// @param newAsset asset type to be registered in the contract.
    function sponsorCapeAsset(
        address erc20Address,
        AssetDefinition memory newAsset
    ) public {}

    /// @notice allows to wrap some erc20 tokens into some CAPE asset defined in the record opening
    /// @param ro record opening that will be inserted in the records merkle tree once the deposit is validated.
    /// @param erc20Address address of the ERC20 token corresponding to the deposit.
    function depositErc20(RecordOpening memory ro, address erc20Address)
        public
    {
        address depositorAddress = msg.sender;
    }

    /// @notice submit a new block to the CAPE contract. Transactions are validated and the blockchain state is updated. Moreover burn transactions trigger the unwrapping of cape asset records into erc20 tokens.
    /// @param newBlock block to be processed by the CAPE contract.
    /// @param burnedRos record opening of the second outputs of the burn transactions. The information contained in these records opening allow the contract to transfer the erc20 tokens.
    function submitCapeBlock(
        CapeBlock memory newBlock,
        RecordOpening[] memory burnedRos
    ) public {
        // TODO check block height

        // Preserve the ordering of the (sub) arrays of notes.
        uint256 transferIdx = 0;
        uint256 mintIdx = 0;
        uint256 freezeIdx = 0;
        uint256 burnIdx = 0;

        // Compute an upper bound on the number of records to be inserted
        // TODO should let the relayer submit this value?
        //     If it's too low, tx will revert, if too high some wasted gas.
        uint256 maxComms = 2 * newBlock.mintNotes.length;
        for (uint256 i = 0; i < newBlock.transferNotes.length; i++) {
            maxComms += newBlock.transferNotes[i].outputCommitments.length;
        }
        for (uint256 i = 0; i < newBlock.burnNotes.length; i++) {
            maxComms += newBlock
                .burnNotes[i]
                .transferNote
                .outputCommitments // TODO is this always of same length?
                .length;
        }
        for (uint256 i = 0; i < newBlock.freezeNotes.length; i++) {
            maxComms += newBlock
                .freezeNotes[i]
                .outputCommitments // TODO is this always of same length?
                .length;
        }

        uint256[] memory comms = new uint256[](maxComms);
        uint256 commsIdx = 0;

        for (uint256 i = 0; i < newBlock.noteTypes.length; i++) {
            NoteType noteType = newBlock.noteTypes[i];

            if (noteType == NoteType.TRANSFER) {
                TransferNote memory note = newBlock.transferNotes[transferIdx];
                _checkMerkleRootContained(note.auxInfo.merkleRoot);

                if (_publish(note.inputsNullifiers)) {
                    commsIdx = _appendCommitments(
                        comms,
                        note.outputCommitments,
                        commsIdx
                    );

                    // TODO extract proof for batch verification
                }
                transferIdx += 1;
            } else if (noteType == NoteType.MINT) {
                MintNote memory note = newBlock.mintNotes[mintIdx];
                _checkMerkleRootContained(note.auxInfo.merkleRoot);

                if (_publish(note.nullifier)) {
                    comms[commsIdx] = note.mintComm;
                    comms[commsIdx] = note.chgComm;
                    commsIdx += 2;
                    // TODO extract proof for batch verification
                }

                mintIdx += 1;
            } else if (noteType == NoteType.FREEZE) {
                FreezeNote memory note = newBlock.freezeNotes[freezeIdx];
                _checkMerkleRootContained(note.auxInfo.merkleRoot);

                if (_publish(note.inputNullifiers)) {
                    commsIdx = _appendCommitments(
                        comms,
                        note.outputCommitments,
                        commsIdx
                    );

                    // TODO extract proof for batch verification
                }

                freezeIdx += 1;
            } else if (noteType == NoteType.BURN) {
                BurnNote memory note = newBlock.burnNotes[burnIdx];
                TransferNote memory transfer = note.transferNote;
                _checkMerkleRootContained(transfer.auxInfo.merkleRoot);

                _checkBurn(transfer.auxInfo.extraProofBoundData);
                // TODO check burn record opening matches second output commitment

                if (_publish(transfer.inputsNullifiers)) {
                    // TODO collect transfer.outputCommitments
                    // TODO do we need a special logic for how to handle outputs record commitments with BURN notes
                    commsIdx = _appendCommitments(
                        comms,
                        note.transferNote.outputCommitments,
                        commsIdx
                    );
                    // TODO extract proof for batch verification
                }
                // TODO handle withdrawal (better done at end if call is external
                //      or have other reentrancy protection)
                burnIdx += 1;
            }
        }

        // TODO verify plonk proof

        // Batch insert record commitments

        console.log("comms.length: %s", comms.length);

        // Not all transactions are necessary valid so it is needed to remove all the zero values of the array
        // (which should be contiguous to the right)
        // Note: we assume that a record commitment cannot have value 0.
        uint256[] memory commsTrimmed = _trimArrayRightZeroes(comms);
        console.log("commsTrimmed.length: %s", commsTrimmed.length);

        // Check that this is correct
        _updateRecordsMerkleTree(commsTrimmed);

        // Update the list of roots
        _updateRootsList(_rootValue);
    }

    /// @dev Appends `newComms` to `comms` starting at `start`
    /// @return index where next commitments should be inserted (the value of `start` for next invocation)
    function _appendCommitments(
        uint256[] memory comms,
        uint256[] memory newComms,
        uint256 start
    ) internal pure returns (uint256) {
        for (uint256 i = 0; i < newComms.length; i++) {
            comms[start + i] = newComms[i];
        }
        return start + newComms.length;
    }

    function _checkMerkleRootContained(uint256 root) internal view {
        // TODO revert if not contained
    }

    function _handleWithdrawal() internal {
        // TODO
    }

    function _batchInsertRecordCommitments(uint256[] memory commitments)
        internal
    {
        // TODO
    }

    function _checkBurn(bytes memory extraProofBoundData) internal {
        require(_hasBurnPrefix(extraProofBoundData), "Burn note not tagged");
        require(
            _hasBurnDestination(extraProofBoundData),
            "Burn destination wrong"
        );
    }

    function _hasBurnPrefix(bytes memory extraProofBoundData)
        internal
        returns (bool)
    {
        if (extraProofBoundData.length < 12) {
            return false;
        }
        return
            BytesLib.equal(
                BytesLib.slice(extraProofBoundData, 0, 12),
                CAPE_BURN_MAGIC_BYTES
            );
    }

    function _hasBurnDestination(bytes memory extraProofBoundData)
        internal
        returns (bool)
    {
        if (extraProofBoundData.length < 32) {
            return false;
        }
        return BytesLib.toAddress(extraProofBoundData, 12) == address(0);
    }
}
