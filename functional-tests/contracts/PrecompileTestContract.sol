// SPDX-License-Identifier: Unlicense

pragma solidity ^0.8.0;

contract PrecompileTestContract {
    event GasUsed(string testName, uint256 gasUsed);

    function testPrecompiles() public {
        executePrecompile("testShaPrecompile", testShaPrecompile);
        executePrecompile("testEcRecoverPrecompile", testEcRecoverPrecompile);
        executePrecompile("testRIPEMD160Precompile", testRIPEMD160Precompile);
        executePrecompile("testIdentityPrecompile", testIdentityPrecompile);
        executePrecompile("testModExpPrecompile", testModExpPrecompile);
        executePrecompile("testEcAddPrecompile", testEcAddPrecompile);
        executePrecompile("testEcMulPrecompile", testEcMulPrecompile);
        executePrecompile("testEcPairingPrecompile", testEcPairingPrecompile);
    }

    function executePrecompile(
        string memory precompile,
        function() internal view f
    ) internal {
        uint256 gasBefore = gasleft();
        f();
        uint256 gasAfter = gasleft();
        uint256 gasUsed = gasBefore - gasAfter;
        emit GasUsed(precompile, gasUsed);
    }

    function testEcRecoverPrecompile() internal pure {
        // Message hash that was signed
        bytes32 messageHash = 0x456e9aea5e197a1f1af7a3e85a3212fa4049a3ba34c2289b4c860fc0b0c64ef3;

        // Recovery value + 27
        uint8 v = 28;

        // ECDSA signature components
        bytes32 r = 0x9242685bf161793cc25603c231bc2f568eb630ea16aa137d2664ac8038825608;
        bytes32 s = 0x4f8ae3bd7535248d0bd448298cc2e2071e56992d0774dc340c368ae950852ada;

        // Recover the signer's address using ecrecover
        address signer = ecrecover(messageHash, v, r, s);

        // Assert that the recovered signer is the expected signer
        address expectedSigner = 0x7156526fbD7a3C72969B54f64e42c10fbb768C8a;
        require(signer == expectedSigner, "Invalid signer");
    }

    function testShaPrecompile() internal pure {
        bytes memory message = hex"FF";
        bytes32 hash = sha256(message);
        bytes32 expectedHash = 0xa8100ae6aa1940d0b663bb31cd466142ebbdbd5187131b92d93818987832eb89;
        require(hash == expectedHash, "Invalid hash");
    }

    function testRIPEMD160Precompile() internal pure {
        bytes memory message = hex"FF";
        bytes20 hash = ripemd160(message);
        bytes20 expectedHash = bytes20(
            0x2c0C45D3ecab80fE060E5f1d7057cd2F8De5e557
        );
        require(hash == expectedHash, "Invalid hash");
    }

    function testIdentityPrecompile() internal view {
        bytes memory message = hex"FF";
        (bool success, bytes memory output) = address(0x04).staticcall(message);
        require(success, "Identity precompile failed");
        require(keccak256(output) == keccak256(message), "Invalid output");
    }

    function testModExpPrecompile() internal view {
        uint256 base = 8;
        uint256 exp = 9;
        uint256 mod = 10;
        uint256 expected = 8;

        bytes memory precompileData = abi.encode(32, 32, 32, base, exp, mod);
        (bool success, bytes memory output) = address(0x05).staticcall(
            precompileData
        );

        require(success, "ModExp precompile failed");
        require(abi.decode(output, (uint256)) == expected, "Invalid output");
    }

    function testEcAddPrecompile() internal view {
        uint256 x1 = 1;
        uint256 y1 = 2;
        uint256 x2 = 1;
        uint256 y2 = 2;
        bytes memory precompileData = abi.encode(x1, y1, x2, y2);

        bytes32 expectedX3 = 0x030644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd3;
        bytes32 expectedY3 = 0x15ed738c0e0a7c92e7845f96b2ae9c0a68a6a449e3538fc7ff3ebf7a5a18a2c4;

        (bool success, bytes memory output) = address(0x06).staticcall(
            precompileData
        );
        (bytes32 x3, bytes32 y3) = abi.decode(output, (bytes32, bytes32));

        require(success, "EcAdd precompile failed");
        require(x3 == expectedX3, "Invalid x3");
        require(y3 == expectedY3, "Invalid y3");
    }

    function testEcMulPrecompile() internal view {
        uint256 x1 = 1;
        uint256 y1 = 2;
        uint256 scalar = 2;
        bytes memory precompileData = abi.encode(x1, y1, scalar);
        (bool success, bytes memory output) = address(0x07).staticcall(
            precompileData
        );

        bytes32 expectedX3 = 0x030644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd3;
        bytes32 expectedY3 = 0x15ed738c0e0a7c92e7845f96b2ae9c0a68a6a449e3538fc7ff3ebf7a5a18a2c4;

        (bytes32 x3, bytes32 y3) = abi.decode(output, (bytes32, bytes32));

        require(success, "EcMul precompile failed");
        require(x3 == expectedX3, "Invalid x3");
        require(y3 == expectedY3, "Invalid y3");
    }

    function testEcPairingPrecompile() internal view {
        uint256 aG1_x = 0x2cf44499d5d27bb186308b7af7af02ac5bc9eeb6a3d147c186b21fb1b76e18da;
        uint256 aG1_y = 0x2c0f001f52110ccfe69108924926e45f0b0c868df0e7bde1fe16d3242dc715f6;

        uint256 bG2_x1 = 0x1fb19bb476f6b9e44e2a32234da8212f61cd63919354bc06aef31e3cfaff3ebc;
        uint256 bG2_x2 = 0x22606845ff186793914e03e21df544c34ffe2f2f3504de8a79d9159eca2d98d9;
        uint256 bG2_y1 = 0x2bd368e28381e8eccb5fa81fc26cf3f048eea9abfdd85d7ed3ab3698d63e4f90;
        uint256 bG2_y2 = 0x2fe02e47887507adf0ff1743cbac6ba291e66f59be6bd763950bb16041a0a85e;

        uint256 cG1_x = 0x0000000000000000000000000000000000000000000000000000000000000001;
        uint256 cG1_y = 0x30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd45;

        uint256 dG2_x1 = 0x1971ff0471b09fa93caaf13cbf443c1aede09cc4328f5a62aad45f40ec133eb4;
        uint256 dG2_x2 = 0x091058a3141822985733cbdddfed0fd8d6c104e9e9eff40bf5abfef9ab163bc7;
        uint256 dG2_y1 = 0x2a23af9a5ce2ba2796c1f4e453a370eb0af8c212d9dc9acd8fc02c2e907baea2;
        uint256 dG2_y2 = 0x23a8eb0b0996252cb548a4487da97b02422ebc0e834613f954de6c7e0afdc1fc;

        bytes memory precompileData = abi.encode(
            aG1_x,
            aG1_y,
            bG2_x1,
            bG2_x2,
            bG2_y1,
            bG2_y2,
            cG1_x,
            cG1_y,
            dG2_x1,
            dG2_x2,
            dG2_y1,
            dG2_y2
        );

        (bool success, bytes memory output) = address(0x08).staticcall(
            precompileData
        );
        require(success, "EcPairing precompile failed");
        bool isPairingValid = abi.decode(output, (bool));
        require(isPairingValid, "EC pairing check failed");
    }
}
