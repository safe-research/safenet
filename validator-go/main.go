package main

import (
	"context"
	"flag"
	"log"

	"github.com/safe-research/safenet/validator-go/config"
	"github.com/safe-research/safenet/validator-go/network"
)

func main() {
	configPath := flag.String("config", "", "path to TOML config file")
	flag.Parse()

	if *configPath == "" {
		log.Fatal("usage: validator-go -config <path>")
	}

	cfg, err := config.Load(*configPath)
	if err != nil {
		log.Fatalf("failed to load config: %v", err)
	}

	addrs, err := network.Resolve(context.Background(), cfg.RPCURL, cfg.ConsensusAddress, cfg.BlocksPerEpoch)
	if err != nil {
		log.Fatalf("failed to resolve addresses: %v", err)
	}

	log.Printf("chain=%s blocks_per_epoch=%d consensus=%s coordinator=%s",
		addrs.Chain, addrs.BlocksPerEpoch,
		addrs.ConsensusAddress.Hex(), addrs.CoordinatorAddress.Hex())
}
