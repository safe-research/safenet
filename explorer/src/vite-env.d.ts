/// <reference types="vite/client" />

// Custom constants defined in vite.config.js via the define option
declare const __BASE_PATH__: string;
declare const __DOCS_URL__: string;
declare const __TERMS_URL__: string;
declare const __PRIVACY_URL__: string;
declare const __IMPRINT_URL__: string;

// Default explorer settings — configurable per deployment via VITE_DEFAULT_* env vars
declare const __DEFAULT_CONSENSUS__: string;
declare const __DEFAULT_RPC__: string;
declare const __DEFAULT_DECODER__: string;
declare const __DEFAULT_RELAYER__: string;
declare const __DEFAULT_MAX_BLOCK_RANGE__: number;
declare const __DEFAULT_VALIDATOR_INFO__: string;
declare const __DEFAULT_REFETCH_INTERVAL__: number;
declare const __DEFAULT_BLOCKS_PER_EPOCH__: number;
declare const __DEFAULT_SIGNING_TIMEOUT__: number;
