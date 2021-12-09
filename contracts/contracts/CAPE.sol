//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

/// @title Configurable Anonymous Payments on Ethereum
/// CAPE provides auditable anonymous payments on Ethereum.
/// @author Translucence Research, Inc.
/// @notice This is a notice.
/// @dev Developers are awesome!

import {Curve} from "./BN254.sol";

contract CAPE {
    mapping(uint256 => bool) private nullifiers;

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

    // Handling of nullifiers
    // Check if a nullifier has already been inserted
    function hasNullifierAlreadyBeenPublished(uint256 _nullifier)
        public
        view
        returns (bool)
    {
        return nullifiers[_nullifier];
    }

    /// Insert a nullifier into the set of nullifiers.
    function insertNullifier(uint256 _nullifier) internal {
        // This check is relied upon to prevent double spending of nullifiers
        // within the same note.
        require(!nullifiers[_nullifier], "Nullifier already published");
        nullifiers[_nullifier] = true;
    }

    /// Insert nullifiers into the set of nullifiers.
    function insertNullifiers(uint256[] memory _newNullifiers) internal {
        for (uint256 j = 0; j < _newNullifiers.length; j++) {
            insertNullifier(_newNullifiers[j]);
        }
    }

    function isSpendable(uint256 _nullifier) internal view returns (bool) {
        return !nullifiers[_nullifier];
    }

    function isSpendable(uint256[] memory _newNullifiers)
        internal
        view
        returns (bool)
    {
        for (uint256 j = 0; j < _newNullifiers.length; j++) {
            if (!isSpendable(_newNullifiers[j])) {
                return false;
            }
        }
        return true;
    }

    // @return true if the nullifiers were consumed (and note must be part of the block)
    function spendIfUnspent(uint256[] memory nullifiers)
        internal
        returns (bool)
    {
        if (isSpendable(nullifiers)) {
            insertNullifiers(nullifiers);
            return true;
        }
        return false;
    }

    // @return true if the nullifier was consumed (and note must be part of the block)
    function spendIfUnspent(uint256 nullifier) internal returns (bool) {
        if (isSpendable(nullifier)) {
            insertNullifier(nullifier);
            return true;
        }
        return false;
    }

    /// @notice Check if an asset is already registered
    /// @param erc20Address erc20 token address corresponding to the asset type.
    /// @param _newAsset asset type.
    /// @return true if the asset type is registered, false otherwise
    function isCapeAssetRegistered(
        address erc20Address,
        AssetDefinition memory _newAsset
    ) public returns (bool) {
        return true;
    }

    /// @notice create a new asset type associated to some erc20 token and register it in the contract so that it can be used later for wrapping.
    /// @param _erc20Address erc20 token address of corresponding to the asset type.
    /// @param _newAsset asset type to be registered in the contract.
    function sponsorCapeAsset(
        address _erc20Address,
        AssetDefinition memory _newAsset
    ) public {}

    /// @notice allows to wrap some erc20 tokens into some CAPE asset defined in the record opening
    /// @param _ro record opening that will be inserted in the records merkle tree once the deposit is validated.
    /// @param _erc20Address address of the ERC20 token corresponding to the deposit.
    function depositErc20(RecordOpening memory _ro, address _erc20Address)
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
        // Growable in memory arrays (or hashmaps) don't exists so we use a
        // boolean mask to mark the notes that are included.
        bool[] memory transferInclude = new bool[](
            newBlock.transferNotes.length
        );
        bool[] memory mintInclude = new bool[](newBlock.mintNotes.length);
        bool[] memory freezeInclude = new bool[](newBlock.freezeNotes.length);
        bool[] memory burnInclude = new bool[](newBlock.burnNotes.length);

        // Preserve the ordering of the (sub) arrays of notes.
        uint256 transferIdx = 0;
        uint256 mintIdx = 0;
        uint256 freezeIdx = 0;
        uint256 burnIdx = 0;

        for (uint256 i = 0; i < newBlock.noteTypes.length; i++) {
            NoteType noteType = newBlock.noteTypes[i];

            if (noteType == NoteType.TRANSFER) {
                uint256[] memory nullifiers = newBlock
                    .transferNotes[transferIdx]
                    .inputsNullifiers;
                bool ok = spendIfUnspent(nullifiers);
                transferInclude[transferIdx] = ok;
                transferIdx += 1;
            } else if (noteType == NoteType.MINT) {
                uint256 nullifier = newBlock.mintNotes[mintIdx].nullifier;
                bool ok = spendIfUnspent(nullifier);
                mintInclude[mintIdx] = ok;
                mintIdx += 1;
            } else if (noteType == NoteType.FREEZE) {
                uint256[] memory nullifiers = newBlock
                    .freezeNotes[freezeIdx]
                    .inputNullifiers;
                bool ok = spendIfUnspent(nullifiers);
                freezeInclude[freezeIdx] = ok;
                freezeIdx += 1;
            } else if (noteType == NoteType.BURN) {
                uint256[] memory nullifiers = newBlock
                    .burnNotes[burnIdx]
                    .transferNote
                    .inputsNullifiers;
                bool ok = spendIfUnspent(nullifiers);
                burnInclude[burnIdx] = ok;
                burnIdx += 1;
            }
        }

        // TODO apply transactions where ...Include mask is true
    }
}
