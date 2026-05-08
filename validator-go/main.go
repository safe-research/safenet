package main

import (
	"context"
	"errors"
	"flag"
	"log"
	"os"
	"os/signal"
	"syscall"

	"github.com/safe-research/safenet/validator-go/config"
	"github.com/safe-research/safenet/validator-go/driver"
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

	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer cancel()

	if err := driver.Run(ctx, cfg); err != nil && !errors.Is(err, context.Canceled) {
		log.Fatalf("validator stopped: %v", err)
	}
	log.Println("validator stopped")
}
