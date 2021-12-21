//SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

/// @title Configurable Anonymous Payments on Ethereum
/// CAPE provides auditable anonymous payments on Ethereum.
/// @author Translucence Research, Inc.
/// @notice This is a notice.
/// @dev Developers are awesome!

import "solidity-bytes-utils/contracts/BytesLib.sol";
import "./libraries/BN254.sol";
import "./libraries/RescueLib.sol";
import "./interfaces/IPlonkVerifier.sol";

// TODO Remove once functions are implemented
/* solhint-disable no-unused-vars */

contract CAPE {
    mapping(uint256 => bool) public nullifiers;

    bytes public constant CAPE_BURN_MAGIC_BYTES = "TRICAPE burn";

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
        /// Intenral asset code
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

    struct AssetDefinition {
        uint256 code;
        AssetPolicy policy;
    }

    struct AssetPolicy {
        EdOnBn254Point auditorPk;
        EdOnBn254Point credPk;
        EdOnBn254Point freezerPk;
        uint256 revealMap;
        uint64 revealThreshold;
    }

    struct RecordOpening {
        uint64 amount;
        AssetDefinition assetDef;
        EdOnBn254Point userAddr;
        bool freezeFlag;
        uint256 blind;
    }

    struct CapeBlock {
        EdOnBn254Point minerAddr;
        NoteType[] noteTypes;
        TransferNote[] transferNotes;
        MintNote[] mintNotes;
        FreezeNote[] freezeNotes;
        BurnNote[] burnNotes;
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

        for (uint256 i = 0; i < newBlock.noteTypes.length; i++) {
            NoteType noteType = newBlock.noteTypes[i];

            if (noteType == NoteType.TRANSFER) {
                TransferNote memory note = newBlock.transferNotes[transferIdx];
                _checkMerkleRootContained(note.auxInfo.merkleRoot);
                _checkTransfer(note);
                if (_publish(note.inputNullifiers)) {
                    // TODO collect note.outputCommitments
                    // TODO extract proof for batch verification
                }
                transferIdx += 1;
            } else if (noteType == NoteType.MINT) {
                MintNote memory note = newBlock.mintNotes[mintIdx];
                _checkMerkleRootContained(note.auxInfo.merkleRoot);
                if (_publish(note.inputNullifier)) {
                    // TODO collect note.mintComm
                    // TODO collect note.chgComm
                    // TODO extract proof for batch verification
                }
                mintIdx += 1;
            } else if (noteType == NoteType.FREEZE) {
                FreezeNote memory note = newBlock.freezeNotes[freezeIdx];
                _checkMerkleRootContained(note.auxInfo.merkleRoot);
                if (_publish(note.inputNullifiers)) {
                    // TODO collect note.outputCommitments
                    // TODO extract proof for batch verification
                }
                freezeIdx += 1;
            } else if (noteType == NoteType.BURN) {
                BurnNote memory note = newBlock.burnNotes[burnIdx];
                TransferNote memory transfer = note.transferNote;
                _checkMerkleRootContained(transfer.auxInfo.merkleRoot);

                _checkBurn(note);

                if (_publish(transfer.inputNullifiers)) {
                    // TODO collect transfer.outputCommitments
                    // TODO extract proof for batch verification
                }
                // TODO handle withdrawal (better done at end if call is external
                //      or have other reentrancy protection)
                burnIdx += 1;
            }
        }

        // TODO verify plonk proof
        // TODO batch insert record commitments
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

    function _checkTransfer(TransferNote memory note) internal pure {
        // TODO consider moving _checkMerkleRootContained into _check[NoteType] functions
        require(
            !_containsBurnPrefix(note.auxInfo.extraProofBoundData),
            "Burn prefix in transfer note"
        );
    }

    function _checkBurn(BurnNote memory note) internal view {
        bytes memory extra = note.transferNote.auxInfo.extraProofBoundData;
        require(_containsBurnPrefix(extra), "Bad burn tag");
        require(_containsBurnDestination(extra), "Bad burn destination");
        require(_containsBurnRecord(note), "Bad record commitment");
    }

    function _containsBurnPrefix(bytes memory extraProofBoundData)
        internal
        pure
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

    function _containsBurnRecord(BurnNote memory note)
        internal
        view
        returns (bool)
    {
        if (note.transferNote.outputCommitments.length < 2) {
            return false;
        }
        uint256 rc = _deriveRecordCommitment(note.recordOpening);
        return rc == note.transferNote.outputCommitments[1];
    }

    function _deriveRecordCommitment(RecordOpening memory ro)
        internal
        view
        returns (uint256 rc)
    {
        require(
            ro.assetDef.policy.revealMap < 2**12,
            "Reveal map exceeds 12 bits"
        );

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
}
