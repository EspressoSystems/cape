//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import "./NullifiersStore.sol";

/// @title Configurable Anonymous Payments on Ethereum
/// CAPE provides auditable anonymous payments on Ethereum.
/// @author Translucence Research, Inc.
/// @notice This is a notice.
/// @dev Developers are awesome!

contract CAPE is NullifiersStore {
    struct TransferValidityProof {
        // TODO
        uint256 dummy;
    }

    // struct MintValidityProof {
    //     // TODO
    //     uint256 dummy;
    // }

    struct GroupProjective {
        uint256 x;
        uint256 y;
        uint256 t;
        uint256 z;
    }

    struct EncKey {
        GroupProjective key;
    }

    struct AuditMemo {
        // is Ciphertext
        EncKey ephemeral;
        uint256[] data;
    }

    // XXX This wrapper around the solidity array type is to workaround
    // an issue that causes the ethers abigen to fail on nested structs.
    //     https://github.com/gakonst/ethers-rs/issues/538
    //
    // Note: doesn't really work as workaround anymore
    // struct SolidityArray {
    //     uint256[] items;
    // }

    struct TransferNote {
        uint256[] inputNullifiers;
        uint256[] outputCommitments;
        TransferValidityProof proof;
        AuditMemo auditMemo;
        AuxInfo auxInfo;
    }

    struct AuxInfo {
        uint256 merkleRoot;
        uint256 fee;
        uint256 validUntil;
        GroupProjective txnMemoVerKey;
    }

    // struct MintNote {
    //     /// nullifier for the input (i.e. transaction fee record)
    //     uint256 nullifier;
    //     /// output commitment for the fee change
    //     uint256 chgComm;
    //     /// output commitment for the minted asset
    //     uint256 mintComm;
    //     /// the amount of the minted asset
    //     uint64 mintAmount; // TODO change to uint128?
    //     /// the asset definition of the asset
    //     AssetDefinition mintAssedDef;
    //     /// the validity proof of this note
    //     MintValidityProof proof;
    //     /// memo for policy compliance specified for the designated auditor
    //     AuditMemo auditMemo;
    //     /// auxiliary information
    //     MintAuxInfo aux_info;
    // }

    // struct MintAuxInfo {
    //     uint256 merkleRoot;
    //     uint64 fee;
    //     GroupProjective txnMemoVerKey;
    // }

    struct UserPubKey {
        GroupProjective address_; // TODO Probably not the right type.
        GroupProjective encKey;
    }

    struct FreezeNote {
        bool field;
        // TODO
    }

    struct CAPETransaction {
        /// DOC COMMENT IGNORED. Documentation for the field named field.
        // For now we only represent the list of nullifiers of a transactions
        uint256[] nullifiers;
    }

    struct AssetDefinition {
        uint256 code;
        AssetPolicy policy;
    }

    struct AssetPolicy {
        EncKey auditorPk;
        EncKey credPk;
        EncKey freezerPk;
        bool[12] revealMap; // ATTRS_LEN (8) + 3 + 1
        uint64 revealThreshold;
    }

    struct RecordOpening {
        bool field;
        // TODO (Philippe will take care of it)
    }

    struct CapeBlock {
        CAPETransaction[] txns;
        // CAPETransaction[] burnTxns; // TODO
        //UserPubKey miner; // TODO
        // uint64 blockHeight; // TODO
    }

    /// @notice Validate a transaction and if successful apply it.
    /// @dev This is the developer doc for validateAndApply.
    /// @param _block is an array of transactions
    function validateAndApply(CAPETransaction[] calldata _block) internal {}

    /// Process a transaction in the standard way (not a burn?)
    /// @param _transaction is a CAPETransaction
    function processStandardTransaction(CAPETransaction memory _transaction)
        internal
    {}

    /// Process a burn transaction.
    /// @param _transaction is a CAPETransaction
    function processBurnTransaction(CAPETransaction memory _transaction)
        internal
    {}

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
    /// @param mtFrontier latest frontier of the records merkle tree.
    // /// @param burnedRos record opening of the second outputs of the burn transactions. The information contained in these records opening allow the contract to transfer the erc20 tokens.
    function submitCapeBlock(
        CapeBlock memory newBlock,
        uint256[] memory mtFrontier
    )
        public
    // RecordOpening[] memory burnedRos // TODO part of the unwrapping logic
    {

    }
}
