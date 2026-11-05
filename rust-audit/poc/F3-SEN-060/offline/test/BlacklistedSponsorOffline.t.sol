// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test, console} from "forge-std/Test.sol";
import {IERC20} from "@oz/token/ERC20/IERC20.sol";
import {SentinelOracle} from "@/SentinelOracle.sol";
import {SentinelOracleRequest} from "@/libraries/SentinelOracleRequests.sol";
import {UsdcCode} from "../src/UsdcCode.sol";

interface IFiatToken {
    function blacklister() external view returns (address);
    function blacklist(address account) external;
    function unBlacklist(address account) external;
    function isBlacklisted(address account) external view returns (bool);
    function paused() external view returns (bool);
}

/// F3-SEN-060, contract half, with no network: the audit-commit `SentinelOracle` deployed locally
/// with the `contracts/.env.aegis.sample` parameters, on the real Gnosis USDC.e runtime code and
/// proxy storage at block 48,635,154, taken from Foundry's RPC cache.
contract BlacklistedSponsorOfflineTest is Test {
    // contracts/.env.aegis.sample
    address constant ARBITRATOR = 0xc89Ae48382edc81287AcEf0881E88DEacAD58348;
    address constant GOVERNANCE = 0x9D02bed59170cdE96782Ea9a9b17F9bCF4916127;
    address constant FUNDS_RECEIVER = 0x9D02bed59170cdE96782Ea9a9b17F9bCF4916127;
    uint96 constant REQUEST_FEE = 400_000;
    uint32 constant COMMIT_WINDOW = 6;
    uint32 constant REVEAL_WINDOW = 6;
    uint32 constant GOVERNANCE_DELAY = 51_840;
    uint32 constant BOND_MULTIPLIER = 2_000;
    uint32 constant SLASHING_MULTIPLIER = 1;
    uint24 constant DAO_FEE_SHARE = 0;
    uint32 constant ARBITRATION_TIMEOUT = 483_840;

    uint256 constant BOND = uint256(REQUEST_FEE) * BOND_MULTIPLIER;
    uint256 constant SENTINEL_FUNDS = 1_000e6;
    uint256 constant SPONSOR_FUNDS = 10e6;
    bytes32 constant SALT = bytes32(uint256(1));
    bytes32 constant REQUEST_ID = keccak256("F3-SEN-060 offline request");
    // Implementation slot of the ZeppelinOS proxy that FiatTokenProxy is.
    bytes32 constant IMPLEMENTATION_SLOT = keccak256("org.zeppelinos.proxy.implementation");

    IERC20 constant TOKEN = IERC20(UsdcCode.PROXY);
    IFiatToken constant FIAT = IFiatToken(UsdcCode.PROXY);

    event ArbitrationTimedOut(bytes32 indexed requestId);

    SentinelOracle oracle;
    // Stands in for the Aegis Consensus, the oracle's PROPOSER. `Consensus.proposeTransaction` calls
    // `postRequest(message, msg.sender, oracleData)` (contracts/src/Consensus.sol:266), so the
    // sponsor is the proposing account, as here.
    address consensus = makeAddr("consensus");
    address sponsor = makeAddr("sponsor");
    address anyone = makeAddr("anyone");
    address[3] sentinels;

    function setUp() public {
        vm.chainId(100);
        vm.roll(UsdcCode.BLOCK);
        vm.etch(UsdcCode.PROXY, UsdcCode.proxyCode());
        vm.etch(UsdcCode.IMPL, UsdcCode.implCode());
        (bytes32[] memory keys, bytes32[] memory values) = UsdcCode.slots();
        for (uint256 i = 0; i < keys.length; i++) {
            vm.store(UsdcCode.PROXY, keys[i], values[i]);
        }

        oracle = new SentinelOracle(
            SentinelOracle.ConstructorParams({
                arbitrator: ARBITRATOR,
                governance: GOVERNANCE,
                protocolFundsReceiver: FUNDS_RECEIVER,
                proposer: consensus,
                feeToken: UsdcCode.PROXY,
                requestFee: REQUEST_FEE,
                initialBondMultiplier: BOND_MULTIPLIER,
                initialSlashingMultiplier: SLASHING_MULTIPLIER,
                initialDaoFeeShare: DAO_FEE_SHARE,
                commitWindow: COMMIT_WINDOW,
                revealWindow: REVEAL_WINDOW,
                governanceDelay: GOVERNANCE_DELAY,
                arbitrationTimeout: ARBITRATION_TIMEOUT,
                initialCharterEns: "charter.safenet-gov.eth"
            })
        );

        // Funding only, not part of the route.
        deal(address(TOKEN), sponsor, SPONSOR_FUNDS);
        for (uint256 i = 0; i < 3; i++) {
            sentinels[i] = makeAddr(string.concat("sentinel", vm.toString(i)));
            deal(address(TOKEN), sentinels[i], SENTINEL_FUNDS);
        }
    }

    /// Registers three sentinels, posts a request and splits the reveals 2 approve to 1 deny, so the
    /// last reveal finalizes the request into FROZEN.
    function _freeze() internal returns (uint64 deadline) {
        for (uint256 i = 0; i < 3; i++) {
            vm.prank(GOVERNANCE);
            oracle.addSentinel(sentinels[i]);
        }
        vm.roll(block.number + GOVERNANCE_DELAY);

        vm.prank(sponsor);
        TOKEN.approve(address(oracle), REQUEST_FEE);
        vm.prank(consensus);
        oracle.postRequest(REQUEST_ID, sponsor, "");
        uint256 posted = block.number;

        for (uint256 i = 0; i < 3; i++) {
            vm.startPrank(sentinels[i]);
            TOKEN.approve(address(oracle), BOND);
            oracle.commit(REQUEST_ID, oracle.hashCommitment(sentinels[i], REQUEST_ID, i < 2, SALT, ""));
            vm.stopPrank();
        }
        vm.roll(posted + COMMIT_WINDOW + 1);
        for (uint256 i = 0; i < 3; i++) {
            vm.prank(sentinels[i]);
            oracle.reveal(REQUEST_ID, i < 2, SALT, "");
        }

        SentinelOracleRequest.T memory request = oracle.getRequest(REQUEST_ID);
        assertEq(uint8(request.progress.state), uint8(SentinelOracleRequest.State.FROZEN));
        assertEq(request.progress.arbitrationDeadline, block.number + ARBITRATION_TIMEOUT);
        assertEq(TOKEN.balanceOf(address(oracle)), 3 * BOND + REQUEST_FEE);
        deadline = request.progress.arbitrationDeadline;
    }

    function _state() internal view returns (SentinelOracleRequest.State) {
        return oracle.getRequest(REQUEST_ID).progress.state;
    }

    /// The etched code hashes to the cache's code_hash values, and the proxy reaches the
    /// implementation and the cached storage: implementation slot, blacklister, not paused.
    function test_etchedUsdcMatchesCache() public view {
        assertEq(keccak256(UsdcCode.PROXY.code), UsdcCode.PROXY_CODE_HASH);
        assertEq(keccak256(UsdcCode.IMPL.code), UsdcCode.IMPL_CODE_HASH);
        assertEq(vm.load(UsdcCode.PROXY, IMPLEMENTATION_SLOT), bytes32(uint256(uint160(UsdcCode.IMPL))));
        assertEq(FIAT.blacklister(), 0x458cD345B4C05e8DF39d0A07220feb4Ec19F5e6f);
        assertFalse(FIAT.paused());
    }

    function test_blacklistedSponsorLocksFrozenBondsUntilUnblacklisted() public {
        uint64 deadline = _freeze();
        uint256 locked = TOKEN.balanceOf(address(oracle));
        console.log("USDC.e held by the oracle for the frozen request (raw)", locked);
        console.log("arbitrationDeadline", uint256(deadline));

        // Precondition of the finding: USDC.e's blacklister blacklists the sponsor.
        address blacklister = FIAT.blacklister();
        vm.prank(blacklister);
        FIAT.blacklist(sponsor);
        assertTrue(FIAT.isBlacklisted(sponsor));

        // The arbitrator can neither rule nor decline: both push the fee to the sponsor first.
        vm.prank(ARBITRATOR);
        vm.expectRevert(bytes("Blacklistable: account is blacklisted"));
        oracle.resolveDispute(REQUEST_ID, true, "ruling");
        vm.prank(ARBITRATOR);
        vm.expectRevert(bytes("Blacklistable: account is blacklisted"));
        oracle.markOutOfScope(REQUEST_ID, "out of scope");

        // At deadline + 1 every sentinel sends the pair its dispute-timeout arm queues:
        // timeoutArbitration(), then claim().
        vm.roll(uint256(deadline) + 1);
        console.log("block of the sentinels' timeoutArbitration and claim", block.number);
        for (uint256 i = 0; i < 3; i++) {
            vm.prank(sentinels[i]);
            vm.expectRevert(bytes("Blacklistable: account is blacklisted"));
            oracle.timeoutArbitration(REQUEST_ID);
            vm.prank(sentinels[i]);
            vm.expectRevert(SentinelOracleRequest.RequestNotResolved.selector);
            oracle.claim(REQUEST_ID);
        }

        // Nothing moved: still FROZEN, every bond still in the oracle.
        assertEq(uint8(_state()), uint8(SentinelOracleRequest.State.FROZEN));
        assertEq(TOKEN.balanceOf(address(oracle)), locked);
        for (uint256 i = 0; i < 3; i++) {
            assertEq(TOKEN.balanceOf(sentinels[i]), SENTINEL_FUNDS - BOND);
        }

        // Once the blacklist is lifted, anyone can time the request out and every sentinel gets its
        // full bond back.
        vm.prank(blacklister);
        FIAT.unBlacklist(sponsor);
        vm.expectEmit(true, false, false, false, address(oracle));
        emit ArbitrationTimedOut(REQUEST_ID);
        vm.prank(anyone);
        oracle.timeoutArbitration(REQUEST_ID);
        assertEq(uint8(_state()), uint8(SentinelOracleRequest.State.TIMED_OUT));
        assertEq(TOKEN.balanceOf(sponsor), SPONSOR_FUNDS, "fee refunded");
        for (uint256 i = 0; i < 3; i++) {
            vm.prank(sentinels[i]);
            oracle.claim(REQUEST_ID);
            assertEq(TOKEN.balanceOf(sentinels[i]), SENTINEL_FUNDS, "full bond back");
        }
        assertEq(TOKEN.balanceOf(address(oracle)), 0);
    }

    /// Control: the same frozen request without the blacklist. At deadline + 1 the first sentinel's
    /// timeoutArbitration() succeeds, the others revert RequestNotFrozen, and every claim() succeeds,
    /// as the comment in the dispute-timeout arm expects.
    function test_controlWithoutBlacklistEveryClaimSucceeds() public {
        uint64 deadline = _freeze();
        vm.roll(uint256(deadline) + 1);
        for (uint256 i = 0; i < 3; i++) {
            vm.prank(sentinels[i]);
            if (i > 0) {
                vm.expectRevert(SentinelOracleRequest.RequestNotFrozen.selector);
            }
            oracle.timeoutArbitration(REQUEST_ID);
            vm.prank(sentinels[i]);
            oracle.claim(REQUEST_ID);
            assertEq(TOKEN.balanceOf(sentinels[i]), SENTINEL_FUNDS);
        }
        assertEq(uint8(_state()), uint8(SentinelOracleRequest.State.TIMED_OUT));
        assertEq(TOKEN.balanceOf(sponsor), SPONSOR_FUNDS);
        assertEq(TOKEN.balanceOf(address(oracle)), 0);
    }
}
