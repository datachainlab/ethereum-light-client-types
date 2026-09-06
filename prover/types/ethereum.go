package types

import "fmt"

// IsGloas reports whether this fork spec is Gloas or later.
//
// Gloas is identified by execution_block_hash_gindex being set, which is the same
// discriminator the Rust verifier uses (`ForkSpec::is_gloas`).
func (s *ForkSpec) IsGloas() bool {
	return s.GetExecutionBlockHashGindex() != 0
}

// ForkAtEpoch returns the fork active at `epoch`, or nil if `epoch` precedes every fork.
func (p *ForkParameters) ForkAtEpoch(epoch uint64) *Fork {
	forks := p.GetForks()
	for i := len(forks) - 1; i >= 0; i-- {
		if epoch >= forks[i].GetEpoch() {
			return forks[i]
		}
	}
	return nil
}

// IsGloas reports whether the fork active at `epoch` is Gloas or later.
func (p *ForkParameters) IsGloas(epoch uint64) bool {
	return p.ForkAtEpoch(epoch).GetSpec().IsGloas()
}

func (u *ConsensusUpdate) ValidateBasic() error {
	if u == nil {
		return fmt.Errorf("light client update cannot be nil")
	}
	if u.AttestedHeader == nil {
		return fmt.Errorf("attested header cannot be nil")
	}
	if u.FinalizedHeader == nil {
		return fmt.Errorf("finalized header cannot be nil")
	}
	if u.FinalizedHeaderBranch == nil {
		return fmt.Errorf("finalized header branch cannot be nil")
	}
	if u.FinalizedExecutionRoot == nil {
		return fmt.Errorf("finalized execution root cannot be nil")
	}
	if u.FinalizedExecutionBranch == nil {
		return fmt.Errorf("finalized execution branch cannot be nil")
	}
	if u.SyncAggregate == nil {
		return fmt.Errorf("sync aggregate cannot be nil")
	}
	if u.SignatureSlot == 0 {
		return fmt.Errorf("signature slot cannot be zero")
	}
	return nil
}

func (u *AccountUpdate) ValidateBasic() error {
	if u == nil {
		return fmt.Errorf("account update cannot be nil")
	}
	if u.AccountProof == nil {
		return fmt.Errorf("account proof cannot be nil")
	}
	if u.AccountStorageRoot == nil {
		return fmt.Errorf("account storage root cannot be nil")
	}
	return nil
}

func (u *ExecutionUpdate) ValidateBasic() error {
	if u == nil {
		return fmt.Errorf("execution update cannot be nil")
	}
	if u.StateRoot == nil {
		return fmt.Errorf("state root cannot be nil")
	}
	// Gloas proves the execution header by hashing its RLP instead of walking SSZ merkle
	// branches, so StateRootBranch/BlockNumberBranch are intentionally absent there.
	if len(u.Rlp) > 0 {
		if u.BlockHash == nil {
			return fmt.Errorf("block hash cannot be nil")
		}
		return nil
	}
	if u.StateRootBranch == nil {
		return fmt.Errorf("state root branch cannot be nil")
	}
	if u.BlockNumberBranch == nil {
		return fmt.Errorf("block number branch cannot be nil")
	}
	return nil
}
