//go:build nogui

package main

import (
	"context"

	"github.com/unalivin/templateapp/internal/cli"
)

func run(ctx context.Context, env *cli.Env) int {
	return cli.Run(ctx, env)
}
