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

    struct MintValidityProof {
        // TODO
        uint256 dummy;
    }

    // Group Projective
    struct EncKey {
        uint256 x;
        uint256 y;
        uint256 t;
        uint256 z;
    }

    // struct EncKey {
    //     GroupProjective key;
    // }

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
        TransferValidityProof proof;
        AuditMemo auditMemo;
        AuxInfo auxInfo;
        uint256[] inputsNullifiers;
        uint256[] outputCommitments;
    }

    struct AuxInfo {
        uint256 merkleRoot;
        uint256 fee;
        uint256 validUntil;
        EncKey txnMemoVerKey;
    }

    struct UserPubKey {
        EncKey address_; // TODO Probably not the right type.
        EncKey encKey;
    }

    struct FreezeNote {
        bool field;
        // TODO
    }

    struct CapeTransaction {
        /// DOC COMMENT IGNORED. Documentation for the field named field.
        // For now we only represent the list of nullifiers of a transactions
        uint256[] inputsNullifiers; // (works)
        // TransferNote note;
    }

    struct AssetDefinition {
        uint256 code;
        AssetPolicy policy;
    }

    // NOTE: abigen! seems sensitive to order of fields
    struct AssetPolicy {
        uint64 revealThreshold;
        bool[12] revealMap; // ATTRS_LEN (8) + 3 + 1
        EncKey auditorPk;
        EncKey credPk;
        EncKey freezerPk;
    }

    struct RecordOpening {
        bool field;
        // TODO (Philippe will take care of it)
    }

    struct CapeBlock {
        UserPubKey miner; // TODO
        uint64 blockHeight; // TODO
        CapeTransaction[] txns;
        CapeTransaction[] burnTxns; // TODO
    }

    /// @notice Validate a transaction and if successful apply it.
    /// @dev This is the developer doc for validateAndApply.
    /// @param _block is an array of transactions
    function validateAndApply(CapeTransaction[] calldata _block) internal {}

    /// Process a transaction in the standard way (not a burn?)
    /// @param _transaction is a CapeTransaction
    function processStandardTransaction(CapeTransaction memory _transaction)
        internal
    {}

    /// Process a burn transaction.
    /// @param _transaction is a CapeTransaction
    function processBurnTransaction(CapeTransaction memory _transaction)
        internal
    {}

    /// @notice Check if an asset is already registered
    /// @param erc20Address erc20 token address corresponding to the asset type.
    /// @param newAsset asset type.
    /// @return true if the asset type is registered, false otherwise
    // function isCapeAssetRegistered(
    //     address erc20Address,
    //     AssetDefinition memory newAsset
    // ) public returns (bool) {
    //     return true;
    // }

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
        CapeBlock memory newBlock, // TODO use block struct
        uint256[] memory mtFrontier,
        RecordOpening[] memory burnedRos
    ) public {
        // Go through the nullifiers list of each transaction and do the insertion into the Nullifier Store
        for (uint256 i = 0; i < newBlock.txns.length; i++) {
            uint256[] memory nullifiers = newBlock.txns[i].inputsNullifiers;
            // for (uint256 i = 0; i < newBlock.length; i++) {
            //     uint256[] memory nullifiers = newBlock[i].nullifiers;
            for (uint256 j = 0; j < nullifiers.length; j++) {
                insertNullifier(nullifiers[j]);
            }
        }
    }
}
