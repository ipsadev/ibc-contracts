// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.28;

// solhint-disable custom-errors,gas-custom-errors,gas-small-strings

import { Test } from "forge-std/Test.sol";
import { SP1Verifier as SP1VerifierGroth16 } from "@sp1-contracts/v6.1.0/SP1VerifierGroth16.sol";

import { DeploySP1ICSStellar } from "../../scripts/DeploySP1ICSStellar.s.sol";
import { SP1ICSStellar } from "../../contracts/light-clients/sp1-stellar/SP1ICSStellar.sol";
import { ICS26Router } from "../../contracts/ICS26Router.sol";
import { AccessManager } from "@openzeppelin-contracts/access/manager/AccessManager.sol";
import { ERC1967Proxy } from "@openzeppelin-contracts/proxy/ERC1967/ERC1967Proxy.sol";
import { IICSStellarMsgs } from "../../contracts/light-clients/sp1-stellar/msgs/IICSStellarMsgs.sol";

/// @title Deploy script tests
/// @notice Exercises the genesis config the Rust side generates, against a real SP1 verifier.
/// @dev The config under scripts/deployments/stellar/ ships with its verifier and role manager
/// deliberately unset, so every case here starts from that file and fills them in. That is also
/// what keeps this honest: if the generator's field names or encodings drift from what the script
/// reads, these fail rather than the first real deployment.
contract DeploySP1ICSStellarTest is Test {
    string public constant SOURCE = "scripts/deployments/stellar/genesis.json";
    string public constant SCRATCH_DIR = "test/sp1-stellar/fixtures/tmp/";

    DeploySP1ICSStellar public script;
    SP1VerifierGroth16 public verifier;
    address public router;

    function setUp() public {
        script = new DeploySP1ICSStellar();
        verifier = new SP1VerifierGroth16();

        AccessManager accessManager = new AccessManager(address(this));

        router = address(
            new ERC1967Proxy(
                address(new ICS26Router()), abi.encodeCall(ICS26Router.initialize, (address(accessManager)))
            )
        );

        vm.createDir(SCRATCH_DIR, true);
    }

    /// @dev Each case gets its own file and passes it to the script explicitly. A shared scratch
    /// file, or the STELLAR_GENESIS environment variable, is global to the process: one case's
    /// mutation then reaches every case running alongside it, and the failures that produces look
    /// like bugs in the script rather than in the test.
    function _config(
        string memory name,
        address verifierAddress,
        address roleManager
    )
        internal
        returns (string memory path)
    {
        path = string.concat(SCRATCH_DIR, name, ".json");

        vm.writeFile(path, vm.readFile(SOURCE));
        vm.writeJson(vm.toString(verifierAddress), path, ".verifier");
        vm.writeJson(vm.toString(roleManager), path, ".roleManager");
    }

    function test_TheGeneratedGenesisDeploysAgainstARealVerifier() public {
        string memory path = _config("deploys", address(verifier), router);

        SP1ICSStellar client = script.deploy(path);

        assertEq(address(client.VERIFIER()), address(verifier), "the pinned verifier is the one deployed");

        IICSStellarMsgs.ClientState memory state = abi.decode(client.getClientState(), (IICSStellarMsgs.ClientState));

        assertEq(state.rootEventTopic, bytes("ibc_root"), "the topic survives the config round trip");
        assertEq(
            state.latestHeight.revisionHeight,
            vm.parseJsonUint(vm.readFile(path), ".revisionHeight"),
            "the genesis ledger is the trusted one"
        );
        assertFalse(state.isFrozen, "a fresh client is not frozen");
        assertTrue(
            client.hasRole(client.PROOF_SUBMITTER_ROLE(), router), "the router can submit the proofs recvPacket carries"
        );
    }

    /// @dev The consensus state hash is what every later proof is checked against, so the value the
    /// script stores has to be the hash of the consensus state in the config, not of anything else.
    function test_TheStoredConsensusStateIsTheOneInTheConfig() public {
        string memory path = _config("stored-consensus", address(verifier), router);

        SP1ICSStellar client = script.deploy(path);

        string memory json = vm.readFile(path);

        IICSStellarMsgs.ConsensusState memory expected = IICSStellarMsgs.ConsensusState({
            closeTime: uint64(vm.parseJsonUint(json, ".closeTime")),
            ledgerHash: vm.parseJsonBytes32(json, ".ledgerHash"),
            previousLedgerHash: vm.parseJsonBytes32(json, ".previousLedgerHash"),
            bucketListHash: vm.parseJsonBytes32(json, ".bucketListHash"),
            transactionSetResultHash: vm.parseJsonBytes32(json, ".transactionSetResultHash")
        });

        assertEq(
            client.getConsensusStateHash(uint64(vm.parseJsonUint(json, ".revisionHeight"))),
            keccak256(abi.encode(expected)),
            "the stored hash is the hash of the config's consensus state"
        );
    }

    function test_AnUnsetVerifierIsRefused() public {
        string memory path = _config("unset-verifier", address(0), router);

        vm.expectRevert(bytes("verifier is unset: fill in .verifier in the genesis config"));
        script.deploy(path);
    }

    /// @dev The likeliest real mistake is a gateway address that is correct for another chain, where
    /// it holds no code. Deploying against it would produce a client whose every proof reverts.
    function test_AVerifierWithNoCodeIsRefused() public {
        string memory path = _config("no-code-verifier", address(0xBEEF), router);

        vm.expectRevert(bytes("no contract at .verifier on this chain: check the SP1 verifier gateway address"));
        script.deploy(path);
    }

    function test_AnUnsetRoleManagerIsRefused() public {
        string memory path = _config("unset-role-manager", address(verifier), address(0));

        vm.expectRevert(bytes("roleManager is unset: fill in .roleManager in the genesis config"));
        script.deploy(path);
    }

    function test_AnAccountAsRoleManagerIsRefused() public {
        string memory path = _config("account-role-manager", address(verifier), makeAddr("deployer"));

        vm.expectRevert(
            bytes("roleManager has no code: it must be the ICS26Router proxy, which is what calls verifyMembership")
        );
        script.deploy(path);
    }

    function test_AContractThatIsNotARouterIsRefusedAsRoleManager() public {
        string memory path = _config("not-a-router-role-manager", address(verifier), address(verifier));

        vm.expectRevert(bytes("roleManager is not an ICS26Router: it does not answer getNextClientSeq"));
        script.deploy(path);
    }

    function test_AZeroCombinedVerifyingKeyIsRefused() public {
        string memory path = _config("zero-uc-vkey", address(verifier), router);
        vm.writeJson(vm.toString(bytes32(0)), path, ".ucAndMembershipVkey");

        vm.expectRevert(bytes("ucAndMembershipVkey is zero"));
        script.deploy(path);
    }

    function test_AZeroVerifyingKeyIsRefused() public {
        string memory path = _config("zero-vkey", address(verifier), router);
        vm.writeJson(vm.toString(bytes32(0)), path, ".updateClientVkey");

        vm.expectRevert(bytes("updateClientVkey is zero"));
        script.deploy(path);
    }

    /// @dev Two programs sharing a key means one of them was pinned to the other's, which would let
    /// a proof of one be submitted as a proof of another — a membership proof as a header update,
    /// or a plain membership proof where a combined one was required.
    function test_TwoProgramsSharingAVerifyingKeyAreRefused() public {
        string memory path = _config("shared-vkey", address(verifier), router);

        vm.writeJson(vm.toString(vm.parseJsonBytes32(vm.readFile(path), ".membershipVkey")), path, ".updateClientVkey");

        vm.expectRevert(bytes("the three programs cannot share a verifying key"));
        script.deploy(path);
    }

    /// @dev Phase 2's membership proofs anchor on this hash. A genesis that left it zero would
    /// deploy a client that can verify headers and never verify a packet.
    function test_AZeroTransactionSetResultHashIsRefused() public {
        string memory path = _config("zero-result-hash", address(verifier), router);
        vm.writeJson(vm.toString(bytes32(0)), path, ".transactionSetResultHash");

        vm.expectRevert(bytes("the genesis transaction set result hash is zero: membership proofs anchor on it"));
        script.deploy(path);
    }
}
