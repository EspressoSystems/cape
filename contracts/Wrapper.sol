//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

contract Wrapper {
    struct Policy {
        bool field; //TODO
    }

    struct AssetType {
        bool field; //TODO
    }

    struct AssetRecord {
        bool field; // TODO
    }

    function creditERC20Balance(
        address _erc20Token,
        uint256 _amount,
        address _recipient
    ) private {}

    function genAssetTypeERC20(
        address _erc20Token,
        string calldata _sponsor,
        Policy calldata _policy
    ) public {}

    function withdraw(
        address _recipient,
        address _erc20Token,
        uint256 _amount
    ) public {}

    function wrapERC20Token(
        AssetType memory _assetType,
        AssetRecord memory _record,
        address _sender
    ) public {}
}
