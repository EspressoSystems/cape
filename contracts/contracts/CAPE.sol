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

    struct AuditMemo {
        uint256 ephemeralKey;
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
        Curve.G1Point txnMemoVerKey;
        bytes extraProofBoundData;
    }

    struct MintAuxInfo {
        uint256 merkleRoot;
        uint64 fee;
        Curve.G1Point txnMemoVerKey;
    }

    struct FreezeAuxInfo {
        uint256 merkleRoot;
        uint64 fee;
        Curve.G1Point txnMemoVerKey;
    }

    struct UserPubKey {
        Curve.G1Point address_; // TODO Probably not the right type.
        Curve.G1Point encKey;
    }

    struct AssetDefinition {
        uint256 code;
        AssetPolicy policy;
    }

    struct AssetPolicy {
        uint64 revealThreshold;
        bool[12] revealMap; // ATTRS_LEN (8) + 3 + 1
        Curve.G1Point auditorPk;
        Curve.G1Point credPk;
        Curve.G1Point freezerPk;
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
    /// @notice Will revert if nullifier is already in nullifier set.
    function insertNullifier(uint256 _nullifier) internal {
        require(!nullifiers[_nullifier], "Nullifier already inserted");
        nullifiers[_nullifier] = true;
    }

    /// Insert nullifiers into the set of nullifiers.
    /// @notice Will revert if any nullifier is already in nullifier set.
    function insertNullifiers(uint256[] memory _newNullifiers) internal {
        for (uint256 j = 0; j < _newNullifiers.length; j++) {
            insertNullifier(_newNullifiers[j]);
        }
    }

    function validateAndApply(TransferNote memory _note) internal {}

    function validateAndApply(MintNote memory _note) internal {}

    function validateAndApply(FreezeNote memory _note) internal {}

    function validateAndApply(BurnNote memory _note) internal {}

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
        // Insert all the nullifiers (order does not matter)
        for (uint256 i = 0; i < newBlock.transferNotes.length; i++) {
            insertNullifiers(newBlock.transferNotes[i].inputsNullifiers);
        }
        for (uint256 i = 0; i < newBlock.mintNotes.length; i++) {
            insertNullifier(newBlock.mintNotes[i].nullifier);
        }
        for (uint256 i = 0; i < newBlock.freezeNotes.length; i++) {
            insertNullifiers(newBlock.freezeNotes[i].inputNullifiers);
        }
        for (uint256 i = 0; i < newBlock.burnNotes.length; i++) {
            // TODO it's hard to distinguish input{,s}Nullifiers here.
            insertNullifiers(
                newBlock.burnNotes[i].transferNote.inputsNullifiers
            );
        }

        // Verify transactions in the correct order given by noteTypes
        // and the ordering of the arrays of notes.
        uint256 transferNotesIndex = 0;
        uint256 mintNotesIndex = 0;
        uint256 freezeNotesIndex = 0;
        uint256 burnNotesIndex = 0;

        for (uint256 i = 0; i < newBlock.noteTypes.length; i++) {
            NoteType noteType = newBlock.noteTypes[i];

            if (noteType == NoteType.TRANSFER) {
                validateAndApply(newBlock.transferNotes[transferNotesIndex]);
                transferNotesIndex += 1;
            } else if (noteType == NoteType.MINT) {
                validateAndApply(newBlock.mintNotes[mintNotesIndex]);
                mintNotesIndex += 1;
            } else if (noteType == NoteType.FREEZE) {
                validateAndApply(newBlock.freezeNotes[freezeNotesIndex]);
                freezeNotesIndex += 1;
            } else if (noteType == NoteType.BURN) {
                validateAndApply(newBlock.burnNotes[burnNotesIndex]);
                burnNotesIndex += 1;
            }
        }
    }
}
