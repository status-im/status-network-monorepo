// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { Script, console } from "forge-std/Script.sol";

import { KarmaBatchTransfer } from "../src/KarmaBatchTransfer.sol";

/**
 * @dev This script executes batch Karma transfers from a deployed KarmaBatchTransfer contract.
 *
 * Required environment variables:
 *   - KARMA_BATCH_TRANSFER_ADDRESS: address of the deployed KarmaBatchTransfer contract
 *   - BATCH_TRANSFER_JSON: path to a JSON file containing the transfer list
 *
 * Optional environment variables:
 *   - BATCH_SIZE: max recipients per transaction (default: 200)
 *
 * JSON file format:
 * [
 *   { "recipient": "0x...", "amount": "1000000000000000000" },
 *   ...
 * ]
 *
 * Prerequisites (Karma admin operations):
 *   1. karma.setAllowedToTransfer(address(batchTransfer), true)
 *   2. karma.mint(address(batchTransfer), totalAmount)
 *
 * Usage:
 *   forge script script/ExecuteKarmaBatchTransfer.s.sol --rpc-url $RPC_URL --broadcast
 */
contract ExecuteKarmaBatchTransferScript is Script {
    struct TransferEntry {
        uint256 amount;
        address recipient;
    }

    function run() public {
        address batchTransferAddress = vm.envAddress("KARMA_BATCH_TRANSFER_ADDRESS");
        require(batchTransferAddress != address(0), "KARMA_BATCH_TRANSFER_ADDRESS is not set");

        string memory jsonPath = vm.envString("BATCH_TRANSFER_JSON");
        uint256 batchSize = vm.envOr("BATCH_SIZE", uint256(200));

        KarmaBatchTransfer batchTransfer = KarmaBatchTransfer(batchTransferAddress);

        string memory json = vm.readFile(jsonPath);
        bytes memory rawJson = vm.parseJson(json);
        TransferEntry[] memory entries = abi.decode(rawJson, (TransferEntry[]));

        uint256 total = entries.length;
        uint256 numBatches = (total + batchSize - 1) / batchSize;

        console.log("Total transfers:", total);
        console.log("Batch size:", batchSize);
        console.log("Number of batches:", numBatches);

        for (uint256 b = 0; b < numBatches; b++) {
            uint256 start = b * batchSize;
            uint256 end = start + batchSize;
            if (end > total) end = total;
            uint256 count = end - start;

            address[] memory recipients = new address[](count);
            uint256[] memory amounts = new uint256[](count);

            for (uint256 i = 0; i < count; i++) {
                recipients[i] = entries[start + i].recipient;
                amounts[i] = entries[start + i].amount;
            }

            console.log("Executing batch", b + 1, "of", numBatches);
            console.log("  Recipients in batch:", count);

            vm.startBroadcast();
            batchTransfer.batchTransfer(recipients, amounts);
            vm.stopBroadcast();
        }

        console.log("All batches complete.");
    }
}
