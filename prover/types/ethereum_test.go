package types

import "testing"

func fork(epoch uint64, spec *ForkSpec) *Fork {
	return &Fork{Epoch: epoch, Spec: spec}
}

var (
	preGloasSpec = &ForkSpec{FinalizedRootGindex: 105}
	gloasSpec    = &ForkSpec{FinalizedRootGindex: 735, ExecutionBlockHashGindex: 2856}
)

func TestForkSpecIsGloas(t *testing.T) {
	tests := []struct {
		name     string
		spec     *ForkSpec
		expected bool
	}{
		{"nil", nil, false},
		{"pre-Gloas leaves execution_block_hash_gindex unset", preGloasSpec, false},
		{"Gloas sets execution_block_hash_gindex", gloasSpec, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.spec.IsGloas(); got != tt.expected {
				t.Errorf("IsGloas() = %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestForkParametersForkAtEpoch(t *testing.T) {
	params := &ForkParameters{Forks: []*Fork{
		fork(0, preGloasSpec),
		fork(10, preGloasSpec),
		fork(20, gloasSpec),
	}}

	tests := []struct {
		name     string
		epoch    uint64
		expected *Fork
	}{
		{"first fork", 0, params.Forks[0]},
		{"between forks", 9, params.Forks[0]},
		{"exactly on a fork epoch", 10, params.Forks[1]},
		{"latest fork", 100, params.Forks[2]},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := params.ForkAtEpoch(tt.epoch); got != tt.expected {
				t.Errorf("ForkAtEpoch(%v) = %v, want %v", tt.epoch, got, tt.expected)
			}
		})
	}

	t.Run("epoch before every fork", func(t *testing.T) {
		early := &ForkParameters{Forks: []*Fork{fork(5, preGloasSpec)}}
		if got := early.ForkAtEpoch(4); got != nil {
			t.Errorf("ForkAtEpoch(4) = %v, want nil", got)
		}
	})
}

func TestForkParametersIsGloas(t *testing.T) {
	params := &ForkParameters{Forks: []*Fork{
		fork(0, preGloasSpec),
		fork(20, gloasSpec),
	}}

	tests := []struct {
		name     string
		params   *ForkParameters
		epoch    uint64
		expected bool
	}{
		{"before Gloas", params, 19, false},
		{"at the Gloas epoch", params, 20, true},
		{"after Gloas", params, 21, true},
		{"epoch before every fork", &ForkParameters{Forks: []*Fork{fork(5, gloasSpec)}}, 4, false},
		{"no forks", &ForkParameters{}, 0, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.params.IsGloas(tt.epoch); got != tt.expected {
				t.Errorf("IsGloas(%v) = %v, want %v", tt.epoch, got, tt.expected)
			}
		})
	}
}
