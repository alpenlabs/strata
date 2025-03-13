// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract BlockhashOpCode {
    bytes32 public lastBlockHash;

    function updateBlockHash() external {
        // Ensure that there is a previous block.
        require(block.number > 1, "No previous block available");
        // Retrieve the hash of the previous block (block.number - 1)
        lastBlockHash = blockhash(block.number - 1);
    }
}
