// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.28;

// solhint-disable custom-errors,gas-custom-errors,gas-small-strings,reason-string

import { stdJson } from "forge-std/StdJson.sol";
import { Script } from "forge-std/Script.sol";
import { console } from "forge-std/console.sol";

import { SP1ICSStellar } from "../contracts/light-clients/sp1-stellar/SP1ICSStellar.sol";
import { IICSStellarMsgs } from "../contracts/light-clients/sp1-stellar/msgs/IICSStellarMsgs.sol";
import { IICS02ClientMsgs } from "../contracts/msgs/IICS02ClientMsgs.sol";
import { IICS02Client } from "../contracts/interfaces/IICS02Client.sol";

/// @title Deploy SP1ICSStellar
/// @notice Deploys the Stellar light client from a genesis config.
/// @dev The config is produced by `cargo run -p sp1-stellar-prover --bin genesis`, so the trust
/// root, the verifying keys and the trusted ledger all come from the same verified source rather
/// than being retyped. Two fields it cannot produce are the SP1 verifier gateway and the role
/// manager, because both are chain-specific; this script refuses to deploy until they are set.
///
/// The genesis ledger is the one thing here that no later proof checks. Every update is proven
/// against the consensus state stored at deployment, so a wrong ledger hash produces a client that
/// verifies proofs correctly against a chain that does not exist. Check it against a public
/// explorer before deploying, not after.
///
/// ```sh
/// forge script scripts/DeploySP1ICSStellar.s.sol --rpc-url "$RPC_URL"
/// forge script scripts/DeploySP1ICSStellar.s.sol --rpc-url "$RPC_URL" --broadcast --verify
/// ```
contract DeploySP1ICSStellar is Script {
    using stdJson for string;

    /// @notice Path of the genesis config, relative to the `ibc-solidity` directory.
    string public constant DEFAULT_CONFIG = "scripts/deployments/stellar/genesis.json";

    /// @notice Deploys the client from the config named by STELLAR_GENESIS, or the default.
    /// @return client The deployed SP1ICSStellar.
    function run() public returns (SP1ICSStellar client) {
        return deploy(vm.envOr("STELLAR_GENESIS", DEFAULT_CONFIG));
    }

    /// @notice Deploys the client from a named genesis config.
    /// @dev Taking the path as an argument rather than only from the environment is what lets the
    /// tests run concurrently: an environment variable is process-global, so one case setting it
    /// changes which config every other in-flight case reads. It is named `deploy` rather than
    /// being an overload of `run` because `forge script` refuses an ABI with two `run` entries.
    /// @param path Path of the genesis config, relative to the `ibc-solidity` directory.
    /// @return client The deployed SP1ICSStellar.
    function deploy(string memory path) public returns (SP1ICSStellar client) {
        string memory json = vm.readFile(path);

        address verifier = json.readAddress(".verifier");
        address roleManager = json.readAddress(".roleManager");

        require(verifier != address(0), "verifier is unset: fill in .verifier in the genesis config");
        require(
            verifier.code.length > 0, "no contract at .verifier on this chain: check the SP1 verifier gateway address"
        );
        require(roleManager != address(0), "roleManager is unset: fill in .roleManager in the genesis config");
        require(
            roleManager.code.length > 0,
            "roleManager has no code: it must be the ICS26Router proxy, which is what calls verifyMembership"
        );

        (bool answered, bytes memory nextClientSequence) =
            roleManager.staticcall(abi.encodeCall(IICS02Client.getNextClientSeq, ()));

        require(
            answered && nextClientSequence.length == 32,
            "roleManager is not an ICS26Router: it does not answer getNextClientSeq"
        );

        bytes32 updateClientVkey = json.readBytes32(".updateClientVkey");
        bytes32 membershipVkey = json.readBytes32(".membershipVkey");
        bytes32 ucAndMembershipVkey = json.readBytes32(".ucAndMembershipVkey");

        require(updateClientVkey != bytes32(0), "updateClientVkey is zero");
        require(membershipVkey != bytes32(0), "membershipVkey is zero");
        require(ucAndMembershipVkey != bytes32(0), "ucAndMembershipVkey is zero");
        require(
            updateClientVkey != membershipVkey && updateClientVkey != ucAndMembershipVkey
                && membershipVkey != ucAndMembershipVkey,
            "the three programs cannot share a verifying key"
        );

        IICSStellarMsgs.ClientState memory clientState = IICSStellarMsgs.ClientState({
            networkId: json.readBytes32(".networkId"),
            trustRootFingerprint: json.readBytes32(".trustRootFingerprint"),
            routerContractId: json.readBytes32(".routerContractId"),
            rootEventTopic: json.readBytes(".rootEventTopic"),
            latestHeight: IICS02ClientMsgs.Height({
                revisionNumber: uint64(json.readUint(".revisionNumber")),
                revisionHeight: uint64(json.readUint(".revisionHeight"))
            }),
            isFrozen: false,
            zkAlgorithm: IICSStellarMsgs.SupportedZkAlgorithm.Groth16
        });

        require(clientState.networkId != bytes32(0), "networkId is zero");
        require(clientState.trustRootFingerprint != bytes32(0), "trustRootFingerprint is zero");
        require(clientState.routerContractId != bytes32(0), "routerContractId is zero");
        require(clientState.rootEventTopic.length > 0, "rootEventTopic is empty");
        require(clientState.latestHeight.revisionHeight > 0, "the genesis ledger is zero");

        IICSStellarMsgs.ConsensusState memory consensusState = IICSStellarMsgs.ConsensusState({
            closeTime: uint64(json.readUint(".closeTime")),
            ledgerHash: json.readBytes32(".ledgerHash"),
            previousLedgerHash: json.readBytes32(".previousLedgerHash"),
            bucketListHash: json.readBytes32(".bucketListHash"),
            transactionSetResultHash: json.readBytes32(".transactionSetResultHash")
        });

        require(consensusState.ledgerHash != bytes32(0), "the genesis ledger hash is zero");
        require(consensusState.closeTime > 0, "the genesis close time is zero");
        require(consensusState.bucketListHash != bytes32(0), "the genesis bucket list hash is zero");
        require(
            consensusState.transactionSetResultHash != bytes32(0),
            "the genesis transaction set result hash is zero: membership proofs anchor on it"
        );

        vm.startBroadcast();

        client = new SP1ICSStellar(
            updateClientVkey,
            membershipVkey,
            ucAndMembershipVkey,
            verifier,
            abi.encode(clientState),
            keccak256(abi.encode(consensusState)),
            roleManager
        );

        vm.stopBroadcast();

        _report(client, clientState, consensusState);
    }

    /// @notice Prints what was deployed, so the values can be checked against the chains.
    /// @param client The deployed client.
    /// @param clientState The client state it was deployed with.
    /// @param consensusState The consensus state it trusts at genesis.
    function _report(
        SP1ICSStellar client,
        IICSStellarMsgs.ClientState memory clientState,
        IICSStellarMsgs.ConsensusState memory consensusState
    )
        private
        view
    {
        console.log("SP1ICSStellar          ", address(client));
        console.log("SP1 verifier           ", address(client.VERIFIER()));
        console.log("update-client vkey     ", vm.toString(client.UPDATE_CLIENT_PROGRAM_VKEY()));
        console.log("membership vkey        ", vm.toString(client.MEMBERSHIP_PROGRAM_VKEY()));
        console.log("uc+membership vkey     ", vm.toString(client.UPDATE_CLIENT_AND_MEMBERSHIP_PROGRAM_VKEY()));
        console.log("trusted ledger         ", clientState.latestHeight.revisionHeight);
        console.log("trusted ledger hash    ", vm.toString(consensusState.ledgerHash));
        console.log("trust root fingerprint ", vm.toString(clientState.trustRootFingerprint));
        console.log("router contract        ", vm.toString(clientState.routerContractId));
    }
}
