import { getAbiItem, parseAbi, toEventSelector } from "viem";

export const consensusAbi = parseAbi([
	"function getCoordinator() view returns (address)",
	"function getActiveEpoch() external view returns (uint64 epoch, bytes32 groupId)",
	"function getEpochsState() external view returns (uint64 previous, uint64 active, uint64 staged, uint64 rolloverBlock)",
	"function getEpochGroupId(uint64 epoch) external view returns (bytes32 groupId)",
	"event TransactionProposed(bytes32 indexed safeTxHash, bytes32 indexed safeId, address indexed oracle, uint64 epoch, (uint256 chainId, address safe, address to, uint256 value, bytes data, uint8 operation, uint256 safeTxGas, uint256 baseGas, uint256 gasPrice, address gasToken, address refundReceiver, uint256 nonce) transaction)",
	"event TransactionAttested(bytes32 indexed safeTxHash, bytes32 indexed safeId, address indexed oracle, uint64 epoch, bytes32 signatureId, ((uint256 x, uint256 y) r, uint256 z) attestation)",
	"event EpochProposed(uint64 indexed activeEpoch, uint64 indexed proposedEpoch, uint64 rolloverBlock, bytes32 groupId, (uint256 x, uint256 y) groupKey)",
	"event EpochStaged(uint64 indexed activeEpoch, uint64 indexed proposedEpoch, uint64 rolloverBlock, bytes32 groupId, (uint256 x, uint256 y) groupKey, bytes32 signatureId, ((uint256 x, uint256 y) r, uint256 z) attestation)",
]);

export const transactionEventSelectors = ["TransactionProposed" as const, "TransactionAttested" as const].map(
	(eventName) => toEventSelector(getAbiItem({ abi: consensusAbi, name: eventName })),
);

export const proposedEventSelectors = ["TransactionProposed" as const].map((eventName) =>
	toEventSelector(getAbiItem({ abi: consensusAbi, name: eventName })),
);
