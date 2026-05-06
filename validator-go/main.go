package main

import (
	"flag"
	"fmt"
	"log"
	"os"

	"github.com/safe-research/safenet/validator-go/config"
)

func main() {
	configPath := flag.String("config", "", "path to TOML config file")
	flag.Parse()

	if *configPath == "" {
		fmt.Fprintln(os.Stderr, "usage: validator-go -config <path>")
		os.Exit(1)
	}

	cfg, err := config.Load(*configPath)
	if err != nil {
		log.Fatalf("failed to load config: %v", err)
	}

	fmt.Printf("config loaded: rpc_url=%s consensus_address=%s participants=%d\n",
		cfg.RPCURL, cfg.ConsensusAddress.Hex(), len(cfg.Participants))
}
