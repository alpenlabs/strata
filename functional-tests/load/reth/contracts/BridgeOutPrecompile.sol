// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

/**
 * @title BridgeOutCaller
 */
contract BridgeOutCaller {
    address constant PRECOMPILE_BRIDGEOUT_ADDRESS = 0x5400000000000000000000000000000000000001; // L2 precompile address
    uint256 constant BRIDGE_OUT_FIXED_VALUE = 10 ether; // Fixed amount for bridgeOutContractValue

    event Deposited(address indexed sender, uint256 amount);
    event BridgeOut(address indexed sender, uint256 amount, bytes bosd);

    receive() external payable {
        emit Deposited(msg.sender, msg.value); // Emits deposit event
    }

    /**
      * Top level transaction is expected to provide the sBTC
      */
    function bridgeOut(bytes calldata bosd) external payable {
        (bool success, ) = PRECOMPILE_BRIDGEOUT_ADDRESS.call{value: msg.value}(bosd); // Calls precompile with ETH
        require(success, "Precompile call failed");

        emit BridgeOut(msg.sender, msg.value, bosd);
    }

    /**
      * Contract will have sBTC to bridgeOut
      */
    function bridgeOutContractValue(bytes calldata bosd) external {
        require(address(this).balance >= BRIDGE_OUT_FIXED_VALUE, "Insufficient contract balance");

        (bool success, ) = PRECOMPILE_BRIDGEOUT_ADDRESS.call{value: BRIDGE_OUT_FIXED_VALUE}(bosd); // Calls precompile with fixed ETH
        require(success, "Precompile call failed");

        emit BridgeOut(msg.sender, BRIDGE_OUT_FIXED_VALUE, bosd);
    }

    /**
      * Contract will just call precompile. Always expected to fail
      */
    function bridgeOutContractNoValue(bytes calldata bosd) external {
        (bool success, ) = PRECOMPILE_BRIDGEOUT_ADDRESS.call(bosd); // Calls precompile without ETH
        require(success, "Precompile call failed");

        emit BridgeOut(msg.sender, 0, bosd);
    }

    function getBalance() external view returns (uint256) {
        return address(this).balance; // Returns contract balance
    }
}
