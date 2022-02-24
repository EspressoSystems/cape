//SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "./libraries/BN254.sol";
import "./libraries/EdOnBN254.sol";

contract AssetRegistry {
    bytes13 public constant DOM_SEP_FOREIGN_ASSET = "FOREIGN_ASSET";
    bytes14 public constant DOM_SEP_DOMESTIC_ASSET = "DOMESTIC_ASSET";
    uint256 public constant CAP_NATIVE_ASSET_CODE = 1;

    mapping(bytes32 => address) public assets;

    struct AssetDefinition {
        uint256 code;
        AssetPolicy policy;
    }

    struct AssetPolicy {
        EdOnBN254.EdOnBN254Point auditorPk;
        EdOnBN254.EdOnBN254Point credPk;
        EdOnBN254.EdOnBN254Point freezerPk;
        uint256 revealMap;
        uint64 revealThreshold;
    }

    /// @notice Return the CAP-native asset definition
    function nativeDomesticAsset() public pure returns (AssetDefinition memory assetDefinition) {
        assetDefinition.code = CAP_NATIVE_ASSET_CODE;
        // affine representation of zero point in arkwork is (0,1)
        assetDefinition.policy.auditorPk.y = 1;
        assetDefinition.policy.credPk.y = 1;
        assetDefinition.policy.freezerPk.y = 1;
    }

    /// @notice Fetch the ERC-20 token address corresponding to the
    /// given asset definition.
    /// @param assetDefinition an asset definition
    /// @return An ERC-20 address
    function _lookup(AssetDefinition memory assetDefinition) internal view returns (address) {
        bytes32 key = keccak256(abi.encode(assetDefinition));
        return assets[key];
    }

    /// @notice Is the given asset definition registered?
    /// @param assetDefinition an asset definition
    /// @return True if the asset type is registered, false otherwise.
    function isCapeAssetRegistered(AssetDefinition memory assetDefinition)
        public
        view
        returns (bool)
    {
        return _lookup(assetDefinition) != address(0);
    }

    /// @notice Create and register a new asset type associated with an
    /// ERC-20 token. Will revert if the asset type is already
    /// registered or the ERC-20 token address is zero.
    /// @param erc20Address An ERC-20 token address
    /// @param newAsset An asset type to be registered in the contract
    function sponsorCapeAsset(address erc20Address, AssetDefinition memory newAsset) public {
        require(erc20Address != address(0), "Bad asset address");
        require(!isCapeAssetRegistered(newAsset), "Asset already registered");

        _checkForeignAssetCode(newAsset.code, erc20Address, msg.sender);

        bytes32 key = keccak256(abi.encode(newAsset));
        assets[key] = erc20Address;
    }

    /// @notice Throws an exception if the asset definition code is
    /// not correctly derived from the ERC-20 address of the token and
    /// the address of the sponsor.
    /// @dev Requires "view" to access msg.sender.
    /// @param assetDefinitionCode The code of an asset definition
    /// @param erc20Address The ERC-20 address bound to the asset definition
    /// @param sponsor The sponsor address of this wrapped asset
    function _checkForeignAssetCode(
        uint256 assetDefinitionCode,
        address erc20Address,
        address sponsor
    ) internal pure {
        bytes memory description = _computeAssetDescription(erc20Address, sponsor);
        require(
            assetDefinitionCode ==
                BN254.fromLeBytesModOrder(
                    bytes.concat(keccak256(bytes.concat(DOM_SEP_FOREIGN_ASSET, description)))
                ),
            "Wrong foreign asset code"
        );
    }

    /// @dev Checks if the asset definition code is correctly derived from the internal asset code.
    /// @param assetDefinitionCode asset definition code
    /// @param internalAssetCode internal asset code
    function _checkDomesticAssetCode(uint256 assetDefinitionCode, uint256 internalAssetCode)
        internal
        pure
    {
        require(
            assetDefinitionCode ==
                BN254.fromLeBytesModOrder(
                    bytes.concat(
                        keccak256(
                            bytes.concat(
                                DOM_SEP_DOMESTIC_ASSET,
                                bytes32(Utils.reverseEndianness(internalAssetCode))
                            )
                        )
                    )
                ),
            "Wrong domestic asset code"
        );
    }

    /// @dev Compute the asset description from the address of the
    /// ERC-20 token and the address of the sponsor.
    /// @param erc20Address address of the erc20 token
    /// @param sponsor address of the sponsor
    /// @return The asset description
    function _computeAssetDescription(address erc20Address, address sponsor)
        internal
        pure
        returns (bytes memory)
    {
        return
            bytes.concat("TRICAPE ERC20", bytes20(erc20Address), "sponsored by", bytes20(sponsor));
    }
}
