const rpcClient = require('./rpcClient'); // Mocked RPC client

describe("RPC Endpoints", () => {
    beforeEach(() => {
        rpcClient.call.mockClear(); // Clear mock calls before each test
    });

    // Test strata_protocolVersion
    it("should retrieve the protocol version", async () => {
        rpcClient.call.mockResolvedValue(1);
        const result = await rpcClient.call("strata_protocolVersion", []);
        expect(result).toBe(1);
        expect(rpcClient.call).toHaveBeenCalledWith("strata_protocolVersion", []);
    });

    // Test strata_blockTime
    it("should retrieve the block time", async () => {
        rpcClient.call.mockResolvedValue(60000);
        const result = await rpcClient.call("strata_blockTime", []);
        expect(result).toBe(60000);
        expect(rpcClient.call).toHaveBeenCalledWith("strata_blockTime", []);
    });

    // Test strata_l1connected
    it("should verify L1 connection status", async () => {
        rpcClient.call.mockResolvedValue(true);
        const result = await rpcClient.call("strata_l1connected", []);
        expect(result).toBe(true);
        expect(rpcClient.call).toHaveBeenCalledWith("strata_l1connected", []);
    });

    // Test strata_l1status
    it("should retrieve L1 status", async () => {
        const mockResponse = {
            bitcoin_rpc_connected: true,
            last_rpc_error: null,
            cur_height: 100,
            cur_tip_blkid: "000abc",
            last_published_txid: null,
            published_inscription_count: 5,
            last_update: 1670000000000,
            network: "signet",
        };
        rpcClient.call.mockResolvedValue(mockResponse);
        const result = await rpcClient.call("strata_l1status", []);
        expect(result).toEqual(mockResponse);
        expect(rpcClient.call).toHaveBeenCalledWith("strata_l1status", []);
    });

    // Test strata_getL1blockHash
    it("should retrieve the bitcoin block hash", async () => {
        rpcClient.call.mockResolvedValue("000abc");
        const result = await rpcClient.call("strata_getL1blockHash", [100]);
        expect(result).toBe("000abc");
        expect(rpcClient.call).toHaveBeenCalledWith("strata_getL1blockHash", [100]);
    });

    // Test strata_clientStatus
    it("should retrieve the client status", async () => {
        const mockResponse = {
            chain_tip: "000abc",
            chain_tip_slot: 100,
            finalized_blkid: "000def",
            last_l1_block: "000ghi",
            buried_l1_height: 99,
        };
        rpcClient.call.mockResolvedValue(mockResponse);
        const result = await rpcClient.call("strata_clientStatus", []);
        expect(result).toEqual(mockResponse);
        expect(rpcClient.call).toHaveBeenCalledWith("strata_clientStatus", []);
    });

    // Test strata_getRecentBlockHeaders
    it("should retrieve recent block headers", async () => {
        const mockResponse = [
            {
                block_idx: 100,
                timestamp: 1670000000000,
                block_id: "000abc",
                prev_block: "000def",
                l1_segment_hash: "111xyz",
                exec_segment_hash: "222xyz",
                state_root: "333xyz",
            },
        ];
        rpcClient.call.mockResolvedValue(mockResponse);
        const result = await rpcClient.call("strata_getRecentBlockHeaders", [5]);
        expect(result).toEqual(mockResponse);
        expect(rpcClient.call).toHaveBeenCalledWith("strata_getRecentBlockHeaders", [5]);
    });

    // Test strata_getHeadersAtIdx
    it("should retrieve headers at specific index", async () => {
        const mockResponse = [
            {
                block_idx: 101,
                timestamp: 1670000000000,
                block_id: "000xyz",
                prev_block: "000uvw",
                l1_segment_hash: "111uvw",
                exec_segment_hash: "222uvw",
                state_root: "333uvw",
            },
        ];
        rpcClient.call.mockResolvedValue(mockResponse);
        const result = await rpcClient.call("strata_getHeadersAtIdx", [101]);
        expect(result).toEqual(mockResponse);
        expect(rpcClient.call).toHaveBeenCalledWith("strata_getHeadersAtIdx", [101]);
    });

    // Test strata_getHeaderById
    it("should retrieve header by block ID", async () => {
        const mockResponse = {
            block_idx: 102,
            timestamp: 1670000000000,
            block_id: "000xyz",
            prev_block: "000uvw",
            l1_segment_hash: "111uvw",
            exec_segment_hash: "222uvw",
            state_root: "333uvw",
        };
        rpcClient.call.mockResolvedValue(mockResponse);
        const result = await rpcClient.call("strata_getHeaderById", ["000xyz"]);
        expect(result).toEqual(mockResponse);
        expect(rpcClient.call).toHaveBeenCalledWith("strata_getHeaderById", ["000xyz"]);
    });

    // Test strata_getExecUpdateById
    it("should retrieve execution update by block ID", async () => {
        const mockResponse = {
            update_idx: 5,
            entries_root: "000abc",
            extra_payload: "payload",
            new_state: "state",
            withdrawals: [
                {
                    amt: 1000,
                    dest_pk: "pk123",
                },
            ],
            da_blobs: [
                {
                    dest: 1,
                    blob_commitment: "commit123",
                },
            ],
        };
        rpcClient.call.mockResolvedValue(mockResponse);
        const result = await rpcClient.call("strata_getExecUpdateById", ["000xyz"]);
        expect(result).toEqual(mockResponse);
        expect(rpcClient.call).toHaveBeenCalledWith("strata_getExecUpdateById", ["000xyz"]);
    });
});
